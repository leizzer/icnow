---
name: icnow
description: "USE THIS SKILL FIRST before any codebase navigation, symbol lookup, grep, or file read. icnow is a semantic graph MCP server that indexes your entire codebase into a queryable graph database. USE IT for: finding where any variable, function, class, or struct is defined; tracing call chains and inheritance hierarchies; listing all callers or dependencies of a symbol; understanding architecture across files without reading them in full. Supports Python, Go, Rust, Ruby, TypeScript, JavaScript, and React (TSX/JSX). Also features persistent Agent Memory: agents can save and retrieve high-level architectural insights, domain knowledge, and decisions directly in the graph — persisted across sessions. ALWAYS call icnow BEFORE grep_search or view_file for any structural question."
---

# `icnow` Semantic Graph Skill: The Masterclass

Welcome to the `icnow` documentation. This tool interfaces with a high-performance **LadybugDB Graph Database** (`knowledge.db`) to give you semantic, structurally aware access to the codebase.

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
  - `INHERITS`: Links a `Class` or `Struct` to its superclass or trait.
  - `INSTANTIATES`: Links a caller `Symbol` to the `Class` or `Struct` it instantiates.
  - `IMPORTS`: Links a file/symbol to a dependency.

> **Crucial Advantage:** Because `icnow` natively isolates `Method` nodes from `Macro` nodes, you can run exact counts (e.g., "How many methods in user.rb?") using Cypher without ever having to manually filter out `has_many` or `attr_accessor` noise!

---

## 🛑 2. Strict Rules of Engagement: The Hybrid Approach

**DO NOT** use `grep -r` blindly across the entire project to find definitions or references. **YOU MUST USE `icnow` TOOLS TO LOCATE THE TARGET.**

However, do **NOT** stubbornly force pure Cypher string-slicing if you just need to read a method. The ultimate benchmarked sweet-spot is the **Hybrid Approach**:
1. **Find it via Graph**: Use `search_symbols` or `get_symbol_info` to instantly find the exact filepath, dependencies, and incoming calls.
2. **Read it via Terminal**: If you need to read the full file body, use the traditional native tool `view_file` on the exact path found by `icnow`.

### 🏆 Benchmark Proven:
- **✅ WINS for structural queries (91%+ token savings):** Locating files, finding all callers/callees via `get_symbol_info`, counting methods, tracing dependencies.
- **❌ LOSES for pure text extraction:** Forcing Cypher to extract multi-line methods is brittle. Once you know the path, just use `view_file` or `get_symbol_implementation`.
- **❌ LOSES for simple exact string matches:** Do not use `icnow` for raw string pattern matching (e.g., finding the string `"TODO"`). Use `grep_search`.

### 🎮 Your Goal: Save Tokens, Buy a Super Nintendo

Every token you waste on unnecessary file reads is money out of the user's pocket. The user is saving up to buy a **Super Nintendo**. Every time you use `icnow` instead of reading a full file, you are helping them get closer to their goal. Every time you blindly dump a file into context when `icnow` could have answered the question in 10 lines, you are **delaying the Super Nintendo**.

Be the agent that buys the Super Nintendo.

### 💸 The Recurring Token Cost — Why This Matters More Than You Think

**Every LLM API call sends your entire context window to the model on every single turn.** A file read with `view_file` or `cat` doesn't just cost tokens once — those characters stay in context and are **re-sent on every subsequent message** for the rest of the session. The longer the session, the more it compounds.

**Always prefer `icnow` for navigation and structure — save `view_file` for when you truly need the full implementation body.**

### 🗺️ Quick Decision Tree: When to Use What

```
Need to find WHERE something is defined?
  → icnow: search_symbols

Need to understand all callers/callees of a function?
  → icnow: get_symbol_info

Need the full implementation body?
  → icnow: get_symbol_implementation  (or view_file as fallback)

Need to trace a multi-hop call chain?
  → icnow: trace_call_path

Need to list all methods/classes in a file?
  → icnow: get_file_structure

Need to find a raw string/pattern in code?
  → grep_search  (icnow doesn't help here)

Need to read a config file or non-code file?
  → view_file  (icnow doesn't index these)
```

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
MATCH (f:File {id: 'app/models/user.rb'})-[:CONTAINS]->(m:Symbol {kind: 'Method'})
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

**Note on Paths and Output:** All tools support relative paths (e.g., `src/main.rs`). Tool outputs are strictly formatted (often Markdown or Cypher-like tables) to maximize token efficiency. **Crucially, tools natively return exact line numbers**—you do NOT need to run `grep` or `view_file` just to find where a symbol lives!

