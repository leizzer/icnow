use anyhow::Result;
// Removed graphqlite
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

struct ParsedNode {
    name: String,
    kind: String,
    label: String,
    start_line: usize,
    end_line: usize,
    signature: String,
    docstring: String,
}

fn extract_docstring(node: tree_sitter::Node, source_code: &[u8]) -> String {
    let mut docstring = Vec::new();
    let mut current = node.prev_named_sibling();
    
    while let Some(sibling) = current {
        let kind = sibling.kind();
        if kind == "comment" || kind == "line_comment" || kind == "block_comment" {
            if let Ok(text) = sibling.utf8_text(source_code) {
                docstring.push(text.trim().to_string());
            }
            current = sibling.prev_named_sibling();
        } else {
            break;
        }
    }
    docstring.reverse();
    docstring.join("\n")
}

fn extract_identifiers(node: tree_sitter::Node, source_code: &[u8], kinds: &[&str]) -> Vec<String> {
    let mut results = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if kinds.contains(&child.kind()) {
            if let Ok(text) = child.utf8_text(source_code) {
                results.push(text.to_string());
            }
        }
        results.extend(extract_identifiers(child, source_code, kinds));
    }
    results
}

fn extract_signature(code: &str, file_path: &str) -> String {
    if file_path.ends_with(".rb") {
        code.lines().next().unwrap_or("").trim().to_string()
    } else {
        if let Some(idx) = code.find('{') {
            let sig = &code[..idx];
            sig.split_whitespace().collect::<Vec<&str>>().join(" ")
        } else {
            code.lines().next().unwrap_or("").trim().to_string()
        }
    }
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
        if kind == "class_declaration"
            || kind == "interface_declaration"
            || kind == "internal_module"
        {
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

fn get_language_and_query(file_path: &str) -> Result<(tree_sitter::Language, &'static str)> {
    if file_path.ends_with(".rs") {
        Ok((
            tree_sitter_rust::LANGUAGE.into(),
            r#"
            (function_item name: (identifier) @name) @function.node
            (struct_item name: (type_identifier) @name) @struct.node
            (use_declaration) @import.node
            (call_expression function: _ @call.func) @call.node
            "#,
        ))
    } else if file_path.ends_with(".rb") {
        Ok((
            tree_sitter_ruby::LANGUAGE.into(),
            r#"
            (method name: _ @name) @function.node
            (singleton_method name: _ @name) @function.node
            (class name: _ @name) @struct.node
            (module name: _ @name) @struct.node
            (call method: (identifier) @macro_name arguments: (argument_list (_) @name) (#match? @macro_name "^(scope|has_many|belongs_to|has_one|validates|enum|attr_accessor|attr_reader|attr_writer)$")) @function.node
            (call method: (identifier) @import.method arguments: (argument_list (string (string_content) @name))) @import.node
            (call receiver: _ @call.receiver method: [(identifier) (constant)] @call.func) @call.node
            (call method: [(identifier) (constant)] @call.func) @call.node
            "#,
        ))
    } else if file_path.ends_with(".ts") || file_path.ends_with(".tsx") || file_path.ends_with(".js") || file_path.ends_with(".jsx") {
        let lang = tree_sitter_typescript::LANGUAGE_TSX.into();
        Ok((
            lang,
            r#"
            (export_statement) @export.node
            
            (lexical_declaration (variable_declarator name: (identifier) @name value: (arrow_function))) @function.node
            (function_declaration name: (identifier) @name) @function.node
            (method_definition name: (property_identifier) @name) @function.node
            (class_declaration name: (type_identifier) @name) @struct.node
            (interface_declaration name: (type_identifier) @name) @struct.node
            (type_alias_declaration name: (type_identifier) @name) @struct.node
            (internal_module name: (identifier) @name) @struct.node
            
            (import_statement source: (string (string_fragment) @import.source)) @import.node
            (call_expression function: _ @call.func) @call.node
            "#,
        ))
    } else {
        Err(anyhow::anyhow!("Unsupported file extension: {file_path}"))
    }
}

fn process_function_node(
    func_node: tree_sitter::Node,
    capture_map: &HashMap<&str, tree_sitter::Node>,
    file_path: &str,
    source_code: &[u8],
) -> Result<Vec<ParsedNode>> {
    let mut name = capture_map
        .get("name")
        .and_then(|n| n.utf8_text(source_code).ok())
        .unwrap_or("")
        .to_string();

    let kind = func_node.kind().to_string();
    let label;

    if file_path.ends_with(".rs") {
        label = "Function".to_string();
        if let Some(impl_item) = func_node.parent().and_then(|p| p.parent()) {
            if impl_item.kind() == "impl_item" {
                if let Some(type_node) = impl_item.child_by_field_name("type") {
                    if let Ok(struct_name) = type_node.utf8_text(source_code) {
                        name = format!("{struct_name}::{name}");
                    }
                }
            }
        }
    } else if file_path.ends_with(".rb") {
        label = if kind == "call" { "Macro".to_string() } else { "Method".to_string() };
        let ns = get_ruby_namespace(func_node, source_code)?;
        let mut method_name = func_node
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(source_code).ok())
            .unwrap_or("")
            .to_string();
        if method_name.is_empty() {
            method_name = name.clone();
            // Prefix macros so they are readable, e.g. "has_many :users"
            if kind == "call" {
                if let Some(macro_node) = capture_map.get("macro_name") {
                    if let Ok(m_name) = macro_node.utf8_text(source_code) {
                        method_name = format!("{} {}", m_name, method_name);
                    }
                }
            }
        }
        name = if ns.is_empty() {
            method_name
        } else {
            format!("{ns}::{method_name}")
        };
    } else {
        label = if kind == "method_definition" {
            "Method".to_string()
        } else {
            "Function".to_string()
        };
        let ns = get_ts_namespace(func_node, source_code)?;
        let mut func_name = func_node
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(source_code).ok())
            .unwrap_or("")
            .to_string();
        if func_name.is_empty() {
            func_name = name.clone();
        }
        name = if ns.is_empty() {
            func_name
        } else {
            format!("{ns}::{func_name}")
        };
    }

    let code = func_node.utf8_text(source_code)?.to_string();
    let signature = extract_signature(&code, file_path);
    let docstring = extract_docstring(func_node, source_code);

    let start_line = func_node.start_position().row + 1;
    let end_line = func_node.end_position().row + 1;

    Ok(vec![ParsedNode {
        name,
        kind,
        label,
        start_line,
        end_line,
        signature,
        docstring,
    }])
}

fn process_struct_node(
    struct_node: tree_sitter::Node,
    capture_map: &HashMap<&str, tree_sitter::Node>,
    file_path: &str,
    source_code: &[u8],
) -> Result<Vec<ParsedNode>> {
    let mut name = capture_map
        .get("name")
        .and_then(|n| n.utf8_text(source_code).ok())
        .unwrap_or("")
        .to_string();

    let kind = struct_node.kind().to_string();
    let label;

    if file_path.ends_with(".rb") {
        label = if kind == "class" {
            "Class".to_string()
        } else {
            "Module".to_string()
        };
        name = get_ruby_namespace(struct_node, source_code)?;
    } else if file_path.ends_with(".rs") {
        label = "Struct".to_string();
    } else {
        label = if kind == "class_declaration" {
            "Class".to_string()
        } else if kind == "interface_declaration" {
            "Interface".to_string()
        } else {
            "Module".to_string() // internal_module
        };
        name = get_ts_namespace(struct_node, source_code)?;
    }

    let code = struct_node.utf8_text(source_code)?.to_string();
    let signature = extract_signature(&code, file_path);
    let docstring = extract_docstring(struct_node, source_code);

    let start_line = struct_node.start_position().row + 1;
    let end_line = struct_node.end_position().row + 1;

    Ok(vec![ParsedNode {
        name,
        kind,
        label,
        start_line,
        end_line,
        signature,
        docstring,
    }])
}

fn process_import_node(
    import_node: tree_sitter::Node,
    capture_map: &HashMap<&str, tree_sitter::Node>,
    file_path: &str,
    source_code: &[u8],
) -> Result<Vec<ParsedNode>> {
    let kind = "use_declaration".to_string();
    let mut label = "Import".to_string();
    let mut name = String::new();

    if file_path.ends_with(".rs") {
        name = import_node.utf8_text(source_code)?.to_string();
    } else if file_path.ends_with(".rb") {
        let mut is_valid_import = true;
        if let Some(&method_node) = capture_map.get("import.method") {
            let method_name = method_node.utf8_text(source_code)?.to_string();
            if method_name != "require" && method_name != "include" {
                is_valid_import = false;
            }
        }
        if is_valid_import {
            name = capture_map
                .get("name")
                .and_then(|n| n.utf8_text(source_code).ok())
                .unwrap_or("")
                .to_string();
        } else {
            label.clear();
        }
    } else {
        name = capture_map
            .get("name")
            .and_then(|n| n.utf8_text(source_code).ok())
            .unwrap_or("")
            .to_string();
    }

    let mut nodes = Vec::new();
    let start_line = import_node.start_position().row + 1;
    let end_line = import_node.end_position().row + 1;

    if file_path.ends_with(".ts") || file_path.ends_with(".tsx") || file_path.ends_with(".js") || file_path.ends_with(".jsx") {
        let symbols = extract_identifiers(import_node, source_code, &["identifier"]);
        let source_text = capture_map
            .get("import.source")
            .and_then(|n| n.utf8_text(source_code).ok())
            .unwrap_or("")
            .to_string();
        
        for sym in symbols {
            // Include the source in the name, e.g. "useState from 'react'" so we can reconcile it later
            let name = format!("{} FROM '{}'", sym, source_text);
            nodes.push(ParsedNode {
                name,
                kind: kind.clone(),
                label: label.clone(),
                start_line,
                end_line,
                signature: String::new(),
                docstring: String::new(),
            });
        }
    } else {
        nodes.push(ParsedNode {
            name,
            kind,
            label,
            start_line,
            end_line,
            signature: String::new(),
            docstring: String::new(),
        });
    }

    Ok(nodes)
}

fn process_export_node(
    export_node: tree_sitter::Node,
    source_code: &[u8],
) -> Result<Vec<ParsedNode>> {
    let mut nodes = Vec::new();
    let symbols = extract_identifiers(export_node, source_code, &["identifier", "type_identifier"]);
    
    let start_line = export_node.start_position().row + 1;
    let end_line = export_node.end_position().row + 1;
    
    for sym in symbols {
        nodes.push(ParsedNode {
            name: sym,
            kind: "export_statement".to_string(),
            label: "Export".to_string(),
            start_line,
            end_line,
            signature: String::new(),
            docstring: String::new(),
        });
    }
    
    Ok(nodes)
}

fn process_call_node(
    call_node: tree_sitter::Node,
    capture_map: &HashMap<&str, tree_sitter::Node>,
    file_path: &str,
    source_code: &[u8],
    bulk_nodes: &mut Vec<(String, HashMap<String, String>, String)>,
    bulk_edges: &mut Vec<(String, String, HashMap<String, String>, String)>,
) -> Result<()> {
    let func_text = capture_map
        .get("call.func")
        .and_then(|n| n.utf8_text(source_code).ok())
        .unwrap_or("")
        .to_string();
    let receiver_text = capture_map
        .get("call.receiver")
        .and_then(|n| n.utf8_text(source_code).ok())
        .unwrap_or("")
        .to_string();

    let target_name = if !receiver_text.is_empty() && !func_text.is_empty() {
        format!("{receiver_text}.{func_text}")
    } else {
        func_text
    };

    if target_name.is_empty() {
        return Ok(());
    }

    let mut curr = Some(call_node);
    let mut enclosing_func_name = String::new();

    while let Some(n) = curr {
        let k = n.kind();
        if k == "function_item"
            || k == "method"
            || k == "singleton_method"
            || k == "function_declaration"
            || k == "method_definition"
        {
            if let Some(name_node) = n.child_by_field_name("name") {
                if let Ok(text) = name_node.utf8_text(source_code) {
                    enclosing_func_name = text.to_string();
                    if file_path.ends_with(".rs") {
                        if let Some(impl_item) = n.parent().and_then(|p| p.parent()) {
                            if impl_item.kind() == "impl_item" {
                                if let Some(type_node) = impl_item.child_by_field_name("type") {
                                    if let Ok(struct_name) = type_node.utf8_text(source_code) {
                                        enclosing_func_name =
                                            format!("{struct_name}::{enclosing_func_name}");
                                    }
                                }
                            }
                        }
                    } else if file_path.ends_with(".rb") {
                        if let Ok(ns) = get_ruby_namespace(n, source_code) {
                            if !ns.is_empty() {
                                enclosing_func_name = format!("{ns}::{enclosing_func_name}");
                            }
                        }
                    } else if let Ok(ns) = get_ts_namespace(n, source_code) {
                        if !ns.is_empty() {
                            enclosing_func_name = format!("{ns}::{enclosing_func_name}");
                        }
                    }
                    break;
                }
            }
        }
        curr = n.parent();
    }

    if !enclosing_func_name.is_empty() {
        let source_id = format!("{file_path}::{enclosing_func_name}");
        let line = call_node.start_position().row + 1;
        let target_id = format!("{}::unresolved_call_{}", file_path, line);

        let mut props = HashMap::new();
        props.insert("name".to_string(), target_name.clone());
        props.insert("kind".to_string(), "unresolved_symbol".to_string());
        props.insert("file".to_string(), file_path.to_string());
        props.insert("line".to_string(), line.to_string());
        bulk_nodes.push((target_id.clone(), props, "Unresolved".to_string()));

        bulk_edges.push((source_id, target_id, HashMap::new(), "CALLS".to_string()));
    }

    Ok(())
}

fn get_file_metadata_properties(file_path: &str) -> HashMap<String, String> {
    let mut file_props = HashMap::new();
    file_props.insert("name".to_string(), file_path.to_string());
    file_props.insert("kind".to_string(), "file".to_string());
    if let Ok(metadata) = fs::metadata(file_path) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                file_props.insert("last_modified".to_string(), duration.as_secs().to_string());
            }
        }
    }
    file_props
}

