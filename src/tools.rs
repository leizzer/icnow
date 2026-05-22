use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use graphqlite::Graph;
use std::collections::HashMap;

use crate::models::{Node, Edge};

fn infer_project_root(file_path: &str) -> Option<String> {
    let path = std::path::Path::new(file_path);
    let mut curr = if path.is_file() {
        path.parent()
    } else {
        Some(path)
    };
    
    while let Some(dir) = curr {
        if dir.join(".git").exists() 
            || dir.join("Cargo.toml").exists() 
            || dir.join("Gemfile").exists() 
            || dir.join("package.json").exists() 
        {
            return Some(dir.to_string_lossy().to_string());
        }
        curr = dir.parent();
    }
    None
}

fn infer_from_node_id(node_id: &str) -> Option<String> {
    let file_path = node_id.split("::").next()?;
    infer_project_root(file_path)
}

/// This struct is our service definition. It's a simple, clonable struct.
#[derive(Debug, Clone)]
pub struct GraphService {
    pub db_path: String,
}

impl GraphService {
    fn resolve_db_path_and_watch(&self, project_root: Option<&str>, file_path: Option<&str>, node_id: Option<&str>) -> String {
        let root = project_root
            .map(|r| r.to_string())
            .or_else(|| file_path.and_then(|f| infer_project_root(f)))
            .or_else(|| node_id.and_then(|n| infer_from_node_id(n)));
            
        if let Some(ref r) = root {
            let path_buf = std::path::Path::new(r);
            let db_path = path_buf.join("knowledge.db").to_string_lossy().to_string();
            crate::watcher::ensure_watching(path_buf, &db_path);
            db_path
        } else {
            self.db_path.clone()
        }
    }
}

