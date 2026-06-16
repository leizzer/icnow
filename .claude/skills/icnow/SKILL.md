---
name: icnow
description: Triggers whenever the agent is working with source code files (e.g., Ruby, TypeScript, React, JavaScript, Rust), needs to find symbol definitions, trace method/function call paths, inspect code blocks, understand codebase architecture, or query the semantic knowledge graph.
---

# `icnow` Semantic Graph Skill

This skill outlines how to interact with the `icnow` MCP server to navigate the codebase using a semantic graph database. Utilizing `icnow` tools minimizes token usage, accelerates code navigation, and provides high-level architectural insight far superior to traditional recursive grepping.

---

## 🛑 STRICT TOOLING RULES: THE HYBRID APPROACH IS GOLD STANDARD

**DO NOT** use `grep -r` blindly across the entire project to find definitions or references. **YOU MUST USE `icnow` TOOLS TO LOCATE THE TARGET.** 

However, do **NOT** stubbornly force pure Cypher string-slicing if it gets complicated. The ultimate benchmarked sweet-spot is the **Hybrid Approach**:
1. Use `icnow` (`search_symbols`, `get_symbol_info`, `get_file_structure`) to instantly find the exact filepath and node ID.
2. If you need to read the full file body, use the traditional native tool `view_file` on the exact path found by `icnow`. 

### 🏆 Benchmark Results & Token Savings
Recent benchmarks prove:
1. **✅ WINS for structural queries (95-98% savings):** Locating files, finding callers/callees via `get_symbol_info`, tracing dependencies. Average savings is an **80% token reduction**.
2. **❌ LOSES for pure text extraction:** Forcing Cypher `SUBSTR()` queries to extract exact multi-line methods is brittle and prone to syntax errors. Once you know the file path via `icnow`, just use `view_file`.
3. **❌ LOSES for simple text searches:** Do not use `icnow` for raw string pattern matching like `"belongs_to"`. Use traditional `grep_search` within a directory.

---

## 💡 The "360-Degree Context" Workflow

Instead of writing complex Cypher queries to understand how a method or class fits into the codebase, rely on the dedicated `get_symbol_info` tool:

1. **Locate the ID:** Call `search_symbols(query: "authenticate_user")` to find the node ID (e.g., `app/controllers/application.rb::authenticate_user`).
2. **Get 360-Degree Context:** Immediately call `get_symbol_info(node_id: "...")`. 
   - This returns a beautifully formatted markdown summary showing:
     - The exact docstring and signature
     - The parent file/class container
     - **Incoming Usages:** Every file/method that calls *into* this node.
     - **Outgoing Dependencies:** Every method/import this node calls *out* to.
3. **Read Code (Optional):** If you need the exact implementation body, call `view_file` on the file path, or `get_symbol_implementation` for just that block.

---

## 🧠 CRITICAL MEMORY NODE RULES: HIGH-LEVEL CONCEPTS ONLY

Memory nodes must **only** represent **major, high-level architectural or domain concepts** (e.g., `'payment'`, `'authentication'`, `'post review'`). 

### 🚫 DO NOT:
- Create memories for small details, individual bugs, temporary features, or single helper methods.
- Save granular, low-level elements that require constant maintenance.

### ✅ DO:
- **Focus on Big Concepts**: Only create memories for broad, domain-level boundaries that help kickstart future work.
- **Link Key Anchors**: Link memory nodes to high-level entry points or core flow files.
- **Kickstarting Workflow**: When starting a task in a major functional area, **always** call `search_memories` or `list_memories` first to pull the domain map.

---

## 🛠 Available Tools

1.  **`search_symbols(query: String, limit: Option<u32>, kind_filter: Option<Vec<String>>)`**  
    Searches the graph for nodes matching a symbol name or pattern. Use `kind_filter: ["Class"]` or `["Method"]` to reduce noise.
2.  **`get_symbol_info(node_id: String)`** 🌟 **HIGHLY RECOMMENDED** 🌟 
    Returns complete 360-degree context for a single node ID. Includes its basic properties (signature, docstring), the parent container it belongs to, its outgoing dependencies (what it calls/imports), and its incoming usages (what calls it). Use this tool instead of writing complex Cypher queries.
3.  **`get_symbol_implementation(node_id: String)`**  
    Retrieves the raw source code block of a specific symbol directly from the database.
4.  **`get_file_structure(file_path: String)`**  
    Returns a hierarchical outline of a file.
5.  **`get_dependencies(node_id: String, direction: String)`**  
    Traces immediate incoming (`direction: "incoming"` for callers) or outgoing (`direction: "outgoing"` for callees) references. Note: `get_symbol_info` is usually better as it does both at once.
6.  **`trace_call_path(start_node_id: String, end_node_id: String)`**  
    Traces multi-hop call paths between a specific start and end node.
7.  **`query_graph_cypher(query: String)`**  
    Executes a graph query using Cypher syntax. Only use this for complex multi-hop logic that the predefined tools cannot handle.
8.  **`generate_interactive_map(output_path: String, filter_path: Option<String>)`**  
    Generates a standalone, interactive HTML Cytoscape map for visual representations.
9.  **`list_indexed_files()`**  
    Lists all files tracked in the knowledge graph.
10. **`parse_project_file(file_path: String)`**  
    Parses a file and adds it to the graph. Only call if the file is new or recently modified heavily.
11. **`save_memory(id: String, name: String, description: String, keywords: Vec<String>, links: Vec<String>, link_type: Option<String>, project_root: Option<String>)`**  
    Saves or updates a high-level concept memory node.
12. **`get_memory(id: String, project_root: Option<String>)`**  
    Retrieves the properties of a specific memory node along with all its direct links.
13. **`search_memories(query: String, project_root: Option<String>)`**  
    Searches memory nodes matching the text query.
14. **`list_memories(project_root: Option<String>)`**  
    Lists all memory nodes stored in the database.

---

## 📊 Call Tracking (Mandatory Logging)

Every time you call an `icnow` tool, you must track it in a daily CSV file at `/tmp/{day}-{month}-{year}_icnow.csv`. Append a row with the format: `call, tool_name, success, enough, why_not_enough, target_information`.
-   `success`: Was the execution technically successful? (`true`/`false`)
-   `enough`: Did the graph provide enough context to prevent you from using native grep/read? (`true`/`false`/`pending`)
-   `why_not_enough`: Explain exactly why `icnow` was not enough and you had to fall back. Use `"N/A"` if enough is true.
-   `target_information`: Describe what information you were trying to find in the database.
