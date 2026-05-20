use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use graphqlite::Graph;
use std::collections::HashMap;

use crate::models::{Node, Edge};

/// This struct is our service definition. It's a simple, clonable struct.
#[derive(Debug, Clone)]
pub struct GraphService {
    pub db_path: String,
}

#[tool_router(server_handler)]
impl GraphService {
    #[tool(description = "Saves a new node (file, function, class, module, etc.) into the graph. Use this tool manually only if the static parser missed a specific node or when explicitly registering domain-level concepts like Rails Controllers/Models and their fields.")]
    fn save_node(&self, Parameters(node): Parameters<Node>) -> Result<String, String> {
        let graph = Graph::open(&self.db_path).map_err(|e| format!("Failed to open DB: {}", e))?;
        
        node.save(&graph).map_err(|e| e.to_string())?;
        
        Ok(format!("Node {} saved successfully.", node.id))
    }

    #[tool(description = "Creates or updates a directed edge between two existing nodes (e.g. connecting a caller function to a callee method, or mapping database entity relationships). Use this tool to explicitly link imports to their physical file targets, functions to their internal calls, or class inheritance/mixins.")]
    fn save_edge(&self, Parameters(edge): Parameters<Edge>) -> Result<String, String> {
        let graph = Graph::open(&self.db_path).map_err(|e| format!("Failed to open DB: {}", e))?;
        
        edge.save(&graph).map_err(|e| e.to_string())?;
        
        Ok(format!("Edge {} saved successfully.", edge.id))
    }

    #[tool(description = "Parses a source file (Rust, Ruby, TypeScript, TSX) using Tree-sitter, extracts all structural nodes (Functions, Methods, Classes, Interfaces, Imports), and automatically adds them and their container relationships to the graph database. Call this tool first to map out the architecture of a new or modified file.")]
    fn parse_project_file(&self, Parameters(req): Parameters<ParseFileRequest>) -> Result<String, String> {
        let graph = Graph::open(&self.db_path).map_err(|e| format!("Failed to open DB: {}", e))?;
        let summary = crate::parser::parse_file(&req.file_path, &graph).map_err(|e| format!("Parse error: {}", e))?;
        
        let mut out = format!("Successfully parsed `{}` and added nodes to graph.\n\n", req.file_path);
        out.push_str("**File Architecture Summary:**\n");
        
        if !summary.imports.is_empty() {
            out.push_str(&format!("- **Imports**: `{}`\n", summary.imports.join("`, `")));
        }
        
        if !summary.structures.is_empty() {
            for (label, names) in &summary.structures {
                let plural_label = if label == "Class" { "Classes".to_string() } else { format!("{}s", label) };
                out.push_str(&format!("- **{}**:\n", plural_label));
                for name in names {
                    out.push_str(&format!("  - `{}`\n", name));
                    if let Some(methods) = summary.methods.get(name) {
                        // Group child methods by their label
                        let mut grouped_methods: HashMap<String, Vec<String>> = HashMap::new();
                        for (m_label, m_name) in methods {
                            grouped_methods.entry(m_label.clone()).or_insert_with(Vec::new).push(m_name.clone());
                        }
                        for (m_label, m_names) in grouped_methods {
                            out.push_str(&format!("    - {}s: `{}`\n", m_label, m_names.join("`, `")));
                        }
                    }
                }
            }
        }
        
        if !summary.standalone_functions.is_empty() {
            for (label, names) in &summary.standalone_functions {
                out.push_str(&format!("- **{}s**: `{}`\n", label, names.join("`, `")));
            }
        }
        
        Ok(out)
    }

    #[tool(description = "Executes a raw SQL SELECT query against the underlying SQLite database tables (nodes, edges, node_props_text) to retrieve metadata, properties, or precise source code fragments. Use this tool when you need to extract the 'source_code' property of a specific function or class node by its ID.")]
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

