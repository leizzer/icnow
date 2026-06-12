use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use std::collections::HashMap;

use crate::models::{Edge, Node};
use crate::api_handlers::{memory, queries, tracing};

pub(crate) fn infer_project_root(file_path: &str) -> Option<String> {
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

#[derive(Debug, Clone)]
pub struct GraphService {
    pub db_path: String,
}

impl GraphService {
    fn resolve_db_path_and_watch(
        &self,
        project_root: Option<&str>,
        file_path: Option<&str>,
        node_id: Option<&str>,
    ) -> String {
        let root = project_root
            .map(|r| r.to_string())
            .or_else(|| file_path.and_then(infer_project_root))
            .or_else(|| node_id.and_then(infer_from_node_id));

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
    #[tool(
        description = "Saves a new node (file, function, class, module, etc.) into the graph. Use this tool manually only if the static parser missed a specific node or when explicitly registering domain-level concepts like Rails Controllers/Models and their fields."
    )]
    fn save_node(&self, Parameters(req): Parameters<SaveNodeRequest>) -> Result<String, String> {
        let db_path =
            self.resolve_db_path_and_watch(req.project_root.as_deref(), Some(&req.node.id), None);
        let graph =
            crate::open_db_graph(&db_path).map_err(|e| format!("Failed to open DB: {e}"))?;

        req.node.save(&graph).map_err(|e| e.to_string())?;

        Ok(format!("Node {} saved successfully.", req.node.id))
    }

    #[tool(
        description = "Creates or updates a directed edge between two existing nodes (e.g. connecting a caller function to a callee method, or mapping database entity relationships). Use this tool to explicitly link imports to their physical file targets, functions to their internal calls, or class inheritance/mixins."
    )]
    fn save_edge(&self, Parameters(req): Parameters<SaveEdgeRequest>) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(
            req.project_root.as_deref(),
            Some(&req.edge.source),
            None,
        );
        let graph =
            crate::open_db_graph(&db_path).map_err(|e| format!("Failed to open DB: {e}"))?;

        req.edge.save(&graph).map_err(|e| e.to_string())?;

        Ok(format!("Edge {} saved successfully.", req.edge.id))
    }

    #[tool(
        description = "Parses a source file (Rust, Ruby, TypeScript, TSX) using Tree-sitter, extracts all structural nodes (Functions, Methods, Classes, Interfaces, Imports), and automatically adds them and their container relationships to the graph database. Call this tool first to map out the architecture of a new or modified file."
    )]
    fn parse_project_file(
        &self,
        Parameters(req): Parameters<ParseFileRequest>,
    ) -> Result<String, String> {
        let db_path =
            self.resolve_db_path_and_watch(req.project_root.as_deref(), Some(&req.file_path), None);
        let graph =
            crate::open_db_graph(&db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
        let _ = graph.query("BEGIN TRANSACTION");
        let summary = crate::parser::parse_file(&req.file_path, &graph)
            .map_err(|e| {
                let _ = graph.query("ROLLBACK");
                format!("Parse error: {e}")
            })?;
        let _ = graph.query("COMMIT");

        let mut out = format!(
            "Successfully parsed `{}` and added nodes to graph.\n\n",
            req.file_path
        );
        out.push_str("**File Architecture Summary:**\n");

        if !summary.imports.is_empty() {
            out.push_str(&format!(
                "- **Imports**: `{}`\n",
                summary.imports.join("`, `")
            ));
        }

        if !summary.structures.is_empty() {
            for (label, names) in &summary.structures {
                let plural_label = if label == "Class" {
                    "Classes".to_string()
                } else {
                    format!("{label}s")
                };
                out.push_str(&format!("- **{plural_label}**:\n"));
                for name in names {
                    out.push_str(&format!("  - `{name}`\n"));
                    if let Some(methods) = summary.methods.get(name) {
                        let mut grouped_methods: HashMap<String, Vec<String>> = HashMap::new();
                        for (m_label, m_name) in methods {
                            grouped_methods
                                .entry(m_label.clone())
                                .or_default()
                                .push(m_name.clone());
                        }
                        for (m_label, m_names) in grouped_methods {
                            out.push_str(&format!(
                                "    - {}s: `{}`\n",
                                m_label,
                                m_names.join("`, `")
                            ));
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

    #[tool(
        description = "Executes a raw SQL SELECT query against the underlying SQLite database tables (nodes, edges, node_props_text) to retrieve metadata, properties, counts, or precise source code fragments. **CRITICAL:** NEVER use PRAGMA queries to discover tables! ALWAYS call `get_graph_schema` FIRST to get the schema. This is the PREFERRED tool for lookups and filtering over Cypher due to 12,000x index speedups."
    )]
    fn query_graph(
        &self,
        Parameters(req): Parameters<QueryGraphRequest>,
    ) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), None, None);
        queries::handle_query_graph(&db_path, req)
    }

    #[tool(
        description = "Recursively walks the graph bidirectionally from a starting node up to a specified depth (max_depth) and returns an indented relationship list. Use this tool when you want to discover the neighborhood of dependencies, callers, or subclasses of a particular node in a single call."
    )]
    fn traverse_graph(
        &self,
        Parameters(req): Parameters<TraverseGraphRequest>,
    ) -> Result<String, String> {
        let db_path =
            self.resolve_db_path_and_watch(req.project_root.as_deref(), None, Some(&req.node_id));
        tracing::handle_traverse_graph(&db_path, req)
    }

    #[tool(
        description = "Executes a graph query using Cypher syntax (e.g., MATCH (source)-[rel]->(target) WHERE ...) to discover patterns, links, or cross-file dependencies. **CRITICAL:** DO NOT use this for property lookups, text filtering, or counts (use `query_graph` SQL instead). ONLY use Cypher for multi-hop structural/relationship traversals."
    )]
    fn query_graph_cypher(
        &self,
        Parameters(req): Parameters<QueryGraphCypherRequest>,
    ) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), None, None);
        queries::handle_query_graph_cypher(&db_path, req)
    }

    #[tool(
        description = "Searches the graph for nodes matching a symbol name or pattern (e.g., a class, function, or file name). Use this tool to instantly find where a symbol is defined across the entire workspace without knowing its file path."
    )]
    fn search_symbols(
        &self,
        Parameters(req): Parameters<SearchSymbolsRequest>,
    ) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), None, None);
        queries::handle_search_symbols(&db_path, req)
    }

    #[tool(
        description = "Traces incoming or outgoing references for a specific node ID (e.g. finding callers or callees of a function). Provide the node_id and the direction ('incoming' for callers, 'outgoing' for callees)."
    )]
    fn get_dependencies(
        &self,
        Parameters(req): Parameters<GetDependenciesRequest>,
    ) -> Result<String, String> {
        let db_path =
            self.resolve_db_path_and_watch(req.project_root.as_deref(), None, Some(&req.node_id));
        tracing::handle_get_dependencies(&db_path, req)
    }

    #[tool(
        description = "Returns the structural outline of a source file (including its Classes, Methods, and Singleton Methods) by directly querying the pre-indexed graph database. Use this tool instead of `parse_project_file` when you only need to see what symbols are defined inside a file without incurring the heavy cost of disk reads or re-parsing. It efficiently lists the NodeIDs which you can then pass to `traverse_graph` or `get_dependencies` to explore their call relationships."
    )]
    fn get_file_structure(
        &self,
        Parameters(req): Parameters<GetFileStructureRequest>,
    ) -> Result<String, String> {
        let db_path =
            self.resolve_db_path_and_watch(req.project_root.as_deref(), Some(&req.file_path), None);
        queries::handle_get_file_structure(&db_path, req)
    }

    #[tool(
        description = "Retrieves a comprehensive list of all file paths currently tracked and indexed in the knowledge graph. Agents should use this tool to discover available source code files in the workspace (such as controllers, models, or specific modules) that are ready for immediate semantic querying via `get_file_structure` or `query_graph_cypher` without needing to rely on standard terminal commands like `ls` or `find`."
    )]
    fn list_indexed_files(
        &self,
        Parameters(req): Parameters<ListIndexedFilesRequest>,
    ) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), None, None);
        queries::handle_list_indexed_files(&db_path, req)
    }

    #[tool(
        description = "Generates a standalone, interactive HTML map of the knowledge graph and saves it to a specified file. The HTML file uses Cytoscape.js to visually render all files, functions, classes, and their relationships (IMPORTS, CALLS, CONTAINS). Agents should use this tool when the user asks for a visual representation of the architecture or a specific folder/file."
    )]
    fn generate_interactive_map(
        &self,
        Parameters(req): Parameters<GenerateInteractiveMapRequest>,
    ) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), None, None);
        let filter = req.filter_path.unwrap_or_default();

        crate::exporter::generate_html(&db_path, &req.output_path, &filter)
            .map_err(|e| format!("Failed to generate interactive map: {e}"))?;

        Ok(format!(
            "Interactive map successfully generated and saved to: {}",
            req.output_path
        ))
    }

    #[tool(
        description = "Retrieves the raw source code block (implementation body) of a specific symbol (e.g. class, method, or standalone function) without reading the entire file. Use this tool when you need to inspect the actual code logic of a node found via search_symbols or get_file_structure."
    )]
    fn get_symbol_implementation(
        &self,
        Parameters(req): Parameters<GetSymbolImplementationRequest>,
    ) -> Result<String, String> {
        let db_path =
            self.resolve_db_path_and_watch(req.project_root.as_deref(), None, Some(&req.node_id));
        queries::handle_get_symbol_implementation(&db_path, req)
    }

    #[tool(
        description = "Traces multi-hop call paths between a specific start_node_id and end_node_id up to a max_depth. Useful for finding how a controller reaches a specific database model or service without having to call get_dependencies repeatedly."
    )]
    fn trace_call_path(
        &self,
        Parameters(req): Parameters<TraceCallPathRequest>,
    ) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(
            req.project_root.as_deref(),
            None,
            Some(&req.start_node_id),
        );
        tracing::handle_trace_call_path(&db_path, req)
    }

    #[tool(
        description = "Returns documentation about the graph schema (available node labels, relationship types, and property keys). **CRITICAL:** ALWAYS call this tool FIRST to understand the SQLite tables (`nodes`, `edges`, etc.) before writing ANY SQL queries. NEVER use `PRAGMA table_info`."
    )]
    fn get_graph_schema(
        &self,
        Parameters(_req): Parameters<GetGraphSchemaRequest>,
    ) -> Result<String, String> {
        let schema = r#"
# `icnow` Knowledge Graph (SQLite Schema)

**CRITICAL PERFORMANCE RULE:** To maximize performance and avoid slow Cypher translations, prioritize writing **raw SQL queries** using the `query_graph` tool when answering complex questions. **DO NOT** use PRAGMA queries to discover the schema—it is provided below.

## Database Schema

### `nodes`
- `id`: INTEGER PRIMARY KEY

### `edges`
- `source_id`: INTEGER (Foreign key to `nodes.id`)
- `target_id`: INTEGER (Foreign key to `nodes.id`)
- `type`: TEXT (Relationship type: e.g., 'HAS_METHOD', 'REL_CONTAINS', 'CALLS', 'IMPORTS')

### `node_labels`
- `node_id`: INTEGER (Foreign key to `nodes.id`)
- `label`: TEXT (Node type: e.g., 'Class', 'Module', 'File', 'Method', 'Function', 'Struct')

### `property_keys`
- `id`: INTEGER PRIMARY KEY
- `key`: TEXT (Property name: e.g., 'id' (absolute path/identifier), 'name', 'source_code')

### `node_props_text`
- `node_id`: INTEGER (Foreign key to `nodes.id`)
- `key_id`: INTEGER (Foreign key to `property_keys.id`)
- `value`: TEXT (The actual property value)

## Query Patterns and Best Practices

To fetch properties efficiently, join `node_props_text` and `property_keys`. 
**DO NOT** use string manipulation functions (`toLower`, `replace`) on indexed columns (`value`, `label`, `type`), as this forces full table scans. Use standard `LIKE` or `=` operations.

### Example: Finding properties of a Node (e.g. Class 'User')
```sql
SELECT p_id.value AS node_identifier, p_name.value AS name, l.label
FROM nodes n
JOIN node_labels l ON n.id = l.node_id
JOIN node_props_text p_id ON n.id = p_id.node_id 
  AND p_id.key_id = (SELECT id FROM property_keys WHERE key = 'id')
LEFT JOIN node_props_text p_name ON n.id = p_name.node_id 
  AND p_name.key_id = (SELECT id FROM property_keys WHERE key = 'name')
WHERE p_name.value = 'User' AND l.label = 'Class';
```

### Example: Counting Methods of a Specific Class ('User')
```sql
SELECT COUNT(e.target_id)
FROM edges e
JOIN node_props_text p_source ON e.source_id = p_source.node_id 
  AND p_source.key_id = (SELECT id FROM property_keys WHERE key = 'id')
WHERE e.type = 'HAS_METHOD' 
  AND p_source.value LIKE '%::User';
```

### Example: Finding all Files
```sql
SELECT p.value AS FilePath 
FROM node_labels l
JOIN node_props_text p ON l.node_id = p.node_id 
  AND p.key_id = (SELECT id FROM property_keys WHERE key = 'id')
WHERE l.label = 'File';
```
        "#;
        Ok(schema.trim().to_string())
    }

    #[tool(
        description = "Parses an LSIF (Language Server Index Format) dump file to extract precise definition and reference relationships across the codebase and imports them into the graph database. If no lsif_path is provided, it automatically detects the project type (Rust, Ruby, TypeScript/React) and generates the dump on the fly using standard CLI compilers."
    )]
    fn deep_scan(&self, Parameters(req): Parameters<DeepScanRequest>) -> Result<String, String> {
        let inferred_root = req
            .project_root
            .clone()
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .map(|d| d.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| ".".to_string());

        let db_path = self.resolve_db_path_and_watch(Some(&inferred_root), None, None);

        let (actual_lsif_path, is_temp) = match req.lsif_path {
            Some(path) => (path, false),
            None => {
                let generated = crate::lsif::auto_generate_lsif(&inferred_root)
                    .map_err(|e| format!("Auto-generation of LSIF failed: {e}"))?;
                (generated, true)
            }
        };

        let import_res =
            crate::lsif::parse_and_import_lsif(&actual_lsif_path, &db_path, Some(&inferred_root));

        if is_temp {
            let _ = std::fs::remove_file(&actual_lsif_path);
        }

        let (nodes, edges) = import_res.map_err(|e| format!("LSIF Import failed: {e}"))?;

        Ok(format!(
            "LSIF scan completed successfully.\n\n- **Nodes Imported**: {nodes}\n- **Edges Imported**: {edges}"
        ))
    }

    #[tool(
        description = "[EXPERIMENTAL] Creates or updates a memory node representing a high-level concept or business logic flow, linking it to code nodes or other memory nodes. Enforces prefix 'memory::' and validates that all link targets exist in the database."
    )]
    fn save_memory(
        &self,
        Parameters(req): Parameters<SaveMemoryRequest>,
    ) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), None, None);
        memory::handle_save_memory(&db_path, req)
    }

    #[tool(
        description = "[EXPERIMENTAL] Retrieves a detailed memory node, its description, associated keywords, and its connections to code files/methods and sub-concepts."
    )]
    fn get_memory(&self, Parameters(req): Parameters<GetMemoryRequest>) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), None, None);
        memory::handle_get_memory(&db_path, req)
    }

    #[tool(
        description = "[EXPERIMENTAL] Searches for concepts and business logic flows using SQLite FTS5 relevance ranking. Returns matching memory nodes and their descriptions."
    )]
    fn search_memories(
        &self,
        Parameters(req): Parameters<SearchMemoriesRequest>,
    ) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), None, None);
        memory::handle_search_memories(&db_path, req)
    }

    #[tool(
        description = "[EXPERIMENTAL] Lists all high-level concept memory nodes stored in the project's knowledge base."
    )]
    fn list_memories(
        &self,
        Parameters(req): Parameters<ListMemoriesRequest>,
    ) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(req.project_root.as_deref(), None, None);
        memory::handle_list_memories(&db_path, req)
    }
}

