use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use std::collections::HashMap;

use crate::api_handlers::{memory, queries, tracing};
pub mod structs;
pub use structs::*;

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
    pub(crate) fn resolve_db_path_and_watch(
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
            crate::indexer::watcher::ensure_watching(path_buf, &db_path);
            db_path
        } else {
            self.db_path.clone()
        }
    }
}

/// Helper to run a blocking closure on the spawn_blocking pool and flatten the JoinError.
async fn blocking<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| format!("Task join error: {e}"))?
}

#[tool_router(server_handler)]
impl GraphService {
    #[tool(
        description = "Saves a new node (file, function, class, module, etc.) into the graph. Use this tool manually only if the static parser missed a specific node or when explicitly registering domain-level concepts like Rails Controllers/Models and their fields."
    )]
    async fn save_node(
        &self,
        Parameters(req): Parameters<SaveNodeRequest>,
    ) -> Result<String, String> {
        let svc = self.clone();
        blocking(move || {
            let db_path = svc.resolve_db_path_and_watch(
                Some(req.project_root.as_str()),
                Some(&req.node.id),
                None,
            );
            let db = crate::database::get_or_init_db(&db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
            let graph = lbug::Connection::new(db.as_ref()).map_err(|e| format!("Failed to create connection: {e}"))?;

            req.node.save(&graph).map_err(|e| e.to_string())?;

            Ok(format!("Node {} saved successfully.", req.node.id))
        })
        .await
    }

    #[tool(
        description = "Creates or updates a directed edge between two existing nodes (e.g. connecting a caller function to a callee method, or mapping database entity relationships). Use this tool to explicitly link imports to their physical file targets, functions to their internal calls, or class inheritance/mixins."
    )]
    async fn save_edge(
        &self,
        Parameters(req): Parameters<SaveEdgeRequest>,
    ) -> Result<String, String> {
        let svc = self.clone();
        blocking(move || {
            let db_path = svc.resolve_db_path_and_watch(
                Some(req.project_root.as_str()),
                Some(&req.edge.source),
                None,
            );
             
            let db = crate::database::get_or_init_db(&db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
            let graph = lbug::Connection::new(db.as_ref()).map_err(|e| format!("Failed to create connection: {e}"))?;

            req.edge.save(&graph).map_err(|e| e.to_string())?;

            Ok(format!("Edge {} saved successfully.", req.edge.id))
        })
        .await
    }

    #[tool(
        description = r#"Parses a source file using Tree-sitter, extracts all structural nodes, and automatically adds them and their container relationships to the graph database. Call this tool if `coverage_check` shows a file is missing or out-of-date.
**Returns**: A success confirmation and a brief architectural summary of what was newly indexed."#
    )]
    async fn parse_project_file(
        &self,
        Parameters(req): Parameters<ParseFileRequest>,
    ) -> Result<String, String> {
        let svc = self.clone();
        blocking(move || {
            let db_path = svc.resolve_db_path_and_watch(
                Some(req.project_root.as_str()),
                Some(&req.file_path),
                None,
            );
             
            let db = crate::database::get_or_init_db(&db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
            let graph = lbug::Connection::new(db.as_ref()).map_err(|e| format!("Failed to create connection: {e}"))?;
            let _ = graph.query("BEGIN TRANSACTION");
            let summary =
                crate::indexer::parser::parse_file(&req.file_path, &graph).map_err(|e| {
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
        })
        .await
    }

    #[tool(
        description = "Recursively walks the graph bidirectionally from a starting node up to a specified depth (max_depth) and returns an indented relationship list. Use this tool when you want to discover the neighborhood of dependencies, callers, or subclasses of a particular node in a single call."
    )]
    async fn traverse_graph(
        &self,
        Parameters(req): Parameters<TraverseGraphRequest>,
    ) -> Result<String, String> {
        let svc = self.clone();
        blocking(move || {
            let db_path = svc.resolve_db_path_and_watch(
                Some(req.project_root.as_str()),
                None,
                Some(&req.node_id),
            );
            tracing::handle_traverse_graph(&db_path, req)
        })
        .await
    }

    #[tool(
        description = "Executes a graph query using Cypher syntax (e.g., MATCH (source)-[rel]->(target) WHERE ...) to discover patterns, links, or cross-file dependencies."
    )]
    async fn query_graph_cypher(
        &self,
        Parameters(req): Parameters<QueryGraphCypherRequest>,
    ) -> Result<String, String> {
        let svc = self.clone();
        blocking(move || {
            let db_path =
                svc.resolve_db_path_and_watch(Some(req.project_root.as_str()), None, None);
            queries::handle_query_graph_cypher(&db_path, req)
        })
        .await
    }

    #[tool(
        description = r#"Searches the graph for nodes matching a symbol name or pattern (e.g., a class, function, or file name). 
