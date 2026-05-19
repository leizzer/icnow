# Claude Skill: `icnow` Architecture First

Add these guidelines to your custom developer instructions or system prompt when working on this repository to force Claude to leverage the highly-optimized `icnow` graph database instead of expensive shell actions.

---

## 🎯 Primary Directive
> [!IMPORTANT]
> **PRIORITIZE ICNOW**: You must always attempt to use the `icnow` MCP server tools first to search, read, explore, or navigate files ending in `.rs` (Rust), `.rb` (Ruby), `.ts` (TypeScript), or `.tsx` (TSX).
> 
> **TRADITIONAL LOOKUP FALLBACK**: You are **ONLY** permitted to fall back to traditional lookup tools (like `view_file` or raw shell commands such as `cat`, `rg`, `grep`, `find`, `sed`) to search or read files if the response returned by `icnow` is unsatisfactory or insufficient to accomplish the given task.
> Before executing any fallback lookup, you **MUST** explicitly state in your response *why* the `icnow` tool did not satisfy your query or was insufficient. Once explained, you may complete the task using traditional lookup tools.

---

## 🛠️ Step-by-Step Agent Workflow

### 1. File Exploration & Architecture Mapping
When trying to understand a file's structure, imports, modules, classes, or methods:
- **Do NOT**: Run `cat path/to/file` or `grep` it.
- **Do**: Call the `parse_project_file` tool with the path to the file.
- **Why**: This returns a lightweight, token-saving architectural summary of all classes, modules, and methods in milliseconds.

### 2. Method/Function Implementation Search
When you need to read the exact implementation of a specific struct method or standalone function:
- **Do NOT**: Open the raw file to read the lines.
- **Do**: Issue a `query_graph` SQL call to pull the `source_code` property for that specific node:
  ```sql
  SELECT s.value as source_code 
  FROM nodes n 
  JOIN node_props_text s ON n.id = s.node_id AND s.key_id = (SELECT id FROM property_keys WHERE key='source_code') 
  JOIN node_props_text id_prop ON n.id = id_prop.node_id AND id_prop.key_id = (SELECT id FROM property_keys WHERE key='id') 
  WHERE id_prop.value = 'path/to/file::ClassName::method_name';
  ```
- **Why**: This retrieves the precise, context-isolated function body, slashing token consumption by up to **98%** compared to viewing the whole file.

### 3. Native Cypher Graph Queries
When traversing complex relationship patterns (e.g. matching specific labels, relationships, or filters across the codebase):
- **Do NOT**: Try to construct complex SQLite SQL joins.
- **Do**: Call the `query_graph_cypher` tool and write clean, expressive Cypher query language:
  ```cypher
  MATCH (c:Class)-[:HAS_METHOD]->(m:Method) 
  RETURN c.id, m.id 
  LIMIT 5;
  ```
- **Why**: `graphqlite` natively parses Cypher! This matches graph patterns natively and returns beautiful, easy-to-read Markdown result tables.

### 4. Transitive Neighborhood Paths
When you need to discover all connected callers, containers, or dependencies surrounding a file, class, or method recursively:
- **Do NOT**: Write complex multi-level recursive CTEs in SQL.
- **Do**: Call `traverse_graph` passing the starting `node_id` and a `max_depth` (e.g., 3).
- **Why**: It executes a lightning-fast bidirectional recursive walk and returns a complete nested map of all neighboring nodes up to `N` hops away in a single call.

---

## 📊 Token Usage Enforcement
- If you use `cat` or `view_file` on a `.rs` or `.rb` file directly, it is considered an **architectural failure** (wastes context window).
- Leverage the `icnow` MCP tools to maintain a hyper-optimized context footprint.