pub(crate) fn format_cypher_result(res: &mut lbug::QueryResult) -> Result<String, String> {
    let cols = res.get_column_names();
    if cols.is_empty() {
        return Ok("No columns returned.".to_string());
    }

    let mut out = format!("| {} |\n", cols.join(" | "));
    out.push_str(&format!(
        "| {} |\n",
        cols.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
    ));

    for row in res.by_ref() {
        let mut row_vals = Vec::new();
        for (i, _col) in cols.iter().enumerate() {
            let val_str = match &row[i] {
                lbug::Value::String(s) => s.clone(),
                lbug::Value::Int64(i) => i.to_string(),
                lbug::Value::Int32(i) => i.to_string(),
                lbug::Value::Double(f) => f.to_string(),
                lbug::Value::Bool(b) => b.to_string(),
                lbug::Value::Null(_) => "null".to_string(),
                _ => "?".to_string(),
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
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SaveEdgeRequest {
    pub edge: Edge,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ParseFileRequest {
    #[schemars(
        description = "The absolute or relative path to the Rust (.rs), Ruby (.rb), TypeScript (.ts), or TSX (.tsx) file to parse."
    )]
    pub file_path: String,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QueryGraphRequest {
    #[schemars(
        description = "The raw SQL SELECT query to execute against the knowledge.db database (e.g. querying nodes, edges, or node_props_text directly)."
    )]
    pub query: String,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TraverseGraphRequest {
    #[schemars(
        description = "The globally unique string ID of the starting node (e.g. 'src/models.rs::Node')."
    )]
    pub node_id: String,
    #[schemars(description = "Maximum depth of recursive hops to traverse. Defaults to 2.")]
    pub max_depth: Option<u32>,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QueryGraphCypherRequest {
    #[schemars(
        description = "The Cypher query string to execute. Example: 'MATCH (c:Class)-[:HAS_METHOD]->(m) RETURN c.id, m.id LIMIT 10'"
    )]
    pub query: String,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchSymbolsRequest {
    #[schemars(
        description = "The symbol name or pattern to search for (e.g., 'UserController', 'main', 'save_node')."
    )]
    pub query: String,
    #[schemars(description = "Optional limit on the number of results. Defaults to 50.")]
    pub limit: Option<u32>,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: Option<String>,
    #[schemars(
        description = "Optional list of node labels to filter the results (e.g., ['Class', 'Method'])."
    )]
    pub kind_filter: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetDependenciesRequest {
    #[schemars(
        description = "The node ID or exact symbol name to trace dependencies for (e.g. 'src/main.rs::main' or just 'save_node')."
    )]
    pub node_id: String,
    #[schemars(
        description = "Direction to trace: 'incoming' (find callers of this node) or 'outgoing' (find what this node calls)."
    )]
    pub direction: String,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListIndexedFilesRequest {
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetFileStructureRequest {
    #[schemars(
        description = "The absolute or relative path to the file to query (e.g. '/path/to/app/models/user.rb')."
    )]
    pub file_path: String,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GenerateInteractiveMapRequest {
    #[schemars(
        description = "The absolute path where the HTML file should be saved (e.g. '/path/to/project/architecture.html')."
    )]
    pub output_path: String,
    #[schemars(
        description = "Optional path prefix to filter the exported graph. Only nodes starting with this path (e.g. a specific directory or file) will be included."
    )]
    pub filter_path: Option<String>,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSymbolImplementationRequest {
    #[schemars(
        description = "The globally unique string ID of the node to retrieve source code for (e.g. 'src/models.rs::Node')."
    )]
    pub node_id: String,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TraceCallPathRequest {
    #[schemars(description = "The globally unique string ID of the starting node (caller).")]
    pub start_node_id: String,
    #[schemars(description = "The globally unique string ID of the target node (callee).")]
    pub end_node_id: String,
    #[schemars(
        description = "Maximum depth of recursive hops to traverse. Defaults to 5. Maximum is 10."
    )]
    pub max_depth: Option<u32>,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetGraphSchemaRequest {
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeepScanRequest {
    #[schemars(
        description = "Optional path to a pre-generated LSIF dump file. If omitted, icnow will attempt to auto-generate the LSIF dump based on project detection."
    )]
    pub lsif_path: Option<String>,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SaveMemoryRequest {
    #[schemars(
        description = "The globally unique string ID of the memory node. MUST start with the prefix 'memory::' (e.g. 'memory::user_auth')."
    )]
    pub id: String,
    #[schemars(
        description = "A concise, human-readable name for the concept or logic block (e.g. 'User Authentication Flow')."
    )]
    pub name: String,
    #[schemars(
        description = "A detailed description of the memory concept, detailing its architectural role, business rules, or key steps."
    )]
    pub description: String,
    #[schemars(
        description = "A list of relevant keywords to index this memory for search (e.g. ['login', 'jwt', 'session'])."
    )]
    pub keywords: Vec<String>,
    #[schemars(
        description = "A list of globally unique IDs of code elements (Files, Classes, Methods) or other memory nodes that this concept explains or relates to."
    )]
    pub links: Vec<String>,
    #[schemars(
        description = "Optional custom label type for the relationship edges created. Defaults to 'EXPLAINS' for code nodes and 'SUB_CONCEPT' for memory nodes."
    )]
    pub link_type: Option<String>,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetMemoryRequest {
    #[schemars(
        description = "The globally unique string ID of the memory node to retrieve (must start with 'memory::')."
    )]
    pub id: String,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchMemoriesRequest {
    #[schemars(
        description = "The search query to match against memory names, descriptions, and keywords using SQLite FTS5."
    )]
    pub query: String,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListMemoriesRequest {
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_nodes() {
        let db_path = "test_memories.db";
        let _ = std::fs::remove_file(db_path);

        let _conn = crate::open_db_connection(db_path).unwrap();
        let graph = crate::open_db_graph(db_path).unwrap();

        // 1. Create a dummy code node so we can validate links pointing to it
        let node1 = crate::models::Node {
            id: "src/main.rs".to_string(),
            label: "File".to_string(),
            kind: "File".to_string(),
            properties: HashMap::new(),
        };
        node1.save(&graph).unwrap();

        // Create an absolute path node for testing relative path resolution
        let cur_dir = std::env::current_dir().unwrap();
        let abs_file_path = cur_dir
            .join("src/lib.rs")
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let node2 = crate::models::Node {
            id: abs_file_path.clone(),
            label: "File".to_string(),
            kind: "File".to_string(),
            properties: HashMap::new(),
        };
        node2.save(&graph).unwrap();

        let service = GraphService {
            db_path: db_path.to_string(),
        };

        // 2. Try to save a memory node with an invalid prefix
        let err_res = service.save_memory(Parameters(SaveMemoryRequest {
            id: "bad_prefix::auth".to_string(),
            name: "Auth Flow".to_string(),
            description: "Flow".to_string(),
            keywords: vec![],
            links: vec![],
            link_type: None,
            project_root: None,
        }));
        assert!(err_res.is_err());
        assert!(err_res.unwrap_err().contains("prefix"));

        // 3. Try to save with links that don't exist
        let err_res = service.save_memory(Parameters(SaveMemoryRequest {
            id: "memory::auth".to_string(),
            name: "Auth Flow".to_string(),
            description: "Flow".to_string(),
            keywords: vec![],
            links: vec!["src/non_existent.rs".to_string()],
            link_type: None,
            project_root: None,
        }));
        assert!(err_res.is_err());
        assert!(err_res.unwrap_err().contains("Link target not found"));

        // 4. Save a valid memory pointing to an existing file
        let ok_res = service
            .save_memory(Parameters(SaveMemoryRequest {
                id: "memory::auth".to_string(),
                name: "Auth Flow".to_string(),
                description: "User authentication using OAuth and JWT token validation."
                    .to_string(),
                keywords: vec!["oauth".to_string(), "jwt".to_string(), "token".to_string()],
                links: vec!["src/main.rs".to_string()],
                link_type: None,
                project_root: None,
            }))
            .unwrap();
        assert!(ok_res.contains("saved successfully"));

        // 4b. Save a memory pointing to an absolute node via relative path target
        let ok_res2 = service
            .save_memory(Parameters(SaveMemoryRequest {
                id: "memory::relative_test".to_string(),
                name: "Relative Path Test".to_string(),
                description: "Testing relative path resolution.".to_string(),
                keywords: vec![],
                links: vec!["src/lib.rs".to_string()],
                link_type: None,
                project_root: None,
            }))
            .unwrap();
        assert!(ok_res2.contains("saved successfully"));

        // Query the memory and verify that the target was resolved to absolute path
        let get_res2 = service
            .get_memory(Parameters(GetMemoryRequest {
                id: "memory::relative_test".to_string(),
                project_root: None,
            }))
            .unwrap();
        assert!(get_res2.contains("Relative Path Test"));
        assert!(get_res2.contains(&abs_file_path));

        // 5. Query the memory
        let get_res = service
            .get_memory(Parameters(GetMemoryRequest {
                id: "memory::auth".to_string(),
                project_root: None,
            }))
            .unwrap();
        assert!(get_res.contains("Auth Flow"));
        assert!(get_res.contains("JWT token validation"));
        assert!(get_res.contains("Connected Code Elements"));
        assert!(get_res.contains("src/main.rs"));

        // 6. Test list_memories
        let list_res = service
            .list_memories(Parameters(ListMemoriesRequest { project_root: None }))
            .unwrap();
        assert!(list_res.contains("Auth Flow"));
        assert!(list_res.contains("memory::auth"));

        // 7. Test FTS5 search_memories
        let search_res = service
            .search_memories(Parameters(SearchMemoriesRequest {
                query: "jwt token".to_string(),
                project_root: None,
            }))
            .unwrap();
        assert!(search_res.contains("Auth Flow"));
        assert!(search_res.contains("memory::auth"));

        // Test search with prefix or non-alphanumeric chars
        let search_res2 = service
            .search_memories(Parameters(SearchMemoriesRequest {
                query: "oauth".to_string(),
                project_root: None,
            }))
            .unwrap();
        assert!(search_res2.contains("Auth Flow"));

        let _ = std::fs::remove_file(db_path);
    }
}