    #[tool(description = "Recursively walks the graph bidirectionally from a starting node up to a specified depth (max_depth) and returns an indented relationship list. Use this tool when you want to discover the neighborhood of dependencies, callers, or subclasses of a particular node in a single call.")]
    fn traverse_graph(&self, Parameters(req): Parameters<TraverseGraphRequest>) -> Result<String, String> {
        let depth = req.max_depth.unwrap_or(2);
        
        let query = format!(
            "WITH RECURSIVE graph_path(source_int, target_int, label, depth) AS ( \
                SELECT source_id, target_id, type, 1 \
                FROM edges \
                WHERE source_id = ( \
                    SELECT np.node_id FROM node_props_text np \
                    JOIN property_keys pk ON np.key_id = pk.id \
                    WHERE pk.key = 'id' AND np.value = '{node_id}' \
                ) OR target_id = ( \
                    SELECT np.node_id FROM node_props_text np \
                    JOIN property_keys pk ON np.key_id = pk.id \
                    WHERE pk.key = 'id' AND np.value = '{node_id}' \
                ) \
                UNION \
                SELECT e.source_id, e.target_id, e.type, gp.depth + 1 \
                FROM edges e \
                JOIN graph_path gp ON e.source_id = gp.target_int OR e.target_id = gp.source_int OR e.source_id = gp.source_int OR e.target_id = gp.target_int \
                WHERE gp.depth < {depth} \
            ) \
            SELECT DISTINCT \
                (SELECT np.value FROM node_props_text np JOIN property_keys pk ON np.key_id = pk.id WHERE pk.key = 'id' AND np.node_id = gp.source_int) as source, \
                (SELECT np.value FROM node_props_text np JOIN property_keys pk ON np.key_id = pk.id WHERE pk.key = 'id' AND np.node_id = gp.target_int) as target, \
                gp.label, \
                gp.depth \
            FROM graph_path gp ORDER BY gp.depth ASC;",
            node_id = req.node_id,
            depth = depth
        );
        
        let output = std::process::Command::new("sqlite3")
            .arg(&self.db_path)
            .arg("-header")
            .arg("-markdown")
            .arg(&query)
            .output()
            .map_err(|e| format!("Failed to execute traversal: {}", e))?;
            
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Traversal failed: {}", err));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    #[tool(description = "Executes a graph query using Cypher syntax (e.g., MATCH (source)-[rel]->(target) WHERE ...) to discover patterns, links, or cross-file dependencies. This is the preferred tool for high-level semantic lookups and pattern matching in the database.")]
    fn query_graph_cypher(&self, Parameters(req): Parameters<QueryGraphCypherRequest>) -> Result<String, String> {
        let conn = graphqlite::Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open graph database: {}", e))?;
            
        let res = conn.cypher(&req.query)
            .map_err(|e| format!("Cypher query failed: {}", e))?;
            
        let cols = res.columns();
        if cols.is_empty() {
            return Ok("No columns returned.".to_string());
        }
        
        let mut out = format!("| {} |\n", cols.join(" | "));
        out.push_str(&format!("| {} |\n", cols.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")));
        
        for row in &res {
            let mut row_vals = Vec::new();
            for col in cols {
                let val_str = if let Ok(s) = row.get::<String>(col) {
                    s
                } else if let Ok(i) = row.get::<i64>(col) {
                    i.to_string()
                } else if let Ok(f) = row.get::<f64>(col) {
                    f.to_string()
                } else if let Ok(b) = row.get::<bool>(col) {
                    b.to_string()
                } else {
                    "null".to_string()
                };
                row_vals.push(val_str);
            }
            out.push_str(&format!("| {} |\n", row_vals.join(" | ")));
        }
        
        Ok(out)
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ParseFileRequest {
    #[schemars(description = "The absolute or relative path to the Rust (.rs), Ruby (.rb), TypeScript (.ts), or TSX (.tsx) file to parse.")]
    pub file_path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QueryGraphRequest {
    #[schemars(description = "The raw SQL SELECT query to execute against the knowledge.db database (e.g. querying nodes, edges, or node_props_text directly).")]
    pub query: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TraverseGraphRequest {
    #[schemars(description = "The globally unique string ID of the starting node (e.g. 'src/models.rs::Node').")]
    pub node_id: String,
    #[schemars(description = "Maximum depth of recursive hops to traverse. Defaults to 2.")]
    pub max_depth: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QueryGraphCypherRequest {
    #[schemars(description = "The Cypher query string to execute. Example: 'MATCH (c:Class)-[:HAS_METHOD]->(m) RETURN c.id, m.id LIMIT 10'")]
    pub query: String,
}
