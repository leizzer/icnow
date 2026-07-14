# ICNOW Tool Arsenal

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
