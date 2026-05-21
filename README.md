# icnow (Code Knowledge Graph MCP Server)

## Objective

The objective of this project is to create a Model Context Protocol (MCP) server that allows AI agents to save, query, and navigate information about coding projects. By structuring codebase knowledge as a graph, AI agents can better reason about project architectures and dependencies. 

For example, in a Ruby on Rails project, this server is ideal for saving model relationships such as "Users have many Posts".

## Features

- **Graph Representation & Querying:** Utilizes `graphqlite` to natively represent codebase relationships and query them using standard Cypher graph queries.
- **Multi-Layered Architecture Analysis:** Capable of capturing various semantic layers of a project:
  - **Model Level:** Entities and their relationships (e.g., One-to-Many, Belongs-To).
  - **Controller Level:** Endpoints and request handling logic.
  - **Method Level:** Function calls, references, and internal dependencies for each method.

## Getting Started

### 1. Building the Server
Ensure you have the Rust toolchain installed, then build the release binary:
```bash
cargo build --release
```

### 2. Running the Server
By default, the server runs over Standard I/O (stdio) as an MCP service. You can start it manually or configure your MCP client to invoke it:
```bash
./target/release/icnow [path/to/custom_knowledge.db]
```
If no database path is specified as the first argument, the server defaults to creating or opening `knowledge.db` in the current working directory of the process.

## The Knowledge Database (`knowledge.db`)

> [!NOTE]
> **Is the database shared or isolated?**
> The database is **isolated per project**. Each project that uses `icnow` has its own independent database file.

By default, when `icnow` is invoked, it retrieves the current working directory of the process (`std::env::current_dir()`) and initializes or opens a SQLite file named `knowledge.db` directly inside that directory (i.e. `./knowledge.db`).

- **Project Isolation:** Because the database resides in the calling project's root, codebase schemas, call graphs, and metadata are cleanly separated between projects.
- **Custom Location:** If you wish to use a shared database or store the database elsewhere, you can pass the path as a command-line argument:
  ```bash
  icnow /path/to/shared_knowledge.db
  ```

## Architecture & Approach

Building a cross-language code knowledge graph involves two distinct challenges: **Node Extraction** and **Edge Resolution**.

### 1. Node Extraction: Tree-sitter Query API
We use **Tree-sitter** natively inside the MCP server to parse files instantly. 
- **The Query API:** Instead of writing complex, language-specific Rust code to traverse syntax trees, we rely on the industry-standard **Query API**. 
- **How it works:** We feed standard `.scm` queries (e.g., `(function_item name: (identifier) @name) @function.node`) to the engine. This makes `icnow` entirely language-agnostic. To add support for Ruby or Python, we simply provide a new query string—no Rust changes required!
- **Zero Dependencies:** The beauty of Tree-sitter is that the user does not need to install any external language toolchains (like Node.js or a Rust compiler) for the MCP server to extract these Nodes.

### 2. Edge Resolution: The Module Resolution Problem
While Tree-sitter easily extracts an import string (e.g., `use icnow::tools::GraphService;`), figuring out exactly what physical file that string points to on the hard drive is incredibly complex. 
- Every language has drastically different module resolution algorithms (e.g., Rust's `mod.rs`, Node's `package.json` main/index resolution, Python's `__init__.py` and `PYTHONPATH`, Ruby's Zeitwerk autoloading).
- Rebuilding these algorithms natively in `icnow` is virtually impossible.

### 3. The Ultimate Hybrid Solution
Because of the Edge Resolution problem, we rely on **AI Agents** or **LSIF (Language Server Index Format)** to draw the connections between files:

- **The LSIF Route (Perfect Accuracy, High Burden):** The user runs an external language indexer (like `rust-analyzer lsif .`) to perfectly resolve all edges. `icnow` then ingests this JSON dump. The trade-off is the user *must* have the correct language toolchain installed locally.
- **The Agent-Driven Route (Highest Flexibility, Zero Config):** `icnow` instantly provides all the baseline Nodes (Files, Functions, Structs, Imports) using Tree-sitter. Then, we provide tools to an AI Agent to explore the codebase or query the user's running IDE to figure out where an import comes from, and explicitly call `save_edge` to draw the cross-file connections.

## Data Schema

The server strictly enforces the following schema to maintain a consistent graph:

### Nodes
Nodes represent files, functions, classes, models, or imports.
- **`id`**: Must be a globally unique string. To prevent collisions, we use the format `path/to/file.ext::node_name` (e.g., `src/models.rs::Node` or `app/models/user.rb::User`). For files themselves, the ID is simply the file path.
- **`label`**: The domain-level type (e.g., `Function`, `Struct`, `File`, `Model`).
- **`kind`**: The specific AST syntax item (e.g., `function_item`, `class_declaration`).
- **`properties`**: A key-value map for arbitrary metadata.

