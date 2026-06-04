use std::fs;
use tree_sitter::{Parser, StreamingIterator};
use tree_sitter::{Query, QueryCursor};

fn main() {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_ruby::LANGUAGE.into())
        .unwrap();

    let path = "/Users/cristian/Projects/dgapp_bkp/app/models/user.rb";
    let source_code = fs::read_to_string(path).unwrap();
    let tree = parser.parse(&source_code, None).unwrap();

    let root_node = tree.root_node();

    // Check if there are any ERROR nodes in the tree
    fn find_errors(node: tree_sitter::Node, source_code: &str, depth: usize) {
        if node.is_error() || node.is_missing() {
            let start = node.start_byte();
            let end = node.end_byte();
            let line = node.start_position().row + 1;
            println!(
                "Error or Missing node at line {}: {}",
                line,
                &source_code[start..end]
            );
        }
        for i in 0..node.child_count() {
            find_errors(node.child(i as u32).unwrap(), source_code, depth + 1);
        }
    }

    println!("--- AST Errors ---");
    find_errors(root_node, &source_code, 0);
    println!("------------------");

    // Let's run the query
    let query_str = r#"
        (method name: (identifier) @name) @function.node
        (singleton_method name: (identifier) @name) @function.node
    "#;
    let query = Query::new(&tree_sitter_ruby::LANGUAGE.into(), query_str).unwrap();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, root_node, source_code.as_bytes());

    let mut count = 0;
    while let Some(m) = matches.next() {
        count += 1;
        let mut name = "";
        for capture in m.captures {
            let name_idx = query
                .capture_names()
                .iter()
                .position(|n| *n == "name")
                .unwrap();
            if capture.index as usize == name_idx {
                name = capture.node.utf8_text(source_code.as_bytes()).unwrap();
            }
        }
        println!("Match {count}: {name}");
    }

    println!("Total matched methods: {count}");
}
