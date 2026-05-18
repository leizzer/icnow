use graphqlite::Graph;
use std::collections::HashMap;

fn main() {
    // We will create a fresh database for main.rs
    let db_path = "main_schema.db";
    // If it exists, delete it first to ensure a clean graph
    let _ = std::fs::remove_file(db_path);
    
    let graph = Graph::open(db_path).unwrap();
    icnow::parser::parse_file("src/main.rs", &graph).unwrap();
    println!("Parsed src/main.rs successfully into {}!", db_path);
}
