# `icnow` Tooling Roadmap

This document outlines the approved new tools and enhancements for the `icnow` codebase to optimize it for LLM agents.

## 🛠️ New Tools to Implement

### [x] 1. `get_symbol_implementation(node_id: String)`
* **Description**: Retrieve the raw source code block (implementation body) of a specific symbol (e.g., class, method, or standalone function) without reading the entire file.
* **Implementation details**: 
  - Save source code ranges or direct byte strings in the database when parsing in `src/parser.rs`.
  - Expose a new MCP tool in `src/tools.rs` to fetch this.

### [x] 2. `trace_call_path(start_node_id: String, end_node_id: String)`
* **Description**: Trace multi-hop call paths (e.g., Controller -> Service -> Model) in a single invocation.
* **Implementation details**:
  - Write a recursive Cypher query like `MATCH p = (start)-[:CALLS*1..5]->(end) ...` in `src/tools.rs`.

### [x] 3. `get_graph_schema()`
* **Description**: Provide documentation about the graph schema (available node labels, relationship types, and property keys).
* **Why**: Helps agents construct valid Cypher queries without guessing.

---

## ⚡ Improvements to Existing Tools

### [x] 1. `search_symbols(query, limit, kind_filter)`
* **Enhancement**: Add a `kind_filter: Option<Vec<String>>` argument (e.g., `["Class", "Method"]`).
* **Why**: LLMs often search for "User" and get overwhelmed by `File`, `Function`, `Variable`, and `Unresolved` nodes. Filtering makes the tool incredibly precise.

### [x] 2. `get_file_structure(file_path)`
* **Enhancement**: Instead of returning a raw Cypher dump or flat list, recursively format it into an organized markdown tree (e.g., `File -> Class -> Methods`).
* **Why**: LLMs process hierarchical semantic representations far better than flat relational tables.

### [x] 3. Fix SQLite Lock Errors (`database is locked (5)`)
* **Enhancement**: In `src/parser.rs`, accumulating nodes/edges and inserting them in a batch transaction using `graphqlite`'s bulk insertion APIs, instead of autocommitting thousands of times per file.
* **Why**: This prevents `icnow` from locking up the shared macOS directory during parsing and allows us to re-enable `CALL` node extraction safely.