### Edges
Edges represent the relationships between two nodes.
- **`source` / `target`**: These **MUST** be the exact String `id`s of the Nodes you are connecting (e.g., source: `src/main.rs::main`, target: `src/models.rs::Node`).
- **`label`**: The relationship type (e.g., `CALLS`, `IMPORTS`, `REFERENCES`, `BELONGS_TO`, `CONTAINS`).

## Underlying SQL Storage Structure

Under the hood, `graphqlite` maps graph nodes, edges, and properties into a standard SQLite schema. This is useful if you are using the raw `query_graph` SQL tool.

| Table | Purpose | Key Columns |
| :--- | :--- | :--- |
| `nodes` | Primary entity nodes | `id` (int primary key) |
| `edges` | Relationship links | `id` (int), `source_id` (fk nodes), `target_id` (fk nodes), `type` (text) |
| `property_keys` | Metadata key names registry | `id` (int), `key` (text unique) |
| `node_props_text` | Text-valued attributes | `node_id`, `key_id`, `value` (text) |
| `node_props_int` | Integer-valued attributes | `node_id`, `key_id`, `value` (int) |
| `node_props_real` | Real-valued attributes | `node_id`, `key_id`, `value` (real) |
| `node_props_bool` | Boolean-valued attributes | `node_id`, `key_id`, `value` (bool) |
| `node_props_json` | JSON-valued attributes | `node_id`, `key_id`, `value` (json) |

> [!IMPORTANT]
> The node's global string identifier (e.g., `src/models.rs::Node`) is stored as a text property in `node_props_text` under the key `"id"`. The `nodes` table only contains an internal auto-incrementing integer key.

## Cypher Querying Guide

We use **Cypher** (via `graphqlite`) as the primary graph query language for retrieving relationships, code patterns, and semantic dependencies.

### Common Queries & Patterns

- **List Classes and Their Methods:**
  ```cypher
  MATCH (c:Class)-[:CONTAINS]->(m:Method)
  RETURN c.id, m.id
  LIMIT 10
  ```

- **Fuzzy Search for a Symbol:**
  ```cypher
  MATCH (n)
  WHERE toLower(n.id) CONTAINS toLower('user')
  RETURN n.id, labels(n)
  ```

- **Find Callers of a Function:**
  ```cypher
  MATCH (caller)-[r:CALLS]->(callee)
  WHERE callee.id = 'app/models/user.rb::User#full_name'
  RETURN caller.id, type(r)
  ```

- **Find Dependencies of a File:**
  ```cypher
  MATCH (file:File)-[r:IMPORTS]->(dep)
  WHERE file.id = 'src/main.rs'
  RETURN dep.id
  ```

### Performance Tips & Pitfalls

> [!WARNING]
> **Cartesian Products:**
> Avoid disconnected `MATCH` patterns (e.g., `MATCH (a) MATCH (b)`). Because `graphqlite` evaluates them as cross-joins, querying them on large databases will cause the query to hang or exhaust resources. Always link your patterns.

> [!IMPORTANT]
> **Variable-Length Path Directionality:**
> Variable-length paths (e.g. `-[*1..3]-`) in `graphqlite` only traverse in the outgoing/defined direction, even if written without arrows. For bidirectional path search (e.g. tracing up and down a call graph), combine the incoming and outgoing paths with a `UNION`:
> ```cypher
> MATCH (n) WHERE n.id = 'target' MATCH (n)-[*1..2]->(x) RETURN x.id
> UNION
> MATCH (n) WHERE n.id = 'target' MATCH (n)<-[*1..2]-(x) RETURN x.id
> ```

## MCP Tool Guidance & Best Practices

There are three ways to provide guidance to LLM agents on when and how to use `icnow`'s MCP tools:

### 1. Host-Level System Instructions (`instructions.md`)
For advanced MCP clients (such as Antigravity/Gemini), you can place an `instructions.md` file in the MCP server configuration directory:
- **Path:** `/Users/cristian/.gemini/antigravity/mcp/icnow/instructions.md`
- **Behavior:** When the host application registers `icnow`, it automatically reads this file and appends its contents directly to the LLM's system prompt. Use this file to document complex multi-tool workflows and domain-specific rules.

### 2. Protocol-Level Metadata (MCP Specification)
The Model Context Protocol supports two native mechanisms within the server implementation:
- **Tool JSON Schema Descriptions:** Every tool and parameter schema contains a `description` field. The LLM uses these fields to determine the utility, arguments, and return expectations of each tool.
- **Prompts API (`prompts/list`, `prompts/get`):** The server can expose predefined templates (workflows, debugging templates, etc.) that the user can trigger to feed the LLM structured instructions on orchestrating the tools.

### 3. Project-Level Agent Rules (Workspace Configurations)
You can define rules directly inside the repository workspace root using:
- `.clauderules` (For Claude Desktop / CLI)
- `.geminirules` / `.cursorrules` (For Gemini, Antigravity, and Cursor)

These files are read on session initialization to enforce rules such as checking `icnow` first before falling back to traditional file reading (`view_file`, `cat`, or `grep`).