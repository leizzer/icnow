use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Node {
    #[schemars(description = "A globally unique identifier. For structural elements, use the format 'path/to/file.ext::ClassName::method_name' or 'path/to/file.ext::function_name'. For files, use the file path itself.")]
    pub id: String,
    #[schemars(description = "The high-level category of the node, e.g., 'File', 'Class', 'Module', 'Struct', 'Interface', 'Function', 'Method', 'Import'")]
    pub label: String, 
    #[schemars(description = "The specific AST syntax kind or code element, e.g., 'function_declaration', 'class_declaration', 'method_definition'")]
    pub kind: String, 
    #[schemars(description = "A key-value map for arbitrary node metadata, such as 'name', 'file', 'source_code', or 'last_modified'")]
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
    #[schemars(description = "A globally unique identifier for this specific relationship edge, e.g., 'source_id::RELATION_NAME::target_id'")]
    pub id: String,
    #[schemars(description = "The globally unique string ID of the source node")]
    pub source: String,
    #[schemars(description = "The globally unique string ID of the target node")]
    pub target: String,
    #[schemars(description = "The relationship label, e.g., 'CONTAINS' (file contains element), 'HAS_METHOD', 'CALLS' (method calls method), 'IMPORTS'")]
    pub label: String,
    #[schemars(description = "Additional properties to store on the edge (e.g., call line number, import aliases)")]
    pub properties: HashMap<String, String>,
}

impl Edge {
    pub fn save(&self, graph: &graphqlite::Graph) -> anyhow::Result<()> {
        graph.upsert_edge(&self.source, &self.target, &self.properties, &self.label)
             .map_err(|e| anyhow::anyhow!("Failed to save edge: {}", e))
    }
}
