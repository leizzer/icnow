use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};
use std::fs;

fn main() {
    let source_code = fs::read_to_string("src/models.rs").unwrap();
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(&source_code, None).unwrap();

    let query_str = r#"
        (function_item name: (identifier) @name) @function.node
        (struct_item name: (type_identifier) @name) @struct.node
        (use_declaration) @import.node
        (impl_item
            type: (type_identifier) @impl.struct_name
            body: (declaration_list
                (function_item name: (identifier) @impl.method_name)
            )
        )
    "#;
    let query = Query::new(&language, query_str).unwrap();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source_code.as_bytes());

    let mut match_count = 0;
    while let Some(m) = matches.next() {
        match_count += 1;
        println!("--- Match {} ---", match_count);
        for capture in m.captures {
            let name = &query.capture_names()[capture.index as usize];
            let text = capture.node.utf8_text(source_code.as_bytes()).unwrap();
            println!("  @{}: {}", name, text);
        }
    }
}
