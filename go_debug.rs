use tree_sitter::{Parser, Query, QueryCursor};

fn main() {
    let code = "func (u *User) GetName() string { return u.Name }";
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_go::LANGUAGE.into()).unwrap();
    let tree = parser.parse(code, None).unwrap();
    println!("{}", tree.root_node().to_sexp());
}
