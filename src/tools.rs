use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use graphqlite::Graph;

use crate::models::{Node, Edge};

/// This struct is our service definition. It's a simple, clonable struct.
#[derive(Debug, Clone)]
pub struct GraphService {
    pub db_path: String,
}

#[tool_router(server_handler)]
impl GraphService {
    #[tool(description = "Saves a node into the graphqlite graph database")]
    fn save_node(&self, Parameters(node): Parameters<Node>) -> Result<String, String> {
        let graph = Graph::open(&self.db_path).map_err(|e| format!("Failed to open DB: {}", e))?;
        
        graph.upsert_node(
            &node.id.to_string(), 
            &node.properties, 
            &node.label
        ).map_err(|e| format!("Failed to save node: {}", e))?;
        
        Ok(format!("Node {} saved successfully.", node.id))
    }

    #[tool(description = "Saves an edge between two nodes into the graphqlite graph database")]
    fn save_edge(&self, Parameters(edge): Parameters<Edge>) -> Result<String, String> {
        let graph = Graph::open(&self.db_path).map_err(|e| format!("Failed to open DB: {}", e))?;
        
        graph.upsert_edge(
            &edge.source.to_string(),
            &edge.target.to_string(),
            &edge.properties,
            &edge.label
        ).map_err(|e| format!("Failed to save edge: {}", e))?;
        
        Ok(format!("Edge {} saved successfully.", edge.id))
    }
}
