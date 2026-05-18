use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Node {
    #[schemars(description = "A globally unique identifier. Use the format 'path/to/file.rs::function_name' or 'path/to/file.rs'")]
    pub id: String,
    #[schemars(description = "The type of node, e.g., 'Function', 'Struct', 'File', 'Model'")]
    pub label: String, 
    #[schemars(description = "The specific syntax or domain kind, e.g., 'function_item', 'Controller'")]
    pub kind: String, 
    #[schemars(description = "Key-value properties to store on the node (e.g., 'name', 'file', value examples)")]
    pub properties: HashMap<String, String>,
}

impl Node {
    pub fn save(&self, graph: &graphqlite::Graph) -> anyhow::Result<()> {
        let mut props = self.properties.clone();
        props.insert("kind".to_string(), self.kind.clone());
        
        graph.upsert_node(&self.id, &props, &self.label)
             .map_err(|e| anyhow::anyhow!("Failed to save node: {}", e))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Edge {
    #[schemars(description = "A globally unique identifier for this specific edge")]
    pub id: String,
    #[schemars(description = "The string ID of the source node (e.g., 'src/main.rs::main')")]
    pub source: String,
    #[schemars(description = "The string ID of the target node (e.g., 'src/models.rs::Node')")]
    pub target: String,
    #[schemars(description = "The relationship type, e.g., 'CALLS', 'IMPORTS', 'REFERENCES'")]
    pub label: String,
    #[schemars(description = "Any additional properties to store on the edge")]
    pub properties: HashMap<String, String>,
}

impl Edge {
    pub fn save(&self, graph: &graphqlite::Graph) -> anyhow::Result<()> {
        graph.upsert_edge(&self.source, &self.target, &self.properties, &self.label)
             .map_err(|e| anyhow::anyhow!("Failed to save edge: {}", e))
    }
}
