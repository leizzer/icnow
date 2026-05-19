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

pub fn parse_file(file_path: &str, graph: &Graph) -> Result<FileSummary> {
    let source_code = fs::read_to_string(file_path)?;
    let mut parser = Parser::new();
    
    let (language, query_str) = if file_path.ends_with(".rs") {
        (tree_sitter_rust::LANGUAGE.into(), r#"
            (function_item name: (identifier) @name) @function.node
            (struct_item name: (type_identifier) @name) @struct.node
            (use_declaration) @import.node
        "#)
    } else if file_path.ends_with(".rb") {
        (tree_sitter_ruby::LANGUAGE.into(), r#"
            (method name: (identifier) @name) @function.node
            (class name: (constant) @name) @struct.node
            (module name: (constant) @name) @struct.node
            (call method: (identifier) @import.method arguments: (argument_list (string (string_content) @name))) @import.node
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
    file_node.save(graph)?;

    let mut summary = FileSummary {
        file_path: file_path.to_string(),
        ..Default::default()
    };

    while let Some(m) = matches.next() {
        let mut node_name = String::new();
        let mut kind = String::new();
        let mut label = String::new();
        let mut node_code = String::new();
        let mut is_valid_import = true;

        // Iterate through the captures in this match
        for capture in m.captures {
            let capture_name = &query.capture_names()[capture.index as usize];
            
            if *capture_name == "name" {
                node_name = capture.node.utf8_text(source_code.as_bytes())?.to_string();
                
                // Traversal: if this is a function, check if it's inside an impl block
                if file_path.ends_with(".rs") {
                    if capture.node.parent().map(|p| p.kind()) == Some("function_item") {
                        let func_node = capture.node.parent().unwrap();
                        if let Some(dl) = func_node.parent() {
                            if dl.kind() == "declaration_list" {
                                if let Some(impl_item) = dl.parent() {
                                    if impl_item.kind() == "impl_item" {
                                        if let Some(type_node) = impl_item.child_by_field_name("type") {
                                            let struct_name = type_node.utf8_text(source_code.as_bytes())?.to_string();
                                            node_name = format!("{}::{}", struct_name, node_name);
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if file_path.ends_with(".rb") {
                    if capture.node.parent().map(|p| p.kind()) == Some("method") {
                        let func_node = capture.node.parent().unwrap();
                        if let Some(bs) = func_node.parent() {
                            if bs.kind() == "body_statement" {
                                if let Some(class_item) = bs.parent() {
                                    if class_item.kind() == "class" || class_item.kind() == "module" {
                                        if let Some(name_node) = class_item.child_by_field_name("name") {
                                            let struct_name = name_node.utf8_text(source_code.as_bytes())?.to_string();
                                            node_name = format!("{}::{}", struct_name, node_name);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else if *capture_name == "function.node" {
                kind = capture.node.kind().to_string();
                label = if file_path.ends_with(".rb") { "Method".to_string() } else { "Function".to_string() };
                node_code = capture.node.utf8_text(source_code.as_bytes())?.to_string();
            } else if *capture_name == "struct.node" {
                kind = capture.node.kind().to_string();
                label = if kind == "class" { "Class".to_string() } 
                        else if kind == "module" { "Module".to_string() } 
                        else { "Struct".to_string() };
                node_code = capture.node.utf8_text(source_code.as_bytes())?.to_string();
            } else if *capture_name == "import.node" {
                kind = "use_declaration".to_string();
                label = "Import".to_string();
                if file_path.ends_with(".rs") {
                    node_name = capture.node.utf8_text(source_code.as_bytes())?.to_string();
                }
                node_code = capture.node.utf8_text(source_code.as_bytes())?.to_string();
            } else if *capture_name == "import.method" {
                let method_name = capture.node.utf8_text(source_code.as_bytes())?.to_string();
                if method_name != "require" && method_name != "include" {
                    is_valid_import = false;
                }
            }
        }
        
        if label == "Import" && !is_valid_import {
            label.clear();
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
            } else if label == "Class" || label == "Module" || label == "Struct" {
                summary.structures.entry(label.clone()).or_insert_with(Vec::new).push(node_name.clone());
            } else if label == "Method" || label == "Function" {
                if let Some((struct_part, method_part)) = node_name.split_once("::") {
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
                if let Some((struct_part, _method_part)) = node_name.split_once("::") {
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

    Ok(summary)
}