Use this tool to instantly find where a symbol is defined across the entire workspace without knowing its file path.
**Returns**: A list of matches with precise line coordinates (e.g., `[Class] app/models/user.rb:42-80: class User`)."#
    )]
    async fn search_symbols(
        &self,
        Parameters(req): Parameters<SearchSymbolsRequest>,
    ) -> Result<String, String> {
        let svc = self.clone();
        blocking(move || {
            let db_path =
                svc.resolve_db_path_and_watch(Some(req.project_root.as_str()), None, None);
            queries::handle_search_symbols(&db_path, req)
        })
        .await
    }

    #[tool(
        description = "Traces incoming or outgoing references for a specific node ID (e.g. finding callers or callees of a function). Provide the node_id and the direction ('incoming' for callers, 'outgoing' for callees)."
    )]
    async fn get_dependencies(
        &self,
        Parameters(req): Parameters<GetDependenciesRequest>,
    ) -> Result<String, String> {
        let svc = self.clone();
        blocking(move || {
            let db_path = svc.resolve_db_path_and_watch(
                Some(req.project_root.as_str()),
                None,
                Some(&req.node_id),
            );
            tracing::handle_get_dependencies(&db_path, req)
        })
        .await
    }

    #[tool(
        description = r#"Returns the structural outline of a source file (including its Classes, Methods, and Singleton Methods) by directly querying the pre-indexed graph database. 
Use this tool instead of `parse_project_file` when you only need to see what symbols are defined inside a file without incurring the heavy cost of disk reads.
**Returns**: A bulleted list of symbols in the file with line numbers (e.g., `- [Method] (lines: 10-25) class::method signature`)."#
    )]
    async fn get_file_structure(
        &self,
        Parameters(req): Parameters<GetFileStructureRequest>,
    ) -> Result<String, String> {
        let svc = self.clone();
        blocking(move || {
            let db_path = svc.resolve_db_path_and_watch(
                Some(req.project_root.as_str()),
                Some(&req.file_path),
                None,
            );
            queries::handle_get_file_structure(&db_path, req)
        })
        .await
    }

    #[tool(
        description = "Retrieves a comprehensive list of all file paths currently tracked and indexed in the knowledge graph. Agents should use this tool to discover available source code files in the workspace (such as controllers, models, or specific modules) that are ready for immediate semantic querying via `get_file_structure` or `query_graph_cypher` without needing to rely on standard terminal commands like `ls` or `find`."
    )]
    async fn list_indexed_files(
        &self,
        Parameters(req): Parameters<ListIndexedFilesRequest>,
    ) -> Result<String, String> {
        let svc = self.clone();
        blocking(move || {
            let db_path =
                svc.resolve_db_path_and_watch(Some(req.project_root.as_str()), None, None);
            queries::handle_list_indexed_files(&db_path, req)
        })
        .await
    }

    #[tool(
        description = r#"Analyzes a directory to report index staleness and coverage. Given a directory path, it recursively scans for source files and cross-references them against the graph database. 
