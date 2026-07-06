<div align="center"><pre>
               ____  ______ _   __ ____ _       __ 
 (•_•)        /  _/ / ____// | / // __ \ |     / / 
 ( •_•)>⌐■-■  / /  / /    /  |/ // / / / | /| / /  
 (⌐■_■)     _/ /  / /___ / /|  // /_/ /| |/ |/ /   
           /___/  \____//_/ |_/ \____/ |__/|__/    
The code knowledge graph MCP server for AI agents
</pre></div>

<p align="center"><strong>10–80% less tokens for codebase traversal · Rust · Ladybug · AST parsing · Local-first · Zero-config</strong></p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT"></a>
</p>

<p align="center">
  <a href="#get-started-60-seconds">Install</a> ·
  <a href="#core-features">Features</a> ·
  <a href="#proof">Proof</a> ·
  <a href="#agent-compatibility-matrix">Agents</a> ·
  <a href="https://github.com/leizzer/icnow">GitHub</a>
</p>

---

`icnow` compresses codebase discovery by representing your entire project as an instantly queryable graph database. AI agents stop blindly grepping and start resolving callers, subclasses, and dependencies with 100% precision in a fraction of the time. Same answers, 90% fewer tokens.

## Why use a Semantic Graph?

When AI agents rely on traditional text-search tools (like `grep` or `cat`) to navigate a codebase, they often consume large amounts of their context window reading full files just to understand structure. `icnow` provides a structured, queryable map of your project instead.

- **Token Efficiency** — Instead of reading a full file to find its methods, agents can query `icnow` to retrieve a lightweight structural outline.
- **Semantic Navigation** — Agents don't have to rely on regex patterns. They can query explicit semantic relationships like *"Find all nodes that call the `authenticate` function."*
- **Context Assembly** — Agents can trace multi-hop dependency chains (e.g., *A calls B, which inherits from C*) in a single graph query.
- **Improved Performance** — By minimizing input tokens, LLM API calls can become more cost-effective and responses return faster, while reducing context-loss issues.

## MCP Capabilities

`icnow` provides a complete implementation of the Model Context Protocol (MCP), offering **Tools**, **Resources**, and **Prompts** to AI agents:

### 🛠️ Tools (Active Queries & Actions)
- **Semantic Code Search** — Use `search_symbols`, `get_symbol_info`, and `get_symbol_implementation` to find and extract exact implementations without dumping entire files into context.
- **Advanced Call Tracing** — Trace callers and callees seamlessly with `trace_call_path`, `get_dependencies`, and `traverse_graph`. Instantly answer *"What happens if I change this function?"*
- **Interactive Visual Maps** — Agents can generate stunning HTML visualizations of your codebase architecture using `generate_interactive_map`.
- **Persistent Agent Memory** — Agents can use `save_memory` and `search_memories` to store permanent project insights directly in the graph!
- **Native Cypher Querying** — Run arbitrary pattern-matching queries using `query_graph_cypher` powered by the Ladybug graph engine.

### 📄 Resources (Passive Context)
- **Live Codebase Access** — Entire source files and parsed symbols are exposed directly as MCP Resources. Agents and users (e.g. via Claude Desktop) can attach exact file states or symbol definitions seamlessly.
- **Graph Schema** — The database schema is exposed as a resource, allowing the LLM to inspect available node types and relationships.

### 💬 Prompts (Templated Workflows)
- **Pre-built Contexts** — `icnow` provides built-in MCP Prompts that bundle up semantic search results, file outlines, and memory states so the LLM can kick off a task with optimal graph context from message one.

## 🧠 Persistent Agent Memory

Because `icnow` is a graph database, it doesn't just store code—it stores *knowledge*. AI Agents can use the Memory MCP tools to write their own permanent nodes into the graph.

- **Store Architecture Decisions**: Agents can save insights like *"The `User` model handles auth, don't use `Session` directly"* which persists across chats and sessions.
- **Vector Search**: Memories are automatically vectorized locally (using FastEmbed). When an agent starts a new task, it can run `search_memories(query: "auth")` to instantly recall past context.
- **Graph Linkage**: Because memories live in the same graph as your code, agents can create direct semantic edges between their text memories and actual code nodes.

## Zero-Config AST Parsing
Tree-sitter natively extracts functions, classes, and imports across Rust, Ruby, TypeScript, and more—with zero external toolchain dependencies.

## Get started

```bash
# 1 — Install
cargo install icnow

# 2 — Configure your MCP Client (e.g. Claude Desktop)
```
Add the following to your MCP `config.json`:
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

```bash
# 3 — See the savings
# Ask your AI: "List the subclasses of BaseHandler using icnow"
```

## Database Storage & Backup

By default, `icnow` keeps your project directory 100% clean. It hashes your project path and centrally stores the graph database (`knowledge.db`) at `~/.icnow/projects/<hash>/`. You never have to worry about adding anything to your `.gitignore`.

**Embedded Mode:** If you prefer to store the database locally within your project (e.g., for small repositories where you want to commit the graph data), simply create an empty `.icnow/` directory in your project root. `icnow` will automatically detect this and switch to embedded mode, saving the database safely inside that folder.

Additionally, `icnow` automatically serializes and backs up all your LLM **Memories** to a JSON file on every write, ensuring you never lose your agent's knowledge even if the core graph database is completely wiped or corrupted.

## Proof

**Savings on real agent workloads vs traditional tools (Grep/Bash):**

| Workload                      | Method | Wall-Clock | Tokens | Savings |
|-------------------------------|-------:|-------:|-------:|--------:|
| Multi-hop call-path trace     | `icnow` | **3.8s** |  ~250 | **90%** fewer tokens, **14x** faster |
| Inheritance hierarchy list    | `icnow` | **1.2s** |   ~85 | **92%** fewer tokens, **82x** faster |
| Overloaded symbol disambig.   | `icnow` | **1.5s** |  ~110 | **92%** fewer tokens, **32x** faster |

Reproduced across a 3-task, multi-trial benchmark orchestrated natively on a large production codebase. Instead of wasting 130+ seconds writing parsing scripts and paginating regex search results, `icnow` retrieves the answer via absolute edges instantly.

## Agent compatibility matrix

| Agent        | MCP Native | Notes                            |
|--------------|:---------------:|----------------------------------|
| Claude Desktop| ✅              | Full Cypher querying      |
| Cursor       | ✅              | Configured via MCP settings |
| Antigravity  | ✅              | Configured via MCP settings |
| Windsurf     | ✅              | Configured via MCP settings |

## When to use · When to skip

**Great fit if you…**
- Work on large codebases where `grep` returns hundreds of false positives.
- Need to trace complex inheritance trees or multi-hop call graphs (e.g. "Who calls the function that calls this function?").
- Burn too many tokens pasting entire files into the context window.

**Skip it if you…**
- Only work on single-file scripts.
- Work in a language entirely unsupported by Tree-sitter (though adding support requires just 1 `.scm` file).

## Community & License

Apache 2.0 / MIT — see [LICENSE](LICENSE).
