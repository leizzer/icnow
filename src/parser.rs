use anyhow::Result;
use graphqlite::Graph;
use std::collections::HashMap;
use std::fs;
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

#[derive(Debug, Default)]
pub struct FileSummary {
    pub file_path: String,
    pub imports: Vec<String>,
    // Maps Structure Label -> List of Names
    pub structures: HashMap<String, Vec<String>>,
    // Maps Standalone Function Label -> List of Names
    pub standalone_functions: HashMap<String, Vec<String>>,
    // Parent Name -> List of (Label, Name)
    pub methods: HashMap<String, Vec<(String, String)>>,
}

fn get_ruby_namespace(node: tree_sitter::Node, source_code: &[u8]) -> Result<String> {
    let mut parts = Vec::new();
    let mut curr = Some(node);
    while let Some(n) = curr {
        if n.kind() == "class" || n.kind() == "module" {
            if let Some(name_node) = n.child_by_field_name("name") {
                if let Ok(text) = name_node.utf8_text(source_code) {
                    parts.push(text.to_string());
                }
            }
        }
        curr = n.parent();
    }
    parts.reverse();
    Ok(parts.join("::"))
}

fn get_ts_namespace(node: tree_sitter::Node, source_code: &[u8]) -> Result<String> {
    let mut parts = Vec::new();
    let mut curr = Some(node);
    while let Some(n) = curr {
        let kind = n.kind();
        if kind == "class_declaration" || kind == "interface_declaration" || kind == "internal_module" {
            if let Some(name_node) = n.child_by_field_name("name") {
                if let Ok(text) = name_node.utf8_text(source_code) {
                    parts.push(text.to_string());
                }
            }
        }
        curr = n.parent();
    }
    parts.reverse();
    Ok(parts.join("::"))
}