Use this BEFORE searching when you suspect files might be missing from the graph.
**Returns**: A markdown report listing total files, indexed counts, and sample lists of missing/stale files."#
    )]
    async fn coverage_check(
        &self,
        Parameters(req): Parameters<CoverageCheckRequest>,
    ) -> Result<String, String> {
        let svc = self.clone();
        blocking(move || {
            let db_path = svc.resolve_db_path_and_watch(
                Some(req.project_root.as_str()),
                Some(&req.directory_path),
                None,
            );
            queries::handle_coverage_check(&db_path, req)
        })
        .await
    }

    #[tool(
        description = "Generates a standalone, interactive HTML map of the knowledge graph and saves it to a specified file. The HTML file uses Cytoscape.js to visually render all files, functions, classes, and their relationships (IMPORTS, CALLS, CONTAINS). Agents should use this tool when the user asks for a visual representation of the architecture or a specific folder/file."
    )]
    async fn generate_interactive_map(
        &self,
        Parameters(req): Parameters<GenerateInteractiveMapRequest>,
    ) -> Result<String, String> {
        let svc = self.clone();
        blocking(move || {
            let db_path =
                svc.resolve_db_path_and_watch(Some(req.project_root.as_str()), None, None);
            let filter = req.filter_path.unwrap_or_default();

            crate::exporter::generate_html(&db_path, &req.output_path, &filter)
                .map_err(|e| format!("Failed to generate interactive map: {e}"))?;

            Ok(format!(
                "Interactive map successfully generated and saved to: {}",
                req.output_path
            ))
        })
        .await
    }

    #[tool(
        description = r#"Retrieves the raw source code block (implementation body) of a specific symbol (e.g. class, method, or standalone function) directly from the database without reading the entire file. Use this tool when you need to inspect the actual code logic of a node found via search_symbols or get_file_structure.
**Returns**: The raw text of the code implementation block, bounded by exact line numbers."#
    )]
    async fn get_symbol_implementation(
        &self,
        Parameters(req): Parameters<GetSymbolImplementationRequest>,
    ) -> Result<String, String> {
        let svc = self.clone();
        blocking(move || {
            let db_path = svc.resolve_db_path_and_watch(
                Some(req.project_root.as_str()),
                None,
                Some(&req.node_id),
            );
            queries::handle_get_symbol_implementation(&db_path, req)
        })
        .await
    }

    #[tool(
        description = r#"Returns complete 360-degree context for a single node ID. 🌟 HIGHLY RECOMMENDED 🌟
Includes its basic properties (signature, docstring), the parent container it belongs to, its outgoing dependencies (what it calls/imports), and its incoming usages (what calls it). Use this tool instead of writing complex Cypher queries to instantly understand how a symbol fits into the codebase.
**Returns**: A markdown summary containing the exact file location, raw code snippet, and detailed lists of all incoming usages and outgoing dependencies (with file:line accuracy)."#
    )]
    async fn get_symbol_info(
        &self,
        Parameters(req): Parameters<GetSymbolInfoRequest>,
    ) -> Result<String, String> {
        let svc = self.clone();
        blocking(move || {
            let db_path = svc.resolve_db_path_and_watch(
                Some(req.project_root.as_str()),
                None,
                Some(&req.node_id),
            );
            queries::handle_get_symbol_info(&db_path, req)
        })
        .await
    }

    #[tool(
        description = r#"Traces multi-hop call paths between a specific start_node_id and end_node_id up to a max_depth. Useful for finding how a controller reaches a specific database model or service without having to call get_dependencies repeatedly.
