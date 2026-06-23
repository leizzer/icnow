---
name: icnow
description: Triggers whenever the agent is working with source code files (e.g., Ruby, TypeScript, React, JavaScript, Rust), needs to find symbol definitions, trace method/function call paths, inspect code blocks, understand codebase architecture, or query the semantic knowledge graph.
---

# `icnow` Semantic Graph Skill: The Masterclass

Welcome to the `icnow` documentation. This tool interfaces with a high-performance **Kuzu Graph Database** (`knowledge.db`) to give you semantic, structurally aware access to the codebase.

By using `icnow`, you can navigate massive projects with **95% fewer tokens** and **100% higher accuracy** than recursive grepping. You are no longer flying blind—you have a map.

---

## 🛑 1. Strict Rules of Engagement: The Hybrid Approach

**DO NOT** use `grep -r` blindly across the entire project to find definitions or references. **YOU MUST USE `icnow` TOOLS TO LOCATE THE TARGET.**

However, do **NOT** stubbornly force `icnow` if it's missing data. The ultimate benchmarked sweet-spot is the **Hybrid Approach**:
1. **Find it via Graph**: Use `search_symbols` to instantly find the exact filepath.
2. **Context via Graph**: Use `get_symbol_info` to find callers, dependencies, and methods.
3. **Read it via Terminal**: If you need to read the full file body, use the traditional native tool `view_file` on the exact path found by `icnow`.

### 🏆 Benchmark Proven:
- **✅ WINS for structural queries (95% token savings):** Locating files, finding all callers/callees via `get_symbol_info`, counting methods in a file.
- **❌ LOSES for pure text extraction:** Once you know the path, just use `view_file`.
- **❌ LOSES for simple exact string matches:** Do not use `icnow` for raw string pattern matching (e.g., finding the string `"TODO"`). Use `grep_search`.

---

## 💡 2. The Golden Workflows

### Scenario A: Finding a Method Definition and its Context
1. **Locate the ID:** Call `search_symbols(query: "authenticate_user")` to find the node ID (e.g., `app/controllers/application.rb::authenticate_user`).
2. **Get 360-Degree Context:** Call `get_symbol_info(node_id: "...")`. 
   - This returns a beautifully formatted summary showing the exact docstring, signature, **Incoming Usages** (callers), **Outgoing Dependencies**, and **Contains** (methods/children with an exact count!).
3. **Read Code (Optional):** If you need the exact implementation body, call `view_file` on the file path.

### Scenario B: 0-Result Fallback (The Missing File)
If `search_symbols` returns 0 results:
1. **DO NOT** guess file paths and try to `parse_project_file`.
2. **DO NOT** write Cypher queries.
3. **DO** use the traditional `grep_search` terminal tool to find the definition in the codebase.
4. Once you find the file path via grep, run `parse_project_file(file_path)` to index it, then proceed.

### Scenario C: Finding Callers of Dynamic/Metaprogrammed Methods
If you search for a symbol (like `audit_log`) and `search_symbols` returns many rows with `unresolved_symbol` (e.g., `/path/to/user.rb::unresolved_call_10`):
1. **THESE ARE THE CALL SITES!** Tree-sitter creates `unresolved_symbol` placeholders when it encounters a method call whose definition it cannot statically link (like Rails metaprogramming).
2. Look at the `id` property of those unresolved symbols. It tells you exactly the file path and the line number where the call occurred.
3. **DO NOT** try to `get_symbol_info` on the unresolved symbols. You already have the file and line number—just use `view_file` to look at it if needed!

---

## 🛡️ 3. Preventing Staleness: The Coverage Check

The graph is only as good as its data. If files are heavily modified or un-indexed, the graph becomes stale.
- **Always run `coverage_check(directory_path)`** when starting work in a specific folder. It will instantly tell you which files are missing from the graph or out of date.
- If files are missing, call `parse_project_file(file_path)` to ingest them immediately before querying.

---

## 🛠️ 4. The 6 Core Tools

We have intentionally restricted the `icnow` MCP to only the 6 most powerful tools to eliminate decision paralysis. **Do not attempt to use any other icnow tools (no Cypher, no Memory tools).**

1.  **`search_symbols(query: String, limit: Option<u32>, kind_filter: Option<Vec<String>>)`**  
    Searches the graph for nodes matching a symbol name. 
2.  **`get_symbol_info(node_id: String)`** 🌟 **THE MASTER TOOL** 🌟 
    Returns complete context for a node ID. Includes incoming usages, outgoing dependencies, and explicitly lists all methods/children contained inside it (with an exact count so you don't have to count manually).
3.  **`parse_project_file(file_path: String)`**  
    Parses a file and adds it to the graph. Only call if `coverage_check` shows it is missing or out-of-date, or if `search_symbols` yielded 0 results and you manually found the file with `grep`.
4.  **`coverage_check(directory_path: String, project_root: Option<String>)`**
    Checks a directory for missing or stale files in the database. Run this first when you suspect staleness.
5.  **`deep_scan(project_root: Option<String>, compiler_flags: Option<String>)`**  
    Runs a full background indexing job for the entire project. This takes time, so only use it if the user requests a full rebuild.
6.  **`list_indexed_files(project_root: Option<String>)`**  
    Lists all files tracked in the knowledge graph.

---

## 📊 5. Call Tracking (Mandatory Logging)

Every time you call an `icnow` tool, you must track it in a daily CSV file at `/tmp/{day}-{month}-{year}_icnow.csv`. Append a row with the format: `call, tool_name, success, enough, why_not_enough, target_information`.
-   `success`: Was the execution technically successful? (`true`/`false`)
-   `enough`: Did the graph provide enough context to prevent you from using native grep/read? (`true`/`false`/`pending`)
-   `why_not_enough`: Explain exactly why `icnow` was not enough and you had to fall back. Use `"N/A"` if enough is true.
-   `target_information`: Describe what information you were trying to find in the database.
