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

**`mcp__icnow__search_symbols`** - **START HERE** for finding where something is defined. Searches for symbols by name/pattern across the entire workspace without knowing the file path.
```javascript
mcp__icnow__search_symbols(query: "UserController", limit: 10)
```

**`mcp__icnow__get_file_structure`** - Returns a structural outline of a file (Classes, Methods) from the graph database. **More efficient than reading the file** when you only need to see what symbols are defined. Returns NodeIDs you can pass to `get_dependencies`.
```javascript
mcp__icnow__get_file_structure(file_path: "src/main.rs")
```

**`mcp__icnow__get_dependencies`** - Traces incoming (callers) or outgoing (callees) references for a specific node.
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

1. **Find the Symbol**: Use `search_symbols` to find the NodeID and file path of a class or method.
2. **Inspect the File**: Use `get_file_structure` to see what else lives in that file.
3. **Trace Usage**: Use `get_dependencies` to see where a method is called across the codebase.
4. **Visual Maps**: If the user wants to "see" the structure, use `generate_interactive_map`.
5. **Fallback**: Only fall back to standard `read_file` if you specifically need the raw implementation details of a function body that aren't exposed in the graph.

---

## 📊 Call Tracking (Optional Logging)
Every time you call an `icnow` tool, you must track it in a daily CSV file at `/tmp/{day}-{month}-{year}_icnow.csv`. Append a row with the format: `call, tool_name, success, enough`.
- `success`: Was the execution technically successful?
- `enough`: Did the graph provide enough context to prevent you from using native grep/read?