**Returns**: A step-by-step trace showing the chain of CALLS edges between the two nodes."#
    )]
    async fn trace_call_path(
        &self,
        Parameters(req): Parameters<TraceCallPathRequest>,
    ) -> Result<String, String> {
        let svc = self.clone();
        blocking(move || {
            let db_path = svc.resolve_db_path_and_watch(
                Some(req.project_root.as_str()),
                None,
                Some(&req.start_node_id),
            );
            tracing::handle_trace_call_path(&db_path, req)
        })
        .await
    }

    #[tool(
        description = "Returns documentation about the graph schema (available node labels, relationship types, and property keys). **CRITICAL:** ALWAYS call this tool FIRST to understand the LadybugDB schema before writing ANY Cypher queries."
    )]
    async fn get_graph_schema(
        &self,
        Parameters(_req): Parameters<GetGraphSchemaRequest>,
    ) -> Result<String, String> {
        let schema = r#"
# `icnow` Knowledge Graph (LadybugDB Schema)

This graph uses **LadybugDB** and is queried via **Cypher** using the `query_graph_cypher` tool.

## Nodes
- **`File`**: Represents a source file.
  - Properties: `id` (STRING: absolute path)
- **`Symbol`**: Represents a code symbol (class, method, function, macro, struct, etc).
  - Properties: `id` (STRING: globally unique identifier, e.g., '/path/file.rb::ClassName::method_name'), `name` (STRING: short name), `kind` (STRING: e.g., 'Class', 'Method', 'Function', 'Variable'), `signature` (STRING), `docstring` (STRING), `source_code` (STRING).
- **`Memory`**: Represents an architectural concept.
  - Properties: `id` (STRING), `name` (STRING), `description` (STRING).

## Edges
- `(f:File)-[:CONTAINS]->(s:Symbol)`: A file defines a symbol.
- `(s1:Symbol)-[:DEFINES]->(s2:Symbol)`: A class/module contains a method.
- `(s1:Symbol)-[:CALLS]->(s2:Symbol)`: A symbol calls another symbol.
- `(s1:Symbol)-[:INHERITS]->(s2:Symbol)`: A class inherits from another class, or a struct implements a trait.
- `(s1:Symbol)-[:INSTANTIATES]->(s2:Symbol)`: A function/method instantiates a class or struct (e.g., via `new`).
- `(f:File)-[:IMPORTS]->(s:Symbol)`: A file imports a module/symbol.
- `(m:Memory)-[:REFERENCES]->(s:Symbol|f:File)`: A memory refers to a code symbol or file.

## Cypher Examples
- **Count all methods inside a file**: 
  `MATCH (f:File {id: '/path/file.rb'})-[:CONTAINS]->(m:Symbol {kind: 'Method'}) RETURN count(m)`
- **Find all subclasses of ApplicationRecord**: 
  `MATCH (c:Symbol {kind: 'Class'})-[:CALLS]->(p:Symbol {name: 'ApplicationRecord'}) RETURN c.id`
"#;
        Ok(schema.to_string())
    }

    #[tool(
        description = "Parses an LSIF (Language Server Index Format) dump file to extract precise definition and reference relationships across the codebase and imports them into the graph database. If no lsif_path is provided, it automatically detects the project type (Rust, Ruby, TypeScript/React) and generates the dump on the fly using standard CLI compilers."
    )]
    async fn deep_scan(
        &self,
        Parameters(req): Parameters<DeepScanRequest>,
    ) -> Result<String, String> {
        let inferred_root = Some(req.project_root.clone())
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .map(|d| d.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| ".".to_string());

        let path_buf = std::path::Path::new(&inferred_root);
        let db_path = path_buf.join("knowledge.db").to_string_lossy().to_string();
        let inferred_root_clone = inferred_root.clone();
        let db_path_clone = db_path.clone();
        let lsif_path_opt = req.lsif_path.clone();

        tokio::task::spawn_blocking(move || {
            ::tracing::info!("Starting background deep scan for {}", inferred_root_clone);

            let actual_lsif_path = match lsif_path_opt {
                Some(path) => path,
                None => match crate::indexer::scanner::auto_generate_lsif(&inferred_root_clone) {
                    Ok(generated) => generated,
                    Err(e) => {
                        ::tracing::error!("Auto-generation of LSIF failed: {}", e);
                        return;
                    }
                },
            };

            crate::PAUSE_WATCHER.store(true, std::sync::atomic::Ordering::SeqCst);
            let import_res = if actual_lsif_path == "NATIVE_AST" {
                crate::indexer::scanner::scan_directory_native(&inferred_root_clone, &db_path_clone)
            } else {
                let res = crate::indexer::scanner::parse_and_import_lsif(
                    &actual_lsif_path,
                    &db_path_clone,
                    Some(&inferred_root_clone),
                );
                let _ = std::fs::remove_file(&actual_lsif_path);
                res
            };
            crate::PAUSE_WATCHER.store(false, std::sync::atomic::Ordering::SeqCst);

            match import_res {
                Ok((nodes, edges)) => {
                    ::tracing::info!(
                        "Background LSIF/AST Import completed successfully. Nodes: {}, Edges: {}",
                        nodes,
                        edges
                    );
                    let remapped = crate::resolve_centralized_db_path(&db_path_clone);
                    if let Some(parent) = std::path::Path::new(&remapped).parent() {
                        let _ = std::fs::write(parent.join(".deep_scan_complete"), "");
                    }
                    
                    ::tracing::info!("Running fallback workspace reconciliation to catch missing files...");
                    if let Err(e) = crate::indexer::watcher::reconcile_workspace(std::path::Path::new(&inferred_root_clone), &db_path_clone) {
                        ::tracing::error!("Fallback workspace reconciliation failed: {}", e);
                    }
                }
                Err(e) => {
                    ::tracing::error!("Background Import failed: {}", e);
                }
            }
        });

        Ok(format!(
            "Deep scan has been successfully offloaded to a background task and will be performed in chunks. The semantic graph database ({db_path}) will incrementally populate over the next few minutes. You may continue using other tools concurrently without waiting."
        ))
    }

    #[tool(
        description = "Returns the current progress of a background deep scan operation. Use this to check if a previously triggered deep_scan has finished populating the graph."
    )]
    fn get_deep_scan_status(
        &self,
        Parameters(req): Parameters<GetDeepScanStatusRequest>,
    ) -> Result<String, String> {
        let db_path = self.resolve_db_path_and_watch(Some(req.project_root.as_str()), None, None);
        let parent = std::path::Path::new(&db_path).parent().unwrap_or(std::path::Path::new("."));
        
        if parent.join(".deep_scan_complete").exists() {
            return Ok("Status: 100% Complete. The graph is fully populated and ready for queries.".to_string());
        }

        let state_file = parent.join(".icnow_import_state.json");
        if let Ok(data) = std::fs::read_to_string(&state_file) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
                let nodes_imported = json["nodes_imported"].as_u64().unwrap_or(0);
                let edges_imported = json["edges_imported"].as_u64().unwrap_or(0);
                let total_nodes = json["total_nodes"].as_u64();
                let files_scanned = json["files_scanned"].as_u64();
                let total_files = json["total_files"].as_u64();

                if let (Some(scanned), Some(total)) = (files_scanned, total_files) {
                    if total > 0 {
                        let percent = (scanned as f64 / total as f64) * 100.0;
                        return Ok(format!("Status: Scanning files ({:.1}%). Files scanned: {}/{}", percent, scanned, total));
                    }
                }

                if let Some(total) = total_nodes {
                    if total > 0 {
                        let percent = (nodes_imported as f64 / total as f64) * 100.0;
                        return Ok(format!("Status: Importing graph data ({:.1}%). Nodes: {}/{}. Edges: {}.", percent, nodes_imported, total, edges_imported));
                    }
                }

                return Ok(format!("Status: Importing graph data. Nodes imported: {}. Edges imported: {}.", nodes_imported, edges_imported));
            }
        }

        if crate::IS_INDEXING.load(std::sync::atomic::Ordering::SeqCst) {
            return Ok("Status: Scan is currently initializing or running but no progress data is available yet.".to_string());
        }

        Ok("Status: No background deep scan is currently active, and no completion marker was found. The graph might be empty or partially populated.".to_string())
    }

    #[tool(
        description = "[EXPERIMENTAL] Creates or updates a memory node representing a high-level concept or business logic flow, linking it to code nodes or other memory nodes. Enforces prefix 'memory::'. For the `links` array, you DO NOT need exact node IDs! You can simply pass the exact class name (e.g. 'ApplicationController') or file name, and the server will automatically resolve it to the correct Node ID."
    )]
    async fn save_memory(
        &self,
        Parameters(req): Parameters<SaveMemoryRequest>,
    ) -> Result<String, String> {
        let svc = self.clone();
        blocking(move || {
            let db_path =
                svc.resolve_db_path_and_watch(Some(req.project_root.as_str()), None, None);
            memory::handle_save_memory(&db_path, req)
        })
        .await
    }

    #[tool(description = "Returns the current version of the icnow MCP server and the active database path.")]
    fn get_version(&self, req: Parameters<GetVersionRequest>) -> Result<String, String> {
        let db_path =
            self.resolve_db_path_and_watch(req.0.project_root.as_deref(), None, None);
        let actual_db_path = crate::database::resolve_centralized_db_path(&db_path);
        let version = env!("CARGO_PKG_VERSION").to_string();
        let res = serde_json::json!({
            "version": version,
            "db_path": actual_db_path
        });
        Ok(res.to_string())
    }

    #[tool(
        description = "[EXPERIMENTAL] Retrieves a detailed memory node, its description, associated keywords, and its connections to code files/methods and sub-concepts."
    )]
    async fn get_memory(
        &self,
        Parameters(req): Parameters<GetMemoryRequest>,
    ) -> Result<String, String> {
        let svc = self.clone();
        blocking(move || {
            let db_path =
                svc.resolve_db_path_and_watch(Some(req.project_root.as_str()), None, None);
            memory::handle_get_memory(&db_path, req)
        })
        .await
    }

    #[tool(
        description = "[EXPERIMENTAL] Searches for concepts and business logic flows using vector similarity and Ladybug. Returns matching memory nodes and their descriptions."
    )]
    async fn search_memories(
        &self,
        Parameters(req): Parameters<SearchMemoriesRequest>,
    ) -> Result<String, String> {
        let svc = self.clone();
        blocking(move || {
            let db_path =
                svc.resolve_db_path_and_watch(Some(req.project_root.as_str()), None, None);
            memory::handle_search_memories(&db_path, req)
        })
        .await
    }

    #[tool(
        description = "[EXPERIMENTAL] Lists all high-level concept memory nodes stored in the project's knowledge base."
    )]
    async fn list_memories(
        &self,
        Parameters(req): Parameters<ListMemoriesRequest>,
    ) -> Result<String, String> {
        let svc = self.clone();
        blocking(move || {
            let db_path =
                svc.resolve_db_path_and_watch(Some(req.project_root.as_str()), None, None);
            memory::handle_list_memories(&db_path, req)
        })
        .await
    }
}

