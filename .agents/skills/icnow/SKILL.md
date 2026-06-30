---
name: icnow
description: "USE THIS SKILL FIRST for any codebase navigation, exploration, symbol lookup, or BEFORE adding new methods/functions to discover existing patterns. Essential for: finding where a variable/method is defined, tracing function calls, exploring dependencies, reading code blocks, or understanding architecture in Ruby, TypeScript, React, Rust, or JS projects. Call this BEFORE using grep or reading files."
---

# `icnow` Semantic Graph Skill: The Masterclass

Welcome to the `icnow` documentation. This tool interfaces with a high-performance **Kuzu Graph Database** (`knowledge.db`) to give you semantic, structurally aware access to the codebase.

By using `icnow`, you can navigate massive projects with **95% fewer tokens** and **100% higher accuracy** than recursive grepping. You are no longer flying blind—you have a map.

---

## 🧠 1. The Graph Architecture

Before using the tools, you must understand how the code is modeled:

- **Nodes**:
  - `File`: Represents a physical file.
  - `Symbol`: Represents a code construct. Key `kind`s include: `Class`, `Method`, `Macro` (e.g., Rails `has_many`, `scope`), `Struct`, `Variable`, and `Import`.
  - `Unresolved`: Call-site placeholders used to track method invocations before linking.
- **Edges**:
  - `CONTAINS`: Links a `File` or `Class` to the `Symbols` it defines.
  - `CALLS`: Links a caller `Symbol` to the target it invokes. Includes exact `file` and `line` metadata.
  - `IMPORTS`: Links a file/symbol to a dependency.

> **Crucial Advantage:** Because `icnow` natively isolates `Method` nodes from `Macro` nodes, you can run exact counts (e.g., "How many methods in user.rb?") using Cypher without ever having to manually filter out `has_many` or `attr_accessor` noise!

---

## 🛑 2. Strict Rules of Engagement: The Hybrid Approach

**DO NOT** use `grep -r` blindly across the entire project to find definitions or references. **YOU MUST USE `icnow` TOOLS TO LOCATE THE TARGET.**

However, do **NOT** stubbornly force pure Cypher string-slicing if you just need to read a method. The ultimate benchmarked sweet-spot is the **Hybrid Approach**:
1. **Find it via Graph**: Use `search_symbols` or `get_symbol_info` to instantly find the exact filepath, dependencies, and incoming calls.
2. **Read it via Terminal**: If you need to read the full file body, use the traditional native tool `view_file` on the exact path found by `icnow`.

### 🏆 Benchmark Proven:
- **✅ WINS for structural queries (95% token savings):** Locating files, finding all callers/callees via `get_symbol_info`, counting methods, tracing dependencies.
- **❌ LOSES for pure text extraction:** Forcing Cypher to extract multi-line methods is brittle. Once you know the path, just use `view_file` or `get_symbol_implementation`.
- **❌ LOSES for simple exact string matches:** Do not use `icnow` for raw string pattern matching (e.g., finding the string `"TODO"`). Use `grep_search`.

### ⚠️ CRITICAL EXECUTION RULES:
- **DO NOT RUN MCP TOOLS IN BASH**: Never attempt to run tools like `search_memories` or `search_symbols` inside a terminal/Bash command. They are native tool calls.
- **DO NOT GENERALIZE FAILURES**: If a tool call (especially `search_memories`) returns empty or fails, **DO NOT** assume `icnow` is broken and deactivate it. Memories are often sparse in new databases. If a memory search fails, you MUST immediately fall back to `search_symbols` to find what you need.
- **DISCOVER PATTERNS BEFORE ADDING CODE**: Before adding new methods, functions, or classes, you **MUST** use `icnow` to look for existing patterns or abstractions. Use `search_symbols` and `get_symbol_info` to see how similar features are implemented. Do not reinvent the wheel.

---

## 💡 3. The "360-Degree Context" Workflow

Instead of writing complex Cypher queries to understand how a method or class fits into the codebase, rely on the dedicated `get_symbol_info` tool. It perfectly aggregates all unresolved call-site edges to give you a complete picture.

1. **Locate the ID:** Call `search_symbols(query: "authenticate_user")` to find the node ID (e.g., `app/controllers/application.rb::authenticate_user`).
2. **Get 360-Degree Context:** Call `get_symbol_info(node_id: "...")`. 
   - This returns a beautifully formatted summary showing:
     - The exact docstring and signature
     - The parent file/class container
     - **Incoming Usages:** Every file/method that calls *into* this node, complete with exact `file:line` metadata.
     - **Outgoing Dependencies:** Every method/import this node calls *out* to.
3. **Read Code (Optional):** If you need the exact implementation body, call `view_file` on the file path.

---

## 🛡️ 4. Preventing Staleness: The Coverage Check

The graph is only as good as its data. If files are heavily modified or un-indexed, the graph becomes stale.
- **Always run `coverage_check(directory_path)`** when starting work in a specific folder. It will instantly tell you which files are missing from the graph or out of date.
- If files are missing, call `parse_project_file(file_path)` to ingest them immediately before querying.

