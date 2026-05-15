use graphqlite::Graph;
use serde_json::Value;
use std::collections::HashMap;

fn main() {
    let graph = Graph::open("test_schema.db").unwrap();
    let mut props1 = HashMap::new();
    props1.insert("name".to_string(), "Alice".to_string());
    
    let mut props2 = HashMap::new();
    props2.insert("name".to_string(), "Bob".to_string());

    graph.upsert_node("1", &props1, "User").unwrap();
    graph.upsert_node("2", &props2, "User").unwrap();
    
    let edge_props = HashMap::<String, String>::new();
    graph.upsert_edge("1", "2", &edge_props, "KNOWS").unwrap();
}
