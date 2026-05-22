# Project TODOs & Future Ideas

This document tracks upcoming features, improvements, and architectural ideas to implement in the `icnow` codebase knowledge graph.

## MUST IMPLEMENT

- [x] **`search_symbols` (Fuzzy / Pattern Search):** A high-level search tool to query the database for nodes matching a symbol name or pattern (e.g. searching for a class or function name). This resolves the bootstrap/discovery problem for agents.
- [x] **`get_dependencies` (Call Graph & References):** A single-call traversal tool to trace either callers (incoming references) or callees (outgoing calls) for a specific node ID without requiring complex raw SQL or Cypher.
- [x] **`get_file_structure` (Database File Outline):** Directly queries the database to return the structural components (methods, classes, imports) of an already-indexed file. Faster than re-parsing the file from disk.
- [x] **`list_indexed_files` (Workspace Inventory):** Lists all files currently indexed in the graph database so agents can instantly check what parts of the workspace are ready to query.
- [ ] **Interactive UI (Visualization):** Replace or augment the `export_graph.sh` Graphviz script with a lightweight web view (like `vis.js` or Mermaid) that serves the graph interactively on a local port.

## Parsing & Language Support
- [x] **Extract Edges via Tree-Sitter:** The `CALLS` edges are already fully implemented! Tree-sitter actively analyzes function bodies and maps internal `CALLS` edges between methods. (Future sub-task: implement `USES` edges for struct instantiations/type dependencies).
- [ ] **Database Optimization (Byte Ranges):** Migrate from storing raw `source_code` chunks inside the graph database to extracting Tree-sitter byte ranges (e.g., `start_byte`, `end_byte`). The query tool will then stream the code directly from disk to keep the database size incredibly optimized.
- [ ] **Multi-Language Support:** Expand `src/parser.rs` beyond Rust. Add grammar crates like `tree-sitter-typescript`, `tree-sitter-python`, and `tree-sitter-ruby`.
- [x] **Directory Parsing:** Already fully implemented by the `src/watcher.rs` background daemon! When a workspace is connected, `reconcile_workspace` recursively walks the project and ingests all missing or modified files in one go without manual agent intervention.

## Alternative Graph Extraction
- [ ] **LSP/LSIF-like Intelligence without Host Dependencies:** Find a way to replicate the highly-accurate, pre-resolved references typically found in `.lsif` (Language Server Index Format) dumps, but *without* requiring the user to have a Language Server (LSP) installed on their computer. This could involve embedding a lightweight standalone static analyzer or generating LSIF-like edges dynamically.
## MCP Server Enhancements
- [x] **Query Tool:** Expose a new tool to the MCP server that allows AI agents to directly run Cypher queries against the database (e.g., `query_graph_cypher`).
- [x] **Graph Context Tool:** Provide a tool that takes a specific file or node ID and automatically returns its immediate neighbors (implemented via `traverse_graph`).