pub fn parse_file(file_path: &str, conn: &lbug::Connection) -> Result<FileSummary> {
    let (summary, bulk_nodes, bulk_edges) = parse_file_in_memory(file_path)?;

    let mut prep_file = conn.prepare("MERGE (n:File {id: $id}) ON CREATE SET n.name=$name, n.kind=$kind, n.last_modified=$last_modified ON MATCH SET n.name=$name, n.kind=$kind, n.last_modified=$last_modified").map_err(|e| anyhow::anyhow!("Prepare File failed: {}", e))?;
    let mut prep_symbol = conn.prepare("MERGE (n:Symbol {id: $id}) ON CREATE SET n.name=$name, n.signature=$signature, n.docstring=$docstring, n.kind=$kind, n.start_line=$start_line, n.end_line=$end_line, n.file=$file, n.line=$line ON MATCH SET n.name=$name, n.signature=$signature, n.docstring=$docstring, n.kind=$kind, n.start_line=$start_line, n.end_line=$end_line, n.file=$file, n.line=$line").map_err(|e| anyhow::anyhow!("Prepare Symbol failed: {}", e))?;

    for (id, props, label) in bulk_nodes {
        if label == "File" {
            let name = props.get("name").cloned().unwrap_or_default();
            let kind = props.get("kind").cloned().unwrap_or_else(|| "file".to_string());
            let last_modified = props.get("last_modified").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
            
            conn.execute(&mut prep_file, vec![
                ("id", lbug::Value::String(id)),
                ("name", lbug::Value::String(name)),
                ("kind", lbug::Value::String(kind)),
                ("last_modified", lbug::Value::Int64(last_modified)),
            ]).map_err(|e| anyhow::anyhow!("Merge File failed: {}", e))?;
        } else {
            let name = props.get("name").cloned().unwrap_or_default();
            let signature = props.get("signature").cloned().unwrap_or_default();
            let docstring = props.get("docstring").cloned().unwrap_or_default();
            let kind = label.clone();
            let start_line = props.get("start_line").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
            let end_line = props.get("end_line").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
            let file = props.get("file").cloned().unwrap_or_default();
            let line = props.get("line").cloned().unwrap_or_default();

            conn.execute(&mut prep_symbol, vec![
                ("id", lbug::Value::String(id)),
                ("name", lbug::Value::String(name)),
                ("signature", lbug::Value::String(signature)),
                ("docstring", lbug::Value::String(docstring)),
                ("kind", lbug::Value::String(kind)),
                ("start_line", lbug::Value::Int64(start_line)),
                ("end_line", lbug::Value::Int64(end_line)),
                ("file", lbug::Value::String(file)),
                ("line", lbug::Value::String(line)),
            ]).map_err(|e| anyhow::anyhow!("Merge Symbol failed: {}", e))?;
        }
    }

    let mut edge_preps = std::collections::HashMap::new();

    for (source, target, _props, label) in bulk_edges {
        let src_table = if source.starts_with('/') && !source.contains("::") { "File" } else { "Symbol" };
        let tgt_table = if target.starts_with('/') && !target.contains("::") { "File" } else { "Symbol" };
        
        let rel_table = match label.as_str() {
            "REL_CONTAINS" | "CONTAINS" => "REL_CONTAINS",
            "DEFINES" => "DEFINES",
            "CALLS" | _ => "CALLS",
        };

        let query = format!("MATCH (s:{} {{id: $src}}), (t:{} {{id: $tgt}}) MERGE (s)-[:{}]->(t)", src_table, tgt_table, rel_table);
        
        if !edge_preps.contains_key(&query) {
            let prep = conn.prepare(&query).map_err(|e| anyhow::anyhow!("Prepare Edge failed: {}", e))?;
            edge_preps.insert(query.clone(), prep);
        }
        
        let prep = edge_preps.get_mut(&query).unwrap();
        conn.execute(prep, vec![
            ("src", lbug::Value::String(source)),
            ("tgt", lbug::Value::String(target)),
        ]).map_err(|e| anyhow::anyhow!("Merge Edge failed: {}", e))?;
    }

    Ok(summary)
}