#[tool_router(server_handler)]
impl GraphService {
    #[tool(description = "Saves a new node (file, function, class, module, etc.) into the graph. Use this tool manually only if the static parser missed a specific node or when explicitly registering domain-level concepts like Rails Controllers/Models and their fields.")]
    fn save_node(&self, Parameters(req): Parameters<SaveNodeRequest>) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), Some(&req.node.id), None);
        let graph = Graph::open(&db_path).map_err(|e| format!("Failed to open DB: {}", e))?;
        
        req.node.save(&graph).map_err(|e| e.to_string())?;
        
        Ok(format!("Node {} saved successfully.", req.node.id))
    }

    #[tool(description = "Creates or updates a directed edge between two existing nodes (e.g. connecting a caller function to a callee method, or mapping database entity relationships). Use this tool to explicitly link imports to their physical file targets, functions to their internal calls, or class inheritance/mixins.")]
    fn save_edge(&self, Parameters(req): Parameters<SaveEdgeRequest>) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), Some(&req.edge.source), None);
        let graph = Graph::open(&db_path).map_err(|e| format!("Failed to open DB: {}", e))?;
        
        req.edge.save(&graph).map_err(|e| e.to_string())?;
        
        Ok(format!("Edge {} saved successfully.", req.edge.id))
    }

    #[tool(description = "Parses a source file (Rust, Ruby, TypeScript, TSX) using Tree-sitter, extracts all structural nodes (Functions, Methods, Classes, Interfaces, Imports), and automatically adds them and their container relationships to the graph database. Call this tool first to map out the architecture of a new or modified file.")]
    fn parse_project_file(&self, Parameters(req): Parameters<ParseFileRequest>) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), Some(&req.file_path), None);
        let graph = Graph::open(&db_path).map_err(|e| format!("Failed to open DB: {}", e))?;
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
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), None, None);
        let output = std::process::Command::new("sqlite3")
            .arg(&db_path)
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
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), None, Some(&req.node_id));
        let depth = req.max_depth.unwrap_or(2);
        let safe_node_id = req.node_id.replace("'", "''");
        
        let query = if depth <= 1 {
            format!(
                "MATCH (n) WHERE n.id = '{id}' MATCH (n)-[r]->(y) RETURN n.id as source, type(r) as label, y.id as target \
                 UNION \
                 MATCH (n) WHERE n.id = '{id}' MATCH (y)-[r]->(n) RETURN y.id as source, type(r) as label, n.id as target",
                id = safe_node_id
            )
        } else {
            let depth_minus_1 = depth - 1;
            format!(
                "MATCH (n) WHERE n.id = '{id}' MATCH (n)-[r]->(y) RETURN n.id as source, type(r) as label, y.id as target \
                 UNION \
                 MATCH (n) WHERE n.id = '{id}' MATCH (y)-[r]->(n) RETURN y.id as source, type(r) as label, n.id as target \
                 UNION \
                 MATCH (n)-[*1..{d}]->(x) WHERE n.id = '{id}' MATCH (x)-[r]->(y) RETURN x.id as source, type(r) as label, y.id as target \
                 UNION \
                 MATCH (n)-[*1..{d}]->(x) WHERE n.id = '{id}' MATCH (y)-[r]->(x) RETURN y.id as source, type(r) as label, x.id as target \
                 UNION \
                 MATCH (n)<-[*1..{d}]-(x) WHERE n.id = '{id}' MATCH (x)-[r]->(y) RETURN x.id as source, type(r) as label, y.id as target \
                 UNION \
                 MATCH (n)<-[*1..{d}]-(x) WHERE n.id = '{id}' MATCH (y)-[r]->(x) RETURN y.id as source, type(r) as label, x.id as target",
                id = safe_node_id, d = depth_minus_1
            )
        };
        
        let conn = graphqlite::Connection::open(&db_path)
            .map_err(|e| format!("Failed to open graph database: {}", e))?;
            
        let res = conn.cypher(&query)
            .map_err(|e| format!("Cypher query failed: {}", e))?;
            
        format_cypher_result(res)
    }

    #[tool(description = "Executes a graph query using Cypher syntax (e.g., MATCH (source)-[rel]->(target) WHERE ...) to discover patterns, links, or cross-file dependencies. This is the preferred tool for high-level semantic lookups and pattern matching in the database.")]
    fn query_graph_cypher(&self, Parameters(req): Parameters<QueryGraphCypherRequest>) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), None, None);
        let conn = graphqlite::Connection::open(&db_path)
            .map_err(|e| format!("Failed to open graph database: {}", e))?;
            
        let res = conn.cypher(&req.query)
            .map_err(|e| format!("Cypher query failed: {}", e))?;
            
        format_cypher_result(res)
    }

    #[tool(description = "Searches the graph for nodes matching a symbol name or pattern (e.g., a class, function, or file name). Use this tool to instantly find where a symbol is defined across the entire workspace without knowing its file path.")]
    fn search_symbols(&self, Parameters(req): Parameters<SearchSymbolsRequest>) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), None, None);
        let limit = req.limit.unwrap_or(50);
        let safe_query = req.query.replace("'", "''");
        
        let cypher_query = format!(
            "MATCH (n) WHERE toLower(n.id) CONTAINS toLower('{}') RETURN n.id as id, labels(n) as label LIMIT {}",
            safe_query, limit
        );
        
        let conn = graphqlite::Connection::open(&db_path)
            .map_err(|e| format!("Failed to open graph database: {}", e))?;
            
        let res = conn.cypher(&cypher_query)
            .map_err(|e| format!("Cypher query failed: {}", e))?;
            
        format_cypher_result(res)
    }

    #[tool(description = "Traces incoming or outgoing references for a specific node ID (e.g. finding callers or callees of a function). Provide the node_id and the direction ('incoming' for callers, 'outgoing' for callees).")]
    fn get_dependencies(&self, Parameters(req): Parameters<GetDependenciesRequest>) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), None, Some(&req.node_id));
        let safe_node_id = req.node_id.replace("'", "''");
        
        let cypher_query = if req.direction == "incoming" {
            format!(
                "MATCH (source)-[r]->(target) WHERE toLower(target.id) ENDS WITH toLower('{}') OR toLower(target.id) = toLower('{}') RETURN source.id as caller, type(r) as relationship",
                safe_node_id, safe_node_id
            )
        } else {
            format!(
                "MATCH (source)-[r]->(target) WHERE toLower(source.id) ENDS WITH toLower('{}') OR toLower(source.id) = toLower('{}') RETURN type(r) as relationship, target.id as callee",
                safe_node_id, safe_node_id
            )
        };
        
        let conn = graphqlite::Connection::open(&db_path)
            .map_err(|e| format!("Failed to open graph database: {}", e))?;
            
        let res = conn.cypher(&cypher_query)
            .map_err(|e| format!("Cypher query failed: {}", e))?;
            
        format_cypher_result(res)
    }

    #[tool(description = "Returns the structural outline of a source file (including its Classes, Methods, and Singleton Methods) by directly querying the pre-indexed graph database. Use this tool instead of `parse_project_file` when you only need to see what symbols are defined inside a file without incurring the heavy cost of disk reads or re-parsing. It efficiently lists the NodeIDs which you can then pass to `traverse_graph` or `get_dependencies` to explore their call relationships.")]
    fn get_file_structure(&self, Parameters(req): Parameters<GetFileStructureRequest>) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), Some(&req.file_path), None);
        let safe_file_path = req.file_path.replace("'", "''");
        
        let cypher_query = format!(
            "MATCH (f)-[r]->(n) WHERE f.id = '{}' AND type(r) = 'REL_CONTAINS' RETURN labels(n) as Type, n.id as NodeID, n.kind as AST_Kind ORDER BY Type, NodeID",
            safe_file_path
        );
        
        let conn = graphqlite::Connection::open(&db_path)
            .map_err(|e| format!("Failed to open graph database: {}", e))?;
            
        let res = conn.cypher(&cypher_query)
            .map_err(|e| format!("Cypher query failed: {}", e))?;
            
        format_cypher_result(res)
    }

    #[tool(description = "Retrieves a comprehensive list of all file paths currently tracked and indexed in the knowledge graph. Agents should use this tool to discover available source code files in the workspace (such as controllers, models, or specific modules) that are ready for immediate semantic querying via `get_file_structure` or `query_graph_cypher` without needing to rely on standard terminal commands like `ls` or `find`.")]
    fn list_indexed_files(&self, Parameters(req): Parameters<ListIndexedFilesRequest>) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), None, None);
        
        let cypher_query = "MATCH (n:File) RETURN n.id as FilePath ORDER BY FilePath";
        
        let conn = graphqlite::Connection::open(&db_path)
            .map_err(|e| format!("Failed to open graph database: {}", e))?;
            
        let res = conn.cypher(cypher_query)
            .map_err(|e| format!("Cypher query failed: {}", e))?;
            
        format_cypher_result(res)
    }
}

