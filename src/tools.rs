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
        
        let mut cypher_query = format!(
            "MATCH (n) WHERE toLower(n.id) CONTAINS toLower('{}')",
            safe_query
        );
        
        if let Some(filters) = &req.kind_filter {
            if !filters.is_empty() {
                let conditions: Vec<String> = filters.iter()
                    .map(|f| format!("'{}' IN labels(n)", f.replace("'", "''")))
                    .collect();
                cypher_query.push_str(&format!(" AND ({})", conditions.join(" OR ")));
            }
        }
        
        cypher_query.push_str(&format!(" RETURN n.id as id, labels(n) as label LIMIT {}", limit));
        
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
        
        let conn = graphqlite::Connection::open(&db_path)
            .map_err(|e| format!("Failed to open graph database: {}", e))?;
            
        let nodes_query = format!(
            "MATCH (f)-[:CONTAINS]->(n) WHERE f.id = '{}' RETURN n.id as id, n.name as name, labels(n) as label",
            safe_file_path
        );
        let res_nodes = conn.cypher(&nodes_query).map_err(|e| format!("Nodes query failed: {}", e))?;
        
        let mut nodes: std::collections::HashMap<String, (String, String)> = std::collections::HashMap::new();
        for row in res_nodes {
            if let (Ok(id), Ok(name), Ok(label)) = (row.get::<String>("id"), row.get::<String>("name"), row.get::<String>("label")) {
                let clean_label = label.replace("[\"", "").replace("\"]", "").replace("[", "").replace("]", "");
                nodes.insert(id, (name, clean_label));
            }
        }
        
        let edges_query = format!(
            "MATCH (s)-[:HAS_METHOD]->(m) WHERE s.id STARTS WITH '{}::' AND m.id STARTS WITH '{}::' RETURN s.id as parent, m.id as child",
            safe_file_path, safe_file_path
        );
        let res_edges = conn.cypher(&edges_query).map_err(|e| format!("Edges query failed: {}", e))?;
        
        let mut children_map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        let mut child_set: std::collections::HashSet<String> = std::collections::HashSet::new();
        
        for row in res_edges {
            if let (Ok(p), Ok(c)) = (row.get::<String>("parent"), row.get::<String>("child")) {
                children_map.entry(p).or_default().push(c.clone());
                child_set.insert(c);
            }
        }
        
        if nodes.is_empty() {
            return Ok(format!("No symbols found in file {}", req.file_path));
        }
        
        let mut out = format!("File Structure for `{}`:\n\n", req.file_path);
        let mut root_nodes: Vec<&String> = nodes.keys().filter(|k| !child_set.contains(*k)).collect();
        root_nodes.sort();
        
        for root_id in root_nodes {
            if let Some((name, label)) = nodes.get(root_id) {
                out.push_str(&format!("- {} `{}` ({})\n", label, name, root_id));
                if let Some(children) = children_map.get(root_id) {
                    let mut sorted_children = children.clone();
                    sorted_children.sort();
                    for child_id in sorted_children {
                        if let Some((cname, clabel)) = nodes.get(&child_id) {
                            out.push_str(&format!("  - {} `{}` ({})\n", clabel, cname, child_id));
                        }
                    }
                }
            }
        }
        
        Ok(out)
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

    #[tool(description = "Generates a standalone, interactive HTML map of the knowledge graph and saves it to a specified file. The HTML file uses Cytoscape.js to visually render all files, functions, classes, and their relationships (IMPORTS, CALLS, CONTAINS). Agents should use this tool when the user asks for a visual representation of the architecture or a specific folder/file.")]
    fn generate_interactive_map(&self, Parameters(req): Parameters<GenerateInteractiveMapRequest>) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), None, None);
        let filter = req.filter_path.unwrap_or_default();
        
        crate::exporter::generate_html(&db_path, &req.output_path, &filter)
            .map_err(|e| format!("Failed to generate interactive map: {}", e))?;
            
        Ok(format!("Interactive map successfully generated and saved to: {}", req.output_path))
    }

    #[tool(description = "Retrieves the raw source code block (implementation body) of a specific symbol (e.g. class, method, or standalone function) without reading the entire file. Use this tool when you need to inspect the actual code logic of a node found via search_symbols or get_file_structure.")]
    fn get_symbol_implementation(&self, Parameters(req): Parameters<GetSymbolImplementationRequest>) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), None, Some(&req.node_id));
        let safe_node_id = req.node_id.replace("'", "''");
        let cypher = format!("MATCH (n) WHERE n.id = '{}' RETURN n.source_code as source_code", safe_node_id);
        
        let conn = graphqlite::Connection::open(&db_path).map_err(|e| format!("Failed to open DB: {}", e))?;
        let res = conn.cypher(&cypher).map_err(|e| format!("Query failed: {}", e))?;
        
        let mut out = String::new();
        for row in res {
            if let Ok(src) = row.get::<String>("source_code") {
                out = src;
                break;
            }
        }
        
        if out.is_empty() {
            return Err(format!("Source code not found for node '{}'. It might not be a structure/method, or the file lacks source mapping.", req.node_id));
        }
        
        Ok(out)
    }

    #[tool(description = "Traces multi-hop call paths between a specific start_node_id and end_node_id up to a max_depth. Useful for finding how a controller reaches a specific database model or service without having to call get_dependencies repeatedly.")]
    fn trace_call_path(&self, Parameters(req): Parameters<TraceCallPathRequest>) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), None, Some(&req.start_node_id));
        let conn = graphqlite::Connection::open(&db_path)
            .map_err(|e| format!("Failed to open graph database: {}", e))?;
            
        let max_depth = req.max_depth.unwrap_or(5);
        if max_depth > 10 {
            return Err("max_depth cannot exceed 10".into());
        }
        
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(vec![req.start_node_id.clone()]);
        let mut visited = std::collections::HashSet::new();
        visited.insert(req.start_node_id.clone());
        
        let mut paths = Vec::new();
        
        while let Some(path) = queue.pop_front() {
            if path.len() - 1 > max_depth as usize {
                continue;
            }
            let current = path.last().unwrap();
            if current == &req.end_node_id {
                paths.push(path.clone());
                if paths.len() >= 5 { break; } // Limit to 5 paths to avoid massive output
                continue;
            }
            
            let safe_curr = current.replace("'", "''");
            let q = format!("MATCH (s)-[r:CALLS]->(t) WHERE s.id = '{}' RETURN t.id as target", safe_curr);
            if let Ok(res) = conn.cypher(&q) {
                for row in res {
                    if let Ok(target) = row.get::<String>("target") {
                        if !path.contains(&target) { // avoid cycles
                            let mut new_path = path.clone();
                            new_path.push(target.clone());
                            queue.push_back(new_path);
                        }
                    }
                }
            }
        }
        
        if paths.is_empty() {
            return Ok(format!("No CALLS path found between {} and {} within {} hops.", req.start_node_id, req.end_node_id, max_depth));
        }
        
        let mut out = format!("Found {} path(s) between {} and {}:\n\n", paths.len(), req.start_node_id, req.end_node_id);
        for (i, p) in paths.iter().enumerate() {
            out.push_str(&format!("Path {}:\n", i + 1));
            out.push_str(&p.join(" -> "));
            out.push_str("\n\n");
        }
        
        Ok(out)
    }

    #[tool(description = "Returns documentation about the graph schema (available node labels, relationship types, and property keys). Useful to understand what data exists in the knowledge graph before writing Cypher queries.")]
    fn get_graph_schema(&self, Parameters(_req): Parameters<GetGraphSchemaRequest>) -> Result<String, String> {
        let schema = r#"
# `icnow` Knowledge Graph Schema

## Node Labels
- `File`: Represents a source file (e.g. `src/main.rs`). Property `id` is the absolute path.
- `Class` / `Module` / `Struct` / `Interface`: Represent object-oriented and structural containers.
- `Method` / `Function`: Represent callable logic blocks.
- `Import`: Represents a module or package import.
- `Unresolved`: Represents a symbol that was called but its definition couldn't be accurately statically resolved.

## Edge/Relationship Labels
- `CONTAINS`: Structural containment (e.g. `File` -[:CONTAINS]-> `Class`, `File` -[:CONTAINS]-> `Function`).
- `HAS_METHOD`: Class-to-method containment (e.g. `Class` -[:HAS_METHOD]-> `Method`).
- `CALLS`: Function invocation (e.g. `Function` -[:CALLS]-> `Function`).
- `IMPORTS`: Cross-file dependency tracking (e.g. `File` -[:IMPORTS]-> `File`).

## Common Properties
- `id`: The globally unique identifier for nodes/edges. For structural nodes, it's `filepath::namespace::name`.
- `name`: The local name of the symbol.
- `file`: The absolute path of the file containing this node.
- `source_code`: The raw text implementation of the node (available for Classes, Methods, Functions).
- `last_modified`: Epoch timestamp (for `File` nodes).
        "#;
        Ok(schema.trim().to_string())
    }

    #[tool(description = "Parses an LSIF (Language Server Index Format) dump file to extract precise definition and reference relationships across the codebase and imports them into the graph database. This is a heavy, precise scan that resolves cross-file connections with compiler-grade accuracy.")]
    fn deep_scan(&self, Parameters(req): Parameters<DeepScanRequest>) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), None, None);
        
        let (nodes, edges) = crate::lsif::parse_and_import_lsif(&req.lsif_path, &db_path, req.project_root.as_deref())
            .map_err(|e| format!("LSIF Import failed: {}", e))?;
            
        Ok(format!(
            "LSIF scan completed successfully.\n\n- **Nodes Imported**: {}\n- **Edges Imported**: {}",
            nodes, edges
        ))
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
    #[schemars(description = "Optional list of node labels to filter the results (e.g., ['Class', 'Method']).")]
    pub kind_filter: Option<Vec<String>>,
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

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GenerateInteractiveMapRequest {
    #[schemars(description = "The absolute path where the HTML file should be saved (e.g. '/path/to/project/architecture.html').")]
    pub output_path: String,
    #[schemars(description = "Optional path prefix to filter the exported graph. Only nodes starting with this path (e.g. a specific directory or file) will be included.")]
    pub filter_path: Option<String>,
    #[schemars(description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory.")]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSymbolImplementationRequest {
    #[schemars(description = "The globally unique string ID of the node to retrieve source code for (e.g. 'src/models.rs::Node').")]
    pub node_id: String,
    #[schemars(description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory.")]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TraceCallPathRequest {
    #[schemars(description = "The globally unique string ID of the starting node (caller).")]
    pub start_node_id: String,
    #[schemars(description = "The globally unique string ID of the target node (callee).")]
    pub end_node_id: String,
    #[schemars(description = "Maximum depth of recursive hops to traverse. Defaults to 5. Maximum is 10.")]
    pub max_depth: Option<u32>,
    #[schemars(description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory.")]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetGraphSchemaRequest {
    #[schemars(description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory.")]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeepScanRequest {
    #[schemars(description = "The path to the LSIF dump file (e.g. 'dump.lsif').")]
    pub lsif_path: String,
    #[schemars(description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory.")]
    pub project_root: Option<String>,
}
