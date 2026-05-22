# LLM Agent Instructions for icnow

This file contains instructions to inject into your LLM's system prompt or workspace context (e.g., `.cursorrules`, `claude.json`, or `.windsurfrules`). This will force the LLM to utilize the semantic knowledge graph instead of relying on inefficient native search tools.

---

## 🛑 STRICT TOOLING RULES: USE ICNOW FIRST

The `icnow` MCP server provides a pre-parsed, semantic knowledge graph of this codebase. It indexes Ruby, TypeScript, JavaScript, Rust, and other source files into a graph database. 

**DO NOT** use `grep`, `find`, or read entire files when you need to understand architecture, trace method calls, or find definitions. **YOU MUST USE `icnow` TOOLS INSTEAD.** Reading full files consumes too much context and `grep` is unreliable.

### When to Use icnow:
- **Finding Definitions**: "Where is `UserController` defined?" -> Use `search_symbols`
- **File Outlines**: "What methods are in `user.rb`?" -> Use `get_file_structure`
- **Tracing Calls**: "What calls `authenticate`?" -> Use `get_dependencies`
- **Visualizing**: "Show me a map of `app/models`" -> Use `generate_interactive_map`

### When NOT to Use icnow:
- Non-source code files (Markdown, JSON, Configs). Use standard read tools.
- When you already know the exact file path and need to read the *entire* file body.

---

## 🛠 Available Tools

**`mcp__icnow__get_graph_schema`** - Returns documentation about the graph schema (available node labels, relationship types, and property keys). Useful to understand what data exists before writing queries.
```javascript
mcp__icnow__get_graph_schema()
```

**`mcp__icnow__search_symbols`** - **START HERE** for finding where something is defined. Searches for symbols by name/pattern across the workspace. Use `kind_filter` to reduce noise.
```javascript
mcp__icnow__search_symbols(query: "UserController", limit: 10, kind_filter: ["Class"])
```

**`mcp__icnow__get_symbol_implementation`** - Retrieves the raw source code block (implementation body) of a specific symbol directly from the graph database without needing to use standard file read tools.
```javascript
mcp__icnow__get_symbol_implementation(node_id: "src/controllers/user.rb::UserController")
```

**`mcp__icnow__get_file_structure`** - Returns a hierarchical structural outline of a file (e.g. File -> Class -> Methods). **More efficient than reading the file** when you only need to see what symbols are defined.
```javascript
mcp__icnow__get_file_structure(file_path: "src/main.rs")
```

**`mcp__icnow__trace_call_path`** - Traces multi-hop call paths between a specific start node and end node. Returns the exact chain of calls connecting them up to a max_depth.
```javascript
mcp__icnow__trace_call_path(start_node_id: "src/api.rs::endpoint", end_node_id: "src/db.rs::save")
```

**`mcp__icnow__get_dependencies`** - Traces immediate incoming (callers) or outgoing (callees) references for a specific node.
```javascript
// Find what calls this method (incoming)
mcp__icnow__get_dependencies(node_id: "src/main.rs::main", direction: "incoming")
```

**`mcp__icnow__generate_interactive_map`** - Generates an interactive Cytoscape HTML map of the graph. Use this whenever the user asks for a visual representation.
```javascript
mcp__icnow__generate_interactive_map(output_path: "architecture.html", filter_path: "src/models")
```

**`mcp__icnow__list_indexed_files`** - Lists all files tracked in the knowledge graph.
```javascript
mcp__icnow__list_indexed_files()
```

**`mcp__icnow__query_graph_cypher`** - Executes a graph query using Cypher syntax. **Prefer this over SQL** for high-level semantic lookups and pattern matching.
```cypher
MATCH (c:Class {name: "User"})-[:HAS_METHOD]->(m:Method) RETURN m.name ORDER BY m.name
```

**`mcp__icnow__parse_project_file`** - Parses a file and adds it to the graph. Only call this if the file is new or recently modified heavily.

---

## 🔄 Recommended Workflow

1. **Find the Symbol**: Use `search_symbols` (with `kind_filter` if possible) to find the NodeID of a class or method.
2. **Read the Code**: Use `get_symbol_implementation` to read the exact function or class body directly from the database!
3. **Inspect the File**: Use `get_file_structure` to see what else lives in that file hierarchically.
4. **Trace Usage**: Use `trace_call_path` or `get_dependencies` to see how components connect across the codebase.
5. **Visual Maps**: If the user wants to "see" the structure, use `generate_interactive_map`.
6. **Fallback**: Only fall back to standard `read_file` or `grep` if `icnow` fails to provide the answer.

---

## 📊 Call Tracking (Optional Logging)
Every time you call an `icnow` tool, you must track it in a daily CSV file at `/tmp/{day}-{month}-{year}_icnow.csv`. Append a row with the format: `call, tool_name, success, enough`.
- `success`: Was the execution technically successful?
- `enough`: Did the graph provide enough context to prevent you from using native grep/read?