fn format_cypher_result(res: graphqlite::CypherResult) -> Result<String, String> {
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

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SaveNodeRequest {
    pub node: Node,
    #[schemars(description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory.")]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SaveEdgeRequest {
    pub edge: Edge,
    #[schemars(description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory.")]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ParseFileRequest {
    #[schemars(description = "The absolute or relative path to the Rust (.rs), Ruby (.rb), TypeScript (.ts), or TSX (.tsx) file to parse.")]
    pub file_path: String,
    #[schemars(description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory.")]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QueryGraphRequest {
    #[schemars(description = "The raw SQL SELECT query to execute against the knowledge.db database (e.g. querying nodes, edges, or node_props_text directly).")]
    pub query: String,
    #[schemars(description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory.")]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TraverseGraphRequest {
    #[schemars(description = "The globally unique string ID of the starting node (e.g. 'src/models.rs::Node').")]
    pub node_id: String,
    #[schemars(description = "Maximum depth of recursive hops to traverse. Defaults to 2.")]
    pub max_depth: Option<u32>,
    #[schemars(description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory.")]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QueryGraphCypherRequest {
    #[schemars(description = "The Cypher query string to execute. Example: 'MATCH (c:Class)-[:HAS_METHOD]->(m) RETURN c.id, m.id LIMIT 10'")]
    pub query: String,
    #[schemars(description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory.")]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchSymbolsRequest {
    #[schemars(description = "The symbol name or pattern to search for (e.g., 'UserController', 'main', 'save_node').")]
    pub query: String,
    #[schemars(description = "Optional limit on the number of results. Defaults to 50.")]
    pub limit: Option<u32>,
    #[schemars(description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory.")]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetDependenciesRequest {
    #[schemars(description = "The node ID or exact symbol name to trace dependencies for (e.g. 'src/main.rs::main' or just 'save_node').")]
    pub node_id: String,
    #[schemars(description = "Direction to trace: 'incoming' (find callers of this node) or 'outgoing' (find what this node calls).")]
    pub direction: String, 
    #[schemars(description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory.")]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListIndexedFilesRequest {
    #[schemars(description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory.")]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetFileStructureRequest {
    #[schemars(description = "The absolute or relative path to the file to query (e.g. '/path/to/app/models/user.rb').")]
    pub file_path: String,
    #[schemars(description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory.")]
    pub project_root: Option<String>,
}
