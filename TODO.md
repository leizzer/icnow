# Project TODOs & Future Ideas

This document tracks upcoming features, improvements, and architectural ideas to implement in the `icnow` codebase knowledge graph.

## MUST IMPLEMENT

- [x] **`search_symbols` (Fuzzy / Pattern Search):** A high-level search tool to query the database for nodes matching a symbol name or pattern (e.g. searching for a class or function name). This resolves the bootstrap/discovery problem for agents.
- [ ] **`get_dependencies` (Call Graph & References):** A single-call traversal tool to trace either callers (incoming references) or callees (outgoing calls) for a specific node ID without requiring complex raw SQL or Cypher.
- [ ] **`get_file_structure` (Database File Outline):** Directly queries the database to return the structural components (methods, classes, imports) of an already-indexed file. Faster than re-parsing the file from disk.
- [ ] **`list_indexed_files` (Workspace Inventory):** Lists all files currently indexed in the graph database so agents can instantly check what parts of the workspace are ready to query.

## Parsing & Language Support
- [ ] **Extract Edges via Tree-Sitter:** Currently, we extract `Struct` and `Function` nodes. The next step is to analyze the AST for method calls or struct instantiations and map those as `CALLS` or `USES` edges.
- [ ] **Database Optimization (Byte Ranges):** Migrate from storing raw `source_code` chunks inside the graph database to extracting Tree-sitter byte ranges (e.g., `start_byte`, `end_byte`). The query tool will then stream the code directly from disk to keep the database size incredibly optimized.
- [ ] **Multi-Language Support:** Expand `src/parser.rs` beyond Rust. Add grammar crates like `tree-sitter-typescript`, `tree-sitter-python`, and `tree-sitter-ruby`.
- [ ] **Directory Parsing:** Upgrade the `parse_project_file` tool to `parse_directory` so it can recursively walk a project and ingest all files in one go.

## Alternative Graph Extraction (The "Offline LSP" Route)
- [ ] **LSIF Importer:** Build a tool that reads a standard `.lsif` (Language Server Index Format) JSON dump and directly imports the highly-accurate, pre-resolved LSP references into the `graphqlite` database.

## MCP Server Enhancements
- [ ] **Query Tool:** Expose a new tool to the MCP server that allows AI agents to directly run Cypher queries against the database (e.g., `query_graph("MATCH (n)-[r]->(m) RETURN n, r, m")`) so the agent can read the knowledge it has saved.
- [ ] **Graph Context Tool:** Provide a tool that takes a specific file or node ID and automatically returns its immediate neighbors (e.g., "What uses this Struct?").

## Visualization
- [ ] **Interactive UI:** Replace or augment the `export_graph.sh` Graphviz script with a lightweight web view (like `vis.js` or Mermaid) that serves the graph interactively on a local port.
