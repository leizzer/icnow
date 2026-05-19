use tree_sitter::Parser;

fn main() {
    let source_code = r#"
require 'json'
module Api
  class UserController < ApplicationController
    def index
      puts "hello"
    end
  end
end
"#;
    let mut parser = Parser::new();
    let language = tree_sitter_ruby::LANGUAGE.into();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(&source_code, None).unwrap();
    
    println!("{}", tree.root_node().to_sexp());
}
