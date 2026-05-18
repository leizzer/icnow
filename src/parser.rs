use anyhow::Result;
use graphqlite::Graph;
use std::collections::HashMap;
use std::fs;
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

pub fn parse_file(file_path: &str, graph: &Graph) -> Result<()> {
    let source_code = fs::read_to_string(file_path)?;
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    parser.set_language(&language)?;

    let tree = parser.parse(&source_code, None).ok_or_else(|| anyhow::anyhow!("Failed to parse code"))?;
    let root_node = tree.root_node();

    // Standard Query for extracting functions, structs, and imports in Rust
    let query_str = r#"
        (function_item name: (identifier) @name) @function.node
        (struct_item name: (type_identifier) @name) @struct.node
        (use_declaration) @import.node
    "#;

    let query = Query::new(&language, query_str)
        .map_err(|e| anyhow::anyhow!("Invalid query: {:?}", e))?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, root_node, source_code.as_bytes());

    let mut file_props = HashMap::new();
    file_props.insert("name".to_string(), file_path.to_string());
    
    let file_node = crate::models::Node {
        id: file_path.to_string(),
        label: "File".to_string(),
        kind: "file".to_string(),
        properties: file_props,
    };
    file_node.save(graph)?;

    while let Some(m) = matches.next() {
        let mut node_name = String::new();
        let mut kind = String::new();
        let mut label = String::new();

        // Iterate through the captures in this match
        for capture in m.captures {
            let capture_name = &query.capture_names()[capture.index as usize];
            
            if *capture_name == "name" {
                node_name = capture.node.utf8_text(source_code.as_bytes())?.to_string();
                
                // Traversal: if this is a function, check if it's inside an impl block
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
            } else if *capture_name == "function.node" {
                kind = "function_item".to_string();
                label = "Function".to_string();
            } else if *capture_name == "struct.node" {
                kind = "struct_item".to_string();
                label = "Struct".to_string();
            } else if *capture_name == "import.node" {
                kind = "use_declaration".to_string();
                label = "Import".to_string();
                node_name = capture.node.utf8_text(source_code.as_bytes())?.to_string();
            }
        }

        if !node_name.is_empty() && !label.is_empty() {
            let mut props = HashMap::new();
            props.insert("name".to_string(), node_name.clone());
            props.insert("file".to_string(), file_path.to_string());
            
            let id = format!("{}::{}", file_path, node_name);
            
            let node = crate::models::Node {
                id: id.clone(),
                label,
                kind,
                properties: props,
            };
            
            node.save(graph)?;

            // Create structural edge between File and Content Node
            let edge = crate::models::Edge {
                id: format!("{}::CONTAINS::{}", file_path, id),
                source: file_path.to_string(),
                target: id.clone(),
                label: "CONTAINS".to_string(),
                properties: HashMap::new(),
            };
            edge.save(graph)?;

            // If it's a Function and its name contains "::", it's an impl method, so link it to its Struct!
            if node.label == "Function" {
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

    Ok(())
}
