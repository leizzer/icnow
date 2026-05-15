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
