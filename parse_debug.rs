use tree_sitter::{Parser, Query, QueryCursor};
fn main() {
    let code = "class User(models.Model):\n    name = models.CharField(max_length=100)";
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_python::LANGUAGE.into()).unwrap();
    let tree = parser.parse(code, None).unwrap();
    
    let query_str = r#"
        (class_definition name: (identifier) @name) @struct.node
    "#;
    let query = Query::new(&tree_sitter_python::LANGUAGE.into(), query_str).unwrap();
    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, tree.root_node(), code.as_bytes());
    
    for m in matches {
        for capture in m.captures {
            let name = query.capture_names()[capture.index as usize];
            println!("{}: {:?}", name, capture.node.utf8_text(code.as_bytes()).unwrap());
        }
    }
}