pub fn parse_file(file_path: &str, graph: &Graph) -> Result<FileSummary> {
    let source_code = fs::read_to_string(file_path)?;
    let mut parser = Parser::new();
    
    let (language, query_str) = if file_path.ends_with(".rs") {
        (tree_sitter_rust::LANGUAGE.into(), r#"
            (function_item name: (identifier) @name) @function.node
            (struct_item name: (type_identifier) @name) @struct.node
            (use_declaration) @import.node
            (call_expression function: _ @call.func) @call.node
        "#)
    } else if file_path.ends_with(".rb") {
        (tree_sitter_ruby::LANGUAGE.into(), r#"
            (method name: (identifier) @name) @function.node
            (singleton_method name: (identifier) @name) @function.node
            (class name: _ @name) @struct.node
            (module name: _ @name) @struct.node
            (call method: (identifier) @import.method arguments: (argument_list (string (string_content) @name))) @import.node
            (call receiver: _ @call.receiver method: [(identifier) (constant)] @call.func) @call.node
            (call method: [(identifier) (constant)] @call.func) @call.node
        "#)
    } else if file_path.ends_with(".ts") || file_path.ends_with(".tsx") {
        let lang = if file_path.ends_with(".tsx") {
            tree_sitter_typescript::LANGUAGE_TSX.into()
        } else {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
        };
        (lang, r#"
            (function_declaration name: (identifier) @name) @function.node
            (method_definition name: (property_identifier) @name) @function.node
            (class_declaration name: (type_identifier) @name) @struct.node
            (interface_declaration name: (type_identifier) @name) @struct.node
            (internal_module name: (identifier) @name) @struct.node
            (import_statement source: (string (string_fragment) @name)) @import.node
            (call_expression function: _ @call.func) @call.node
        "#)
    } else {
        return Err(anyhow::anyhow!("Unsupported file extension: {}", file_path));
    };
    
    parser.set_language(&language)?;

    let tree = parser.parse(&source_code, None).ok_or_else(|| anyhow::anyhow!("Failed to parse code"))?;
    let root_node = tree.root_node();

    let query = Query::new(&language, query_str)
        .map_err(|e| anyhow::anyhow!("Invalid query: {:?}", e))?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, root_node, source_code.as_bytes());

    let mut file_props = HashMap::new();
    file_props.insert("name".to_string(), file_path.to_string());
    if let Ok(metadata) = fs::metadata(file_path) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                file_props.insert("last_modified".to_string(), duration.as_secs().to_string());
            }
        }
    }
    
    let file_node = crate::models::Node {
        id: file_path.to_string(),
        label: "File".to_string(),
        kind: "file".to_string(),
        properties: file_props,
    };

    let mut summary = FileSummary {
        file_path: file_path.to_string(),
        ..Default::default()
    };

    let mut process_all = || -> Result<()> {
        file_node.save(graph)?;

        while let Some(m) = matches.next() {
            let mut capture_map = HashMap::new();
            for capture in m.captures {
                let name: &str = &query.capture_names()[capture.index as usize];
                capture_map.insert(name, capture.node);
            }

            let mut node_name = String::new();
            let mut kind = String::new();
            let mut label = String::new();
            let mut node_code = String::new();

            if let Some(&func_node) = capture_map.get("function.node") {
                let mut name = capture_map.get("name")
                    .and_then(|n| n.utf8_text(source_code.as_bytes()).ok())
                    .unwrap_or("")
                    .to_string();
                
                kind = func_node.kind().to_string();
                if file_path.ends_with(".rs") {
                    label = "Function".to_string();
                    // Check if it's inside an impl block
                    if let Some(dl) = func_node.parent() {
                        if dl.kind() == "declaration_list" {
                            if let Some(impl_item) = dl.parent() {
                                if impl_item.kind() == "impl_item" {
                                    if let Some(type_node) = impl_item.child_by_field_name("type") {
                                        if let Ok(struct_name) = type_node.utf8_text(source_code.as_bytes()) {
                                            name = format!("{}::{}", struct_name, name);
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if file_path.ends_with(".rb") {
                    label = "Method".to_string();
                    let ns = get_ruby_namespace(func_node, source_code.as_bytes())?;
                    let method_name = func_node.child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source_code.as_bytes()).ok())
                        .unwrap_or("");
                    name = if ns.is_empty() {
                        method_name.to_string()
                    } else {
                        format!("{}::{}", ns, method_name)
                    };
                } else {
                    label = if kind == "method_definition" { "Method".to_string() } else { "Function".to_string() };
                    let ns = get_ts_namespace(func_node, source_code.as_bytes())?;
                    let func_name = func_node.child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source_code.as_bytes()).ok())
                        .unwrap_or("");
                    name = if ns.is_empty() {
                        func_name.to_string()
                    } else {
                        format!("{}::{}", ns, func_name)
                    };
                }
                node_name = name;
                node_code = func_node.utf8_text(source_code.as_bytes())?.to_string();
            } else if let Some(&struct_node) = capture_map.get("struct.node") {
                let mut name = capture_map.get("name")
                    .and_then(|n| n.utf8_text(source_code.as_bytes()).ok())
                    .unwrap_or("")
                    .to_string();
                
                kind = struct_node.kind().to_string();
                if file_path.ends_with(".rb") {
                    label = if kind == "class" { "Class".to_string() } else { "Module".to_string() };
                    name = get_ruby_namespace(struct_node, source_code.as_bytes())?;
                } else if file_path.ends_with(".rs") {
                    label = "Struct".to_string();
                } else {
                    label = if kind == "class_declaration" { "Class".to_string() }
                            else if kind == "interface_declaration" { "Interface".to_string() }
                            else { "Module".to_string() }; // internal_module
                    name = get_ts_namespace(struct_node, source_code.as_bytes())?;
                }
                node_name = name;
                node_code = struct_node.utf8_text(source_code.as_bytes())?.to_string();
            } else if let Some(&import_node) = capture_map.get("import.node") {
                kind = "use_declaration".to_string();
                label = "Import".to_string();
                if file_path.ends_with(".rs") {
                    node_name = import_node.utf8_text(source_code.as_bytes())?.to_string();
                } else if file_path.ends_with(".rb") {
                    let mut is_valid_import = true;
                    if let Some(&method_node) = capture_map.get("import.method") {
                        let method_name = method_node.utf8_text(source_code.as_bytes())?.to_string();
                        if method_name != "require" && method_name != "include" {
                            is_valid_import = false;
                        }
                    }
                    if is_valid_import {
                        node_name = capture_map.get("name")
                            .and_then(|n| n.utf8_text(source_code.as_bytes()).ok())
                            .unwrap_or("")
                            .to_string();
                    } else {
                        label.clear();
                    }
                } else {
                    node_name = capture_map.get("name")
                        .and_then(|n| n.utf8_text(source_code.as_bytes()).ok())
                        .unwrap_or("")
                        .to_string();
                }
                node_code = import_node.utf8_text(source_code.as_bytes())?.to_string();
            } else if let Some(&call_node) = capture_map.get("call.node") {
                // Build full qualified call target: prefer receiver.method or receiver::method
                let func_text = capture_map.get("call.func")
                    .and_then(|n| n.utf8_text(source_code.as_bytes()).ok())
                    .unwrap_or("")
                    .to_string();
                let receiver_text = capture_map.get("call.receiver")
                    .and_then(|n| n.utf8_text(source_code.as_bytes()).ok())
                    .unwrap_or("")
                    .to_string();
                let target_name = if !receiver_text.is_empty() && !func_text.is_empty() {
                    format!("{}.{}", receiver_text, func_text)
                } else {
                    func_text
                };
                
                if !target_name.is_empty() {
                    let mut curr = Some(call_node);
                    let mut enclosing_func_name = String::new();
                    while let Some(n) = curr {
                        let k = n.kind();
                        if k == "function_item" || k == "method" || k == "singleton_method" || k == "function_declaration" || k == "method_definition" {
                            if let Some(name_node) = n.child_by_field_name("name") {
                                if let Ok(text) = name_node.utf8_text(source_code.as_bytes()) {
                                    enclosing_func_name = text.to_string();
                                    if file_path.ends_with(".rs") {
                                        if let Some(dl) = n.parent() {
                                            if dl.kind() == "declaration_list" {
                                                if let Some(impl_item) = dl.parent() {
                                                    if impl_item.kind() == "impl_item" {
                                                        if let Some(type_node) = impl_item.child_by_field_name("type") {
                                                            if let Ok(struct_name) = type_node.utf8_text(source_code.as_bytes()) {
                                                                enclosing_func_name = format!("{}::{}", struct_name, enclosing_func_name);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    } else if file_path.ends_with(".rb") {
                                        if let Ok(ns) = get_ruby_namespace(n, source_code.as_bytes()) {
                                            if !ns.is_empty() {
                                                enclosing_func_name = format!("{}::{}", ns, enclosing_func_name);
                                            }
                                        }
                                    } else {
                                        if let Ok(ns) = get_ts_namespace(n, source_code.as_bytes()) {
                                            if !ns.is_empty() {
                                                enclosing_func_name = format!("{}::{}", ns, enclosing_func_name);
                                            }
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                        curr = n.parent();
                    }
                    
                    if !enclosing_func_name.is_empty() {
                        let source_id = format!("{}::{}", file_path, enclosing_func_name);
                        
                        // Workaround for graphqlite bug: target node MUST exist before edge creation
                        let mut props = HashMap::new();
                        props.insert("name".to_string(), target_name.clone());
                        let target_node = crate::models::Node {
                            id: target_name.clone(),
                            label: "Unresolved".to_string(),
                            kind: "unresolved_symbol".to_string(),
                            properties: props,
                        };
                        target_node.save(graph)?;

                        let edge_id = format!("{}::CALLS::{}", source_id, target_name);
                        let edge = crate::models::Edge {
                            id: edge_id,
                            source: source_id.clone(),
                            target: target_name.clone(),
                            label: "CALLS".to_string(),
                            properties: HashMap::new(),
                        };
                        edge.save(graph)?;
                    }
                }
                continue;
            }

            if !node_name.is_empty() && !label.is_empty() {
                let mut props = HashMap::new();
                props.insert("name".to_string(), node_name.clone());
                props.insert("file".to_string(), file_path.to_string());
                
                if !node_code.is_empty() {
                    props.insert("source_code".to_string(), node_code);
                }
                
                let id = format!("{}::{}", file_path, node_name);
                
                let node = crate::models::Node {
                    id: id.clone(),
                    label: label.clone(),
                    kind: kind.clone(),
                    properties: props,
                };
                
                node.save(graph)?;

                // Populate the FileSummary
                if label == "Import" {
                    summary.imports.push(node_name.clone());
                } else if label == "Class" || label == "Module" || label == "Struct" || label == "Interface" {
                    summary.structures.entry(label.clone()).or_insert_with(Vec::new).push(node_name.clone());
                } else if label == "Method" || label == "Function" {
                    if let Some((struct_part, method_part)) = node_name.rsplit_once("::") {
                        summary.methods.entry(struct_part.to_string())
                            .or_insert_with(Vec::new)
                            .push((label.clone(), method_part.to_string()));
                    } else {
                        summary.standalone_functions.entry(label.clone()).or_insert_with(Vec::new).push(node_name.clone());
                    }
                }

                // Create structural edge between File and Content Node
                let edge = crate::models::Edge {
                    id: format!("{}::CONTAINS::{}", file_path, id),
                    source: file_path.to_string(),
                    target: id.clone(),
                    label: "CONTAINS".to_string(),
                    properties: HashMap::new(),
                };
                edge.save(graph)?;

                // If it's a Function/Method and its name contains "::", it's an impl method, so link it to its Struct!
                if label == "Function" || label == "Method" {
                    if let Some((struct_part, _method_part)) = node_name.rsplit_once("::") {
                        let struct_id = format!("{}::{}", file_path, struct_part);
                        let method_edge = crate::models::Edge {
                            id: format!("{}::HAS_METHOD::{}", struct_id, id),
                            source: struct_id,
                            target: id,
                            label: "HAS_METHOD".to_string(),
                            properties: HashMap::new(),
                        };
                        method_edge.save(graph)?;
                    }
                }
            }
        }
        Ok(())
    };

    process_all()?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_twice() {
        let db_path = "test_parse_twice.db";
        let _ = std::fs::remove_file(db_path);
        let graph = Graph::open(db_path).unwrap();

        let ruby_file = "/Users/cristian/Projects/dgapp_bkp/app/controllers/api/v2/webhooks_controller.rb";

        // First parse: nodes don't exist
        let res1 = parse_file(ruby_file, &graph);
        assert!(res1.is_ok(), "First parse failed: {:?}", res1.err());

        // Second parse: nodes already exist, properties will be updated
        let res2 = parse_file(ruby_file, &graph);
        assert!(res2.is_ok(), "Second parse failed: {:?}", res2.err());

        let _ = std::fs::remove_file(db_path);
    }

}