pub fn clean_path(s: &str) -> String {
    if let Ok(cwd) = std::env::current_dir() {
        let mut cwd_str = cwd.to_string_lossy().to_string();
        if !cwd_str.ends_with('/') {
            cwd_str.push('/');
        }
        if s.contains(&cwd_str) {
            return s.replace(&cwd_str, "");
        }
        // Also try without trailing slash just in case
        let cwd_str_no_slash = cwd.to_string_lossy().to_string();
        if s.contains(&cwd_str_no_slash) {
            return s.replace(&cwd_str_no_slash, "");
        }
    }
    s.to_string()
}

pub fn absolute_node_id(node_id: &str) -> String {
    if node_id.starts_with('/') || (cfg!(windows) && node_id.contains(":\\")) {
        return node_id.to_string();
    }
    if let Ok(cwd) = std::env::current_dir() {
        let mut cwd_str = cwd.to_string_lossy().to_string();
        if !cwd_str.ends_with('/') {
            cwd_str.push('/');
        }
        return format!("{}{}", cwd_str, node_id);
    }
    node_id.to_string()
}

pub(crate) fn format_cypher_result(res: &mut lbug::QueryResult) -> Result<String, String> {
    let cols = res.get_column_names();
    if cols.is_empty() {
        return Ok("[]".to_string());
    }

    let mut rows = Vec::new();
    for row in res.by_ref() {
        let mut map = serde_json::Map::new();
        for (i, col) in cols.iter().enumerate() {
            let val = match &row[i] {
                lbug::Value::String(s) => serde_json::Value::String(clean_path(s)),
                lbug::Value::Int64(i) => serde_json::Value::Number((*i).into()),
                lbug::Value::Int32(i) => serde_json::Value::Number((*i).into()),
                lbug::Value::Double(f) => {
                    if let Some(n) = serde_json::Number::from_f64(*f) {
                        serde_json::Value::Number(n)
                    } else {
                        serde_json::Value::Null
                    }
                }
                lbug::Value::Bool(b) => serde_json::Value::Bool(*b),
                lbug::Value::Null(_) => serde_json::Value::Null,
                _ => serde_json::Value::String("?".to_string()),
            };
            map.insert(col.clone(), val);
        }
        rows.push(serde_json::Value::Object(map));
    }

    serde_json::to_string(&rows).map_err(|e| format!("JSON serialization failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_nodes() {
        let db_path = "test_memories.db";
        let _ = std::fs::remove_dir_all(db_path);
        let _ = std::fs::remove_file(format!("{db_path}.wal"));

        let db = crate::database::get_or_init_db(db_path).unwrap();
        let _conn = lbug::Connection::new(db.as_ref()).unwrap();
        let db2 = crate::database::get_or_init_db(db_path).unwrap();
        let graph = lbug::Connection::new(db2.as_ref()).unwrap();

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
        
        // Drop the connections to release the database locks before service calls
        drop(graph);
        drop(_conn);

        let service = GraphService {
            db_path: db_path.to_string(),
        };

        // 2. Try to save a memory node with an invalid prefix
        let err_res = service
            .save_memory(Parameters(SaveMemoryRequest {
                id: "bad_prefix::auth".to_string(),
                name: "Auth Flow".to_string(),
                description: "Flow".to_string(),
                keywords: vec![],
                links: vec![],
                link_type: None,
                project_root: "".to_string(),
            }))
            .await;
        assert!(err_res.is_err());
        assert!(err_res.unwrap_err().contains("prefix"));

        // 3. Try to save with links that don't exist
        let err_res = service
            .save_memory(Parameters(SaveMemoryRequest {
                id: "memory::auth".to_string(),
                name: "Auth Flow".to_string(),
                description: "Flow".to_string(),
                keywords: vec![],
                links: vec!["src/non_existent.rs".to_string()],
                link_type: None,
                project_root: "".to_string(),
            }))
            .await;
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
                project_root: "".to_string(),
            }))
            .await
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
                project_root: "".to_string(),
            }))
            .await
            .unwrap();
        assert!(ok_res2.contains("saved successfully"));

        // Query the memory and verify that the target was resolved to absolute path
        let get_res2 = service
            .get_memory(Parameters(GetMemoryRequest {
                id: "memory::relative_test".to_string(),
                project_root: "".to_string(),
            }))
            .await
            .unwrap();
        assert!(get_res2.contains("Relative Path Test"));
        assert!(get_res2.contains(&abs_file_path));

        // 5. Query the memory
        let get_res = service
            .get_memory(Parameters(GetMemoryRequest {
                id: "memory::auth".to_string(),
                project_root: "".to_string(),
            }))
            .await
            .unwrap();
        assert!(get_res.contains("Auth Flow"));
        assert!(get_res.contains("JWT token validation"));
        assert!(get_res.contains("Connected Code Elements"));
        assert!(get_res.contains("src/main.rs"));

        // 6. Test list_memories
        let list_res = service
            .list_memories(Parameters(ListMemoriesRequest {
                project_root: "".to_string(),
            }))
            .await
            .unwrap();
        assert!(list_res.contains("Auth Flow"));
        assert!(list_res.contains("memory::auth"));

        // 7. Test FTS5 search_memories
        let search_res = service
            .search_memories(Parameters(SearchMemoriesRequest {
                query: "jwt token".to_string(),
                project_root: "".to_string(),
            }))
            .await
            .unwrap();
        assert!(search_res.contains("Auth Flow"));
        assert!(search_res.contains("memory::auth"));

        // Test search with prefix or non-alphanumeric chars
        let search_res2 = service
            .search_memories(Parameters(SearchMemoriesRequest {
                query: "oauth".to_string(),
                project_root: "".to_string(),
            }))
            .await
            .unwrap();
        assert!(search_res2.contains("Auth Flow"));

        let _ = std::fs::remove_dir_all(db_path);
        let _ = std::fs::remove_file(format!("{db_path}.wal"));
    }
}
