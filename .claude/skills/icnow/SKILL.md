---
name: icnow
description: Triggers whenever the agent is working with source code files (e.g., Ruby, TypeScript, React, JavaScript, Rust), needs to find symbol definitions, trace method/function call paths, inspect code blocks, understand codebase architecture, or query the semantic knowledge graph.
---

# `icnow` Semantic Graph Skill

This skill outlines how to interact with the `icnow` MCP server to navigate the codebase using a semantic graph database. Utilizing `icnow` tools minimizes token usage, accelerates code navigation, and provides high-level architectural insight far superior to traditional file reads or searches.

---

## 🛑 STRICT TOOLING RULES: USE ICNOW FIRST

The `icnow` MCP server hosts a graph representation of this repository. 
**DO NOT** use `grep`, `find`, or read entire files to find definitions, outline structure, or trace calls. **YOU MUST USE `icnow` TOOLS FIRST.** 

### 🏆 Benchmark Results & Token Savings
Recent benchmarks show where `icnow` shines and where it falls short:
1. **✅ WINS for structural queries (95-98% savings):** Method counts, class listings, file structures, finding definitions. Avoids reading full files. Average savings when it works is **80% token reduction**.
2. **❌ LOSES for simple text searches (~50% worse):** Do not use `icnow` for pattern matching like `"belongs_to"`, `"def method_name"`, etc. Use `grep` or traditional search for these.

### When to Use icnow:
- **Finding Definitions**: "Find definition of symbol X" or "Where is `User` model or `SessionsController` defined?" -> Use `search_symbols`
- **File Outlines**: "Show me the structure of this file" or "What classes exist in this app?" -> Use `get_file_structure`
- **Tracing Calls**: "What function invokes `verify_webhook_signature`?" -> Use `get_dependencies` or `trace_call_path`
- **Structural Cypher Queries**: "How many methods does X have?" or "List all methods in class `User` that call `send_email`" -> Use `query_graph_cypher`
- **Visualizing**: "Show me a diagram/map of `app/services`" -> Use `generate_interactive_map`
- **Concept Memory**: "Document/retrieve architectural concepts, entry points, business rules, or code summaries to reduce token usage and share knowledge" -> Use `save_memory`, `get_memory`, `search_memories`, or `list_memories`

### When to Use grep / Traditional Search:
- Simple text pattern matching (e.g., `"belongs_to"`, `"has_many"`, `"def method_name"`).
- "Find all files containing X string".
- Reading/writing non-source code files (Markdown, JSON, Configs, YAML). Use standard read tools.
- When you already know the exact file path and need to read the *entire* file body.

### ⚠️ DO NOT GENERALIZE FAILURES:
- **Never assume complete failure**: If an `icnow` query fails, returns empty results, or lacks context for a *specific* lookup, do NOT assume the tool or database is broken for subsequent lookups.
- **Always try `icnow` first for structural queries**: Treat each code investigation task as independent. You must *always* attempt to use `icnow` first for structural tasks, even if a previous `icnow` call in the same session fell back to native search.

---

## 🧠 CRITICAL MEMORY NODE RULES: HIGH-LEVEL CONCEPTS ONLY

Memory nodes must **only** represent **major, high-level architectural or domain concepts** (e.g., `'payment'`, `'authentication'`, `'post review'`, `'publishing'`). 

### 🚫 DO NOT:
- Create memories for stupid, small details, individual bugs, temporary features, or single helper methods.
- Save granular, low-level elements that require constant maintenance on every minor code change.
- Waste database space with trivial or boilerplate code notes.

### ✅ DO:
- **Focus on Big Concepts**: Only create memories for broad, domain-level boundaries that help kickstart future work (e.g., when a user says "I need to change the payment process" or "I must add a new gateway", the agent should immediately fetch context from `icnow`).
- **Link Key Anchors**: Link memory nodes to high-level entry points, core flow files, main classes/controllers/models, or other sub-concept memory nodes that support the overall idea.
- **Maintain Stability**: Ensure the descriptions and summaries are high-level enough to remain stable and correct over time without needing constant modification.

### Kickstarting Workflow:
1. When starting a task in a major functional area (like Payments), **always** call `search_memories` or `list_memories` first to pull the domain map, entry points, and high-level context.
2. Only write a new memory when introducing a brand new major domain or when a significant existing system concept is undocumented.

---

## 💡 Token-Saving Multi-Tool Concatenation Guide

To prevent exhausting context limits and avoid reading full files, leverage tool concatenation to fetch **only** the precise information needed:

### 1. The Navigation Loop: Search -> Target Implementation
*   **Goal**: Understand a method or class logic without reading the entire file.
*   **Workflow**:
    1. Call `search_symbols(query: "...", kind_filter: ["Class", "Method"])` to retrieve the exact Node ID.
    2. Immediately call `get_symbol_implementation(node_id: "...")` using the matched ID.
*   **Token Savings**: **90-95%**. Pulls in 10-30 lines of targeted code instead of a 300+ line file.

