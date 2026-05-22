# `icnow` Tooling Roadmap

This document outlines the approved new tools and enhancements for the `icnow` codebase to optimize it for LLM agents.

## 🛠️ New Tools to Implement

### 1. `get_symbol_implementation(node_id: String)`
* **Description**: Retrieve the raw source code block (implementation body) of a specific symbol (e.g., class, method, or standalone function) without reading the entire file.
* **Implementation details**: 
  - Save source code ranges or direct byte strings in the database when parsing in `src/parser.rs`.
  - Expose a new MCP tool in `src/tools.rs` to fetch this.

### 2. `trace_call_path(start_node_id: String, end_node_id: String)`
* **Description**: Trace multi-hop call paths (e.g., Controller -> Service -> Model) in a single invocation.
* **Implementation details**:
  - Write a recursive Cypher query like `MATCH p = (start)-[:CALLS*1..5]->(end) ...` in `src/tools.rs`.

### 3. `get_graph_schema()`
* **Description**: Provide documentation about the graph schema (available node labels, relationship types, and property keys).
* **Why**: Helps agents construct valid Cypher queries without guessing.

---

## 📈 Improvements to Existing Tools

### 1. Rich Search & Filtering in `search_symbols`
* Add optional filters for node labels (e.g., search only classes, or search only methods).
* Integrate fuzzy search (potentially via SQLite `FTS5` or simple distance matching) to allow typos.

### 2. Hierarchical Outlines in `get_file_structure`
* Change flat table outputs to nested outlines showing which methods belong to which classes.

### 3. SQLite Batching/Transactions in Parser
* Implement proper SQL transaction batching during indexing in `src/parser.rs` to avoid locking the SQLite database during high-frequency inserts (e.g., `CALL` nodes). Re-enable call extraction once fixed.
