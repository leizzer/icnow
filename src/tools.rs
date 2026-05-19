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
        let summary = crate::parser::parse_file(&req.file_path, &graph).map_err(|e| format!("Parse error: {}", e))?;
        
        let mut out = format!("Successfully parsed `{}` and added nodes to graph.\n\n", req.file_path);
        out.push_str("**File Architecture Summary:**\n");
        
        if !summary.imports.is_empty() {
            out.push_str(&format!("- **Imports**: `{}`\n", summary.imports.join("`, `")));
        }
        
        if !summary.structs.is_empty() {
            out.push_str("- **Structs**:\n");
            for s in &summary.structs {
                out.push_str(&format!("  - `{}`\n", s));
                if let Some(methods) = summary.methods.get(s) {
                    out.push_str(&format!("    - Methods: `{}`\n", methods.join("`, `")));
                }
            }
        }
        
        if !summary.functions.is_empty() {
            out.push_str(&format!("- **Functions**: `{}`\n", summary.functions.join("`, `")));
        }
        
        Ok(out)
    }

    #[tool(description = "Executes an arbitrary SQLite query against the graph database and returns the results in a formatted Markdown table")]
    fn query_graph(&self, Parameters(req): Parameters<QueryGraphRequest>) -> Result<String, String> {
        let output = std::process::Command::new("sqlite3")
            .arg(&self.db_path)
            .arg("-header")
            .arg("-markdown")
            .arg(&req.query)
            .output()
            .map_err(|e| format!("Failed to execute query: {}", e))?;
            
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Query failed: {}", err));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ParseFileRequest {
    #[schemars(description = "The path to the Rust file to parse")]
    pub file_path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QueryGraphRequest {
    #[schemars(description = "The SQLite query to execute against knowledge.db")]
    pub query: String,
}