#[allow(clippy::type_complexity)]
pub fn parse_file_in_memory(
    file_path: &str,
) -> Result<(
    FileSummary,
    Vec<(String, HashMap<String, String>, String)>,
    Vec<(String, String, HashMap<String, String>, String)>,
)> {
    let source_code = fs::read_to_string(file_path)?;
    let mut parser = Parser::new();

    let (language, query_str) = get_language_and_query(file_path)?;
    parser.set_language(&language)?;

    let tree = parser
        .parse(&source_code, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse code"))?;
    let root_node = tree.root_node();

    let query =
        Query::new(&language, query_str).map_err(|e| anyhow::anyhow!("Invalid query: {e:?}"))?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, root_node, source_code.as_bytes());

    let mut summary = FileSummary {
        file_path: file_path.to_string(),
        ..Default::default()
    };

    let mut bulk_nodes: Vec<(String, HashMap<String, String>, String)> = Vec::new();
    let mut bulk_edges: Vec<(String, String, HashMap<String, String>, String)> = Vec::new();

    let file_props = get_file_metadata_properties(file_path);
    bulk_nodes.push((file_path.to_string(), file_props, "File".to_string()));

    while let Some(m) = matches.next() {
        let mut capture_map = HashMap::new();
        for capture in m.captures {
            let name: &str = query.capture_names()[capture.index as usize];
            capture_map.insert(name, capture.node);
        }

        let parsed_nodes = if let Some(&func_node) = capture_map.get("function.node") {
            process_function_node(func_node, &capture_map, file_path, source_code.as_bytes())?
        } else if let Some(&struct_node) = capture_map.get("struct.node") {
            process_struct_node(struct_node, &capture_map, file_path, source_code.as_bytes())?
        } else if let Some(&import_node) = capture_map.get("import.node") {
            process_import_node(import_node, &capture_map, file_path, source_code.as_bytes())?
        } else if let Some(&export_node) = capture_map.get("export.node") {
            process_export_node(export_node, source_code.as_bytes())?
        } else if let Some(&call_node) = capture_map.get("call.node") {
            process_call_node(
                call_node,
                &capture_map,
                file_path,
                source_code.as_bytes(),
                &mut bulk_nodes,
                &mut bulk_edges,
            )?;
            vec![]
        } else {
            vec![]
        };

        for node in parsed_nodes {
            if node.name.is_empty() || node.label.is_empty() {
                continue;
            }

            let mut props = HashMap::new();
            props.insert("name".to_string(), node.name.clone());
            props.insert("file".to_string(), file_path.to_string());
            props.insert("kind".to_string(), node.kind);

            props.insert("start_line".to_string(), node.start_line.to_string());
            props.insert("end_line".to_string(), node.end_line.to_string());
            if !node.signature.is_empty() {
                props.insert("signature".to_string(), node.signature);
            }
            if !node.docstring.is_empty() {
                props.insert("docstring".to_string(), node.docstring);
            }

            let id = format!("{file_path}::{}", node.name);
            bulk_nodes.push((id.clone(), props, node.label.clone()));

            // Populate the FileSummary
            if node.label == "Import" {
                summary.imports.push(node.name.clone());
            } else if node.label == "Class"
                || node.label == "Module"
                || node.label == "Struct"
                || node.label == "Interface"
            {
                summary
                    .structures
                    .entry(node.label.clone())
                    .or_default()
                    .push(node.name.clone());
            } else if node.label == "Method" || node.label == "Function" || node.label == "Macro" {
                if let Some((struct_part, method_part)) = node.name.rsplit_once("::") {
                    summary
                        .methods
                        .entry(struct_part.to_string())
                        .or_default()
                        .push((node.label.clone(), method_part.to_string()));
                } else {
                    summary
                        .standalone_functions
                        .entry(node.label.clone())
                        .or_default()
                        .push(node.name.clone());
                }
            }

            // Create structural edge between File and Content Node
            bulk_edges.push((
                file_path.to_string(),
                id.clone(),
                HashMap::new(),
                "CONTAINS".to_string(),
            ));

            // If it's a Function/Method and its name contains "::", it's an impl method, link it to Struct
            if node.label == "Function" || node.label == "Method" {
                if let Some((struct_part, _method_part)) = node.name.rsplit_once("::") {
                    let struct_id = format!("{file_path}::{struct_part}");
                    bulk_edges.push((struct_id, id, HashMap::new(), "DEFINES".to_string()));
                }
            }
        }
    }

    Ok((summary, bulk_nodes, bulk_edges))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_twice() {
        let db_path = "test_parse_twice.db";
        let _ = std::fs::remove_file(db_path);
        let graph = crate::open_db_graph(db_path).unwrap();

        let ruby_file =
            "/Users/cristian/Projects/dgapp_bkp/app/controllers/api/v2/webhooks_controller.rb";

        // First parse: nodes don't exist
        let res1 = parse_file(ruby_file, &graph);
        assert!(res1.is_ok(), "First parse failed: {:?}", res1.err());

        // Second parse: nodes already exist, properties will be updated
        let res2 = parse_file(ruby_file, &graph);
        assert!(res2.is_ok(), "Second parse failed: {:?}", res2.err());

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn test_parse_rust() {
        let db_path = "test_parse_rust.db";
        let _ = std::fs::remove_file(db_path);
        let graph = crate::open_db_graph(db_path).unwrap();

        let rs_file = "src/parser.rs";

        let res = parse_file(rs_file, &graph);
        assert!(res.is_ok(), "Parse rust failed: {:?}", res.err());
        
        let summary = res.unwrap();
        assert!(!summary.imports.is_empty(), "Expected some imports in parser.rs");
        assert!(!summary.standalone_functions.is_empty(), "Expected some functions in parser.rs");

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn test_parse_user_rb() {
        let db_path = "test_parse_user_rb.db";
        let _ = std::fs::remove_file(db_path);
        let graph = crate::open_db_graph(db_path).unwrap();
        let ruby_file = "/Users/cristian/Projects/dgapp_bkp/app/models/user.rb";
        let res = parse_file(ruby_file, &graph).unwrap();
        println!("Structures: {:?}", res.structures);
        println!("Standalone: {:?}", res.standalone_functions);
        println!("Methods found keys: {:?}", res.methods.keys());
        if let Some(methods) = res.methods.get("User") {
            println!("User Methods ({}): {:?}", methods.len(), methods);
        } else {
            println!("No methods found for User class");
        }
        let _ = std::fs::remove_file(db_path);
    }
}

