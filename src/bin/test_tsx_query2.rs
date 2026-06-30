use tree_sitter::{Parser, Query, QueryCursor, Node};

fn extract_identifiers(node: Node, source_code: &[u8], kinds: &[&str]) -> Vec<String> {
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

fn main() {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_typescript::LANGUAGE_TSX.into()).unwrap();

    let source_code = r#"
        import React, { useState, useEffect } from 'react';
        export const MyComponent = () => {};
        export function MyFunc() {}
        export { useState };
    "#;

    let tree = parser.parse(source_code, None).unwrap();
    
    let query_str = r#"
        (export_statement) @export.node
        (import_statement source: (string (string_fragment) @import.source)) @import.node
    "#;

    let query = Query::new(&tree_sitter_typescript::LANGUAGE_TSX.into(), query_str).unwrap();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source_code.as_bytes());

    use tree_sitter::StreamingIterator;
    while let Some(m) = matches.next() {
        for c in m.captures {
            let name = query.capture_names()[c.index as usize];
            if name == "import.node" {
                let symbols = extract_identifiers(c.node, source_code.as_bytes(), &["identifier"]);
                println!("Import Node -> Symbols: {:?}", symbols);
            } else if name == "export.node" {
                let symbols = extract_identifiers(c.node, source_code.as_bytes(), &["identifier", "type_identifier"]);
                println!("Export Node -> Symbols: {:?}", symbols);
            }
        }
    }
}