### 2. The Structural Probe: Outline File -> Target Methods
*   **Goal**: Inspect a file's public interface and read specific methods of interest.
*   **Workflow**:
    1. Call `get_file_structure(file_path: "...")` to obtain a hierarchical list of defined classes and methods.
    2. Selectively call `get_symbol_implementation(node_id: "...")` *only* on the specific method nodes needed.
*   **Token Savings**: **70-80%**. Avoids reading imports, docstrings, helper boilerplate, and unrelated method implementations.

### 3. The Call-Chain Discovery: Trace Calls -> Snippet Inspection
*   **Goal**: Trace how data flows from an entrypoint (e.g., Controller callback) to a database operation.
*   **Workflow**:
    1. Call `trace_call_path(start_node_id: "...", end_node_id: "...")` (or run a Cypher query) to map out intermediate calls.
    2. Inspect the bodies of the intermediate function nodes using `get_symbol_implementation` for the specific call site.
*   **Token Savings**: **80-90%**. Avoids opening and reading 4-5 files along the dependency path.

### 4. Cypher Performance Best Practices on LadybugDB
The database backend is powered by LadybugDB (Kùzu). Standard SQL queries are deprecated and not supported. Always use `query_graph_cypher` to execute custom Cypher queries.

*   **Node Labels**: There are exactly three node tables: `:Symbol`, `:File`, and `:Memory`. Do not use labels like `:Class`, `:Method`, or `:Unresolved`. Instead, filter on the `kind` property.
    *   *Example: Find all methods in class `User`*
        `MATCH (c:Symbol {kind: "Class", name: "User"})-[:HAS_METHOD]->(m:Symbol) WHERE m.kind = "Method" RETURN m.name ORDER BY m.name`
*   **Case Insensitivity**: Cypher `CONTAINS` searches are case-sensitive by default. To perform case-insensitive text searches, use `toLower(property) CONTAINS "search_term"` (with the search term in lowercase).
    *   *Example: Find all memory nodes containing 'auth' in name or description*
        `MATCH (m:Memory) WHERE toLower(m.name) CONTAINS 'auth' OR toLower(m.description) CONTAINS 'auth' RETURN m.id, m.name`
*   **Relationship Types**: Relationship tables are `REL_CONTAINS` (File to Symbol, Symbol to Symbol), `CALLS` (Symbol to Symbol), `HAS_METHOD` (Symbol to Symbol), `LINKS_TO` (Memory to Memory/Symbol/File), and `IMPORTS` (File to File).

---

## 🛠 Available Tools

1.  **`search_symbols(query: String, limit: Option<u32>, kind_filter: Option<Vec<String>>)`**  
    Searches the graph for nodes matching a symbol name or pattern. Use `kind_filter: ["Class"]` or `["Method"]` to reduce noise.
2.  **`get_symbol_implementation(node_id: String)`**  
    Retrieves the raw source code block of a specific symbol directly from the database.
3.  **`get_file_structure(file_path: String)`**  
    Returns a hierarchical outline of a file.
4.  **`trace_call_path(start_node_id: String, end_node_id: String)`**  
    Traces multi-hop call paths between a specific start and end node.
5.  **`get_dependencies(node_id: String, direction: String)`**  
    Traces immediate incoming (`incoming` for callers) or outgoing (`outgoing` for callees) references.
6.  **`query_graph_cypher(query: String)`**  
    Executes a graph query using Cypher syntax.
7.  **`generate_interactive_map(output_path: String, filter_path: Option<String>)`**  
    Generates a standalone, interactive HTML Cytoscape map.
8.  **`list_indexed_files()`**  
    Lists all files tracked in the knowledge graph.
9.  **`parse_project_file(file_path: String)`**  
    Parses a file and adds it to the graph. Only call if the file is new or recently modified heavily.
10. **`save_memory(id: String, name: String, description: String, keywords: Vec<String>, links: Vec<String>, link_type: Option<String>, project_root: Option<String>)`**  
    Saves or updates a high-level concept memory node. Enforces `memory::` prefix on `id`. Validates that all items in `links` (file paths, method/class/function IDs, or other memory IDs) already exist in the graph database.
11. **`get_memory(id: String, project_root: Option<String>)`**  
    Retrieves the properties of a specific memory node along with all its direct links and neighborhood concepts.
12. **`search_memories(query: String, project_root: Option<String>)`**  
    Searches memory nodes matching the text query case-insensitively across name, description, and keywords using Cypher.
13. **`list_memories(project_root: Option<String>)`**  
    Lists all memory nodes stored in the database for a high-level overview of codebase concepts.

---

## 📊 Call Tracking (Mandatory Logging)

Every time you call an `icnow` tool, you must track it in a daily CSV file at `/tmp/{day}-{month}-{year}_icnow.csv`. Append a row with the format: `call, tool_name, success, enough, why_not_enough, target_information`.
-   `success`: Was the execution technically successful? (`true`/`false`)
-   `enough`: Did the graph provide enough context to prevent you from using native grep/read? (`true`/`false`/`pending`)
-   `why_not_enough`: Explain exactly why `icnow` was not enough and you had to fall back (e.g., "Empty search result", "Missing CALLS edge"). Use `"N/A"` if enough is true.
-   `target_information`: Describe what information you were trying to find in the database (e.g., "Definition of method call_webhook").
