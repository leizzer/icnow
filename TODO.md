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

### [ ] 4. Semantic Vector Search (Embeddings)
* **Description**: Generate vector embeddings for node `source_code` or `docstring` (using the `fastembed` crate) and expose a `semantic_search` MCP tool.
* **Why**: Allows agents to find methods and classes by semantic meaning (e.g., "Find methods related to processing Stripe payments") rather than relying purely on exact-match symbol searches.

### [ ] 5. Codebase Architecture Onboarding (`get_architecture_overview`)
* **Description**: Create a tool that runs graph centrality algorithms (like degree counting or PageRank) to find the most highly referenced nodes in the codebase and returns a condensed markdown summary.
* **Why**: When an agent starts a new task in a massive codebase, reading file structures is slow. This provides immediate orientation by identifying the core models and controllers that most of the application depends on.

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

## 🔗 Missing DEPENDS_ON Opportunities

### [x] 1. TypeScript/JavaScript `implements` clauses
* **Description**: Capture `implements_clause` as `DEPENDS_ON` edges for classes implementing interfaces.
* **Why**: The `extends_clause` is currently captured, but `implements` (which enforces a dependency on an interface structure) is missing from the tree-sitter queries.

### [x] 2. Python Class Fields
* **Description**: Capture `DEPENDS_ON` for class fields (e.g., `dataclasses` and `pydantic` structural dependencies).
* **Why**: Modern Python relies on type hints defined at the class level (`user: UserInfo = None`), which are not currently captured.

### [x] 3. Go Type Aliases and Composites
* **Description**: Capture `DEPENDS_ON` for type aliases and composited declarations (e.g., `type HandlerFunc func(...)`).
* **Why**: This will capture explicit type composition dependencies beyond standard struct fields and function signatures.