---

## 🧠 5. Creating Memories

You **MUST** create `icnow` memories when:
- You uncover a high-level **concept about the project architecture** or complex **business logic**.
- You want to document a **"high altitude view"** of how a complex logic flow works across multiple files/components.
- You identify major domain boundaries (e.g., `'payment processing'`, `'user authentication'`).

**Updating Outdated Memories:**
- If you read an existing memory and notice it is outdated, incorrect, or lacks newly discovered context, you **MUST correct it** by calling `save_memory` with the same `id` and updated descriptions/links. Keep the graph accurate!

**Rules:**
- **Link Key Anchors**: Always link memory nodes to the high-level classes or files that implement the concept. (e.g., passing `"ApplicationController"` in the `links` array will automatically resolve to the node).
- **Transient Data**: Do NOT save memories for granular details, individual bugs, or single helper methods.
- **Kickstarting Workflow**: Try calling `search_memories(query)` when starting a task to see if a domain map exists. **If it returns empty or fails, DO NOT STOP USING ICNOW.** Simply proceed to use `search_symbols` instead.

---

## 📝 6. Cypher Query Examples (`query_graph_cypher`)

When using `query_graph_cypher`, remember that nodes are either `Symbol` or `File`. `Symbol` nodes have a `kind` property (e.g., `'Method'`, `'Class'`, `'Macro'`, `'Variable'`, `'Import'`).

**Example 1: Count all methods inside a specific file**
```cypher
MATCH (f:File {id: '/Users/path/to/app/models/user.rb'})-[:CONTAINS]->(m:Symbol {kind: 'Method'})
RETURN count(m)
```

**Example 2: Find all classes that inherit from `ApplicationRecord`**
```cypher
MATCH (c:Symbol {kind: 'Class'})-[:CALLS]->(p:Symbol {name: 'ApplicationRecord'})
RETURN c.id, c.name
```

**Example 3: Find all files that import a specific module**
```cypher
MATCH (f:File)-[:IMPORTS]->(i:Symbol {name: 'react'})
RETURN f.id
```

---

## 🛠️ 7. The Tool Arsenal

1.  **`search_symbols(query: String, limit: Option<u32>, kind_filter: Option<Vec<String>>)`**  
    Searches the graph for nodes matching a symbol name. Use `kind_filter: ["Class"]` or `["Method"]` to reduce noise. Artificial unresolved nodes are filtered out automatically.
2.  **`get_symbol_info(node_id: String)`** 🌟 **HIGHLY RECOMMENDED** 🌟 
    Returns complete 360-degree context for a single node ID. Includes incoming usages (with file:line accuracy) and outgoing dependencies.
3.  **`coverage_check(directory_path: String, project_root: Option<String>)`**
    Checks a directory for missing or stale files in the database. Run this first when you suspect staleness.
4.  **`get_symbol_implementation(node_id: String)`**  
    Retrieves the raw source code block of a specific symbol directly from the database.
5.  **`get_file_structure(file_path: String)`**  
    Returns a hierarchical outline of a file, perfectly separating `Methods`, `Macros`, and `Classes`.
6.  **`parse_project_file(file_path: String)`**  
    Parses a file and adds it to the graph. Only call if `coverage_check` shows it is missing or out-of-date.
7.  **`query_graph_cypher(query: String)`**  
    Executes a Kuzu Graph query using Cypher syntax (SQLite is NOT supported). Use this for custom aggregations (e.g., `MATCH (m:Symbol {kind: 'Method'}) RETURN count(m)`).
8.  **`trace_call_path(start_node_id: String, end_node_id: String)`**  
    Traces multi-hop call paths between a specific start and end node.
9.  **`generate_interactive_map(output_path: String, filter_path: Option<String>)`**  
    Generates a standalone, interactive HTML Cytoscape map for visual representations.
10. **`list_indexed_files()`**  
    Lists all files tracked in the knowledge graph.
11. **`save_memory(id: String, name: String, description: String, keywords: Vec<String>, links: Vec<String>, link_type: Option<String>, project_root: Option<String>)`**  
    Saves a high-level concept memory node. 
12. **`get_memory(id: String, project_root: Option<String>)`**  
    Retrieves the properties of a specific memory node along with all its direct links.
13. **`search_memories(query: String, project_root: Option<String>)`**  
    Performs a semantic vector search on memory nodes.
14. **`list_memories(project_root: Option<String>)`**  
    Lists all memory nodes stored in the database.

---

## 📊 7. Call Tracking (Mandatory Logging)

Every time you call an `icnow` tool, you must track it in a daily CSV file at `/tmp/{day}-{month}-{year}_icnow.csv`. Append a row with the format: `call, tool_name, success, enough, why_not_enough, target_information`.
-   `success`: Was the execution technically successful? (`true`/`false`)
-   `enough`: Did the graph provide enough context to prevent you from using native grep/read? (`true`/`false`/`pending`)
-   `why_not_enough`: Explain exactly why `icnow` was not enough and you had to fall back. Use `"N/A"` if enough is true.
-   `target_information`: Describe what information you were trying to find in the database.
