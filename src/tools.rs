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
        
        node.save(&graph).map_err(|e| e.to_string())?;
        
        Ok(format!("Node {} saved successfully.", node.id))
    }

    #[tool(description = "Saves an edge between two nodes into the graphqlite graph database")]
    fn save_edge(&self, Parameters(edge): Parameters<Edge>) -> Result<String, String> {
        let graph = Graph::open(&self.db_path).map_err(|e| format!("Failed to open DB: {}", e))?;
        
        edge.save(&graph).map_err(|e| e.to_string())?;
        
        Ok(format!("Edge {} saved successfully.", edge.id))
    }

    #[tool(description = "Parses a Rust file using tree-sitter and saves its structural nodes (functions, structs) to the graph")]
    fn parse_project_file(&self, Parameters(req): Parameters<ParseFileRequest>) -> Result<String, String> {
        let graph = Graph::open(&self.db_path).map_err(|e| format!("Failed to open DB: {}", e))?;
        crate::parser::parse_file(&req.file_path, &graph).map_err(|e| format!("Parse error: {}", e))?;
        Ok(format!("Successfully parsed {} and added nodes to graph.", req.file_path))
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ParseFileRequest {
    #[schemars(description = "The path to the Rust file to parse")]
    pub file_path: String,
}
