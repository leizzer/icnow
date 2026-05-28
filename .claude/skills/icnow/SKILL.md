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

### When to Use icnow:
- **Finding Definitions**: "Where is `User` model or `SessionsController` defined?" $\rightarrow$ Use `search_symbols`
- **File Outlines**: "What classes or methods are inside `user.rb`?" $\rightarrow$ Use `get_file_structure`
- **Tracing Calls**: "What function invokes `verify_webhook_signature`?" $\rightarrow$ Use `get_dependencies` or `trace_call_path`
- **Visualizing**: "Show me a diagram/map of `app/services`" $\rightarrow$ Use `generate_interactive_map`
- **Architectural Queries**: "List all methods in class `User` that call `send_email`" $\rightarrow$ Use `query_graph_cypher`

### When NOT to Use icnow:
- Reading/writing non-source code files (Markdown, JSON, Configs, YAML). Use standard read tools.
- When you already know the exact file path and need to read the *entire* file body.

### ⚠️ DO NOT GENERALIZE FAILURES:
- **Never assume complete failure**: If an `icnow` query fails, returns empty results, or lacks context for a *specific* lookup, do NOT assume the tool or database is broken for subsequent lookups.
- **Always try `icnow` first**: Treat each code investigation task as independent. You must *always* attempt to use `icnow` first for every new task, even if a previous `icnow` call in the same session fell back to native search.

---

## 💡 Token-Saving Multi-Tool Concatenation Guide

To prevent exhausting context limits and avoid reading full files, leverage tool concatenation to fetch **only** the precise information needed:

### 1. The Navigation Loop: Search $\rightarrow$ Target Implementation
*   **Goal**: Understand a method or class logic without reading the entire file.
*   **Workflow**:
    1. Call `search_symbols(query: "...", kind_filter: ["Class", "Method"])` to retrieve the exact Node ID.
    2. Immediately call `get_symbol_implementation(node_id: "...")` using the matched ID.
*   **Token Savings**: **90-95%**. Pulls in 10-30 lines of targeted code instead of a 300+ line file.

### 2. The Structural Probe: Outline File $\rightarrow$ Target Methods
*   **Goal**: Inspect a file's public interface and read specific methods of interest.
*   **Workflow**:
    1. Call `get_file_structure(file_path: "...")` to obtain a hierarchical list of defined classes and methods.
    2. Selectively call `get_symbol_implementation(node_id: "...")` *only* on the specific method nodes needed.
*   **Token Savings**: **70-80%**. Avoids reading imports, docstrings, helper boilerplate, and unrelated method implementations.

### 3. The Call-Chain Discovery: Trace Calls $\rightarrow$ Snippet Inspection
*   **Goal**: Trace how data flows from an entrypoint (e.g., Controller callback) to a database operation.
*   **Workflow**:
    1. Call `trace_call_path(start_node_id: "...", end_node_id: "...")` (or run a Cypher query) to map out intermediate calls.
    2. Inspect the bodies of the intermediate function nodes using `get_symbol_implementation` for the specific call site.
*   **Token Savings**: **80-90%**. Avoids opening and reading 4-5 files along the dependency path.

### 4. Cypher Aggregation (One Call, Global Answers)
*   **Goal**: Answer questions about architectural structure or relationships across the entire codebase in one round-trip.
*   **Workflow**: Use `query_graph_cypher(query: "...")` to extract high-level metrics or patterns.
    *   *Example: Find all methods in class `User` that call a method starting with `UserLog.`*
        `MATCH (c:Class {name: "User"})-[:HAS_METHOD]->(m:Method)-[:CALLS]->(t:Unresolved) WHERE t.id STARTS WITH 'UserLog.' RETURN m.name, t.id`
*   **Token Savings**: **99%**. One single Cypher query returns a tabular list, avoiding grepping and reading dozens of potential files.

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

---

## 📊 Call Tracking (Mandatory Logging)

Every time you call an `icnow` tool, you must track it in a daily CSV file at `/tmp/{day}-{month}-{year}_icnow.csv`. Append a row with the format: `call, tool_name, success, enough, why_not_enough, target_information`.
-   `success`: Was the execution technically successful? (`true`/`false`)
-   `enough`: Did the graph provide enough context to prevent you from using native grep/read? (`true`/`false`/`pending`)
-   `why_not_enough`: Explain exactly why `icnow` was not enough and you had to fall back (e.g., "Empty search result", "Missing CALLS edge"). Use `"N/A"` if enough is true.
-   `target_information`: Describe what information you were trying to find in the database (e.g., "Definition of method call_webhook").
