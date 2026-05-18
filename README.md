# icnow (Code Knowledge Graph MCP Server)

## Objective

The objective of this project is to create a Model Context Protocol (MCP) server that allows AI agents to save, query, and navigate information about coding projects. By structuring codebase knowledge as a graph, AI agents can better reason about project architectures and dependencies. 

For example, in a Ruby on Rails project, this server is ideal for saving model relationships such as "Users have many Posts".

## Features

- **Graph Representation:** Utilizes `graphqlite` to natively represent codebase relationships and knowledge as a graph.
- **Multi-Layered Architecture Analysis:** Capable of capturing various semantic layers of a project:
  - **Model Level:** Entities and their relationships (e.g., One-to-Many, Belongs-To).
  - **Controller Level:** Endpoints and request handling logic.
  - **Method Level:** Function calls, references, and internal dependencies for each method.

## Getting Started

*(Instructions for building and running the MCP server will be added here)*


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
- **`label`**: The relationship type (e.g., `CALLS`, `IMPORTS`, `REFERENCES`, `BELONGS_TO`).