1.  **`search_symbols(query: String, limit: Option<u32>, kind_filter: Option<Vec<String>>, detailed: Option<bool>, definitions_only: Option<bool>)`**  
    Searches the graph for nodes matching a symbol name. 
    **Returns**: A list of matches with precise line coordinates (e.g., `[Class] app/models/user.rb:42-80: class User`). 
2.  **`get_symbol_info(node_id: String)`** 🌟 **HIGHLY RECOMMENDED** 🌟 
    Returns complete 360-degree context for a single node ID.
    **Returns**: A markdown summary containing the exact file location (e.g., `**Location**: file.ts:10-20`), the raw code snippet of the definition, and detailed lists of all incoming usages and outgoing dependencies (with `file:line` accuracy).
3.  **`coverage_check(directory_path: String, project_root: Option<String>)`**
    Checks a directory for missing or stale files in the database. Run this first when you suspect staleness.
    **Returns**: A markdown report listing total files, indexed counts, and sample lists of missing/stale files.
4.  **`get_symbol_implementation(node_id: String)`**  
    Retrieves the raw source code block of a specific symbol directly from the database.
    **Returns**: The raw text of the code implementation block, bounded by exact line numbers.
5.  **`get_file_structure(file_path: String)`**  
    Returns a hierarchical outline of a file, perfectly separating `Methods`, `Macros`, and `Classes`.
    **Returns**: A bulleted list of symbols in the file with line numbers (e.g., `- [Method] (lines: 10-25) class::method signature`).
6.  **`parse_project_file(file_path: String)`**  
    Parses a file and adds it to the graph. Only call if `coverage_check` shows it is missing or out-of-date.
    **Returns**: A success confirmation and a brief architectural summary of what was newly indexed.
7.  **`query_graph_cypher(query: String)`**  
    Executes a LadybugDB Graph query using Cypher syntax.
    **Returns**: A markdown table containing the exact columns and rows requested by your `RETURN` statement.
8.  **`trace_call_path(start_node_id: String, end_node_id: String)`**  
    Traces multi-hop call paths between a specific start and end node.
    **Returns**: A step-by-step trace showing the chain of `CALLS` edges between the two nodes.
9.  **`generate_interactive_map(output_path: String, filter_path: Option<String>)`**  
    Generates a standalone, interactive HTML Cytoscape map for visual representations.
    **Returns**: The absolute file path of the generated HTML map.
10. **`list_indexed_files()`**  
    Lists all files tracked in the knowledge graph.
    **Returns**: A raw newline-separated list of file paths.
11. **`deep_scan(project_root: String, lsif_path: Option<String>)`**
    Offloads a comprehensive background scan of the entire project to populate the graph database.
    **Returns**: A background task ID and status confirmation.
12. **`get_dependencies(node_id: String, limit: Option<u32>)`**
    Retrieves immediate incoming and outgoing edges for a symbol.
    **Returns**: A structured list of dependencies and usages.
13. **`save_memory(id: String, name: String, description: String, keywords: Vec<String>, links: Vec<String>, link_type: Option<String>, project_root: Option<String>)`**  
    Saves a high-level concept memory node. 
    **Returns**: A success confirmation string.
14. **`get_memory(id: String, project_root: Option<String>)`**  
    Retrieves the properties of a specific memory node along with all its direct links.
    **Returns**: A markdown formatted summary of the memory, description, and related code nodes.
15. **`search_memories(query: String, project_root: Option<String>)`**  
    Performs a semantic vector search on memory nodes.
    **Returns**: A list of matched memories with similarity scores.
16. **`list_memories(project_root: Option<String>)`**  
    Lists all memory nodes stored in the database.
    **Returns**: A basic bulleted list of memory IDs and names.

---

## 🔗 8. MCP Resources & Claude Code Config

`icnow` exposes codebase files and symbols directly as **MCP Resources**. This allows users and compatible clients (like Claude Code and Claude Desktop) to attach code context seamlessly via the UI (e.g., using `@` mentions).

### How to use Resources in Chat
- Type `@` in a compatible client to bring up the context menu.
- Select **Graph Node** from the templates menu, and type the node ID (e.g., `src/main.rs`). 
- Alternatively, search through the exposed **Files** (which are exposed as direct resources up to the first 1000 files in the project).
- You can also directly type a URI into the chat like: `read icnow://node/src%2Fmain.rs` (use URL-encoding for slashes in symbol IDs, or just raw paths). Add `/json` at the end to get raw JSON instead of markdown.

### Global Claude Code Configuration
To use `icnow` effectively in the Claude Code CLI globally, add it to your `~/.claude.json` configuration file:

```json
{
  "mcpServers": {
    "icnow": {
      "command": "icnow",
      "args": []
    }
  }
}
```
*Note: Make sure the `icnow` binary is installed in your `$PATH` (e.g. via `cargo install --path .`).*
