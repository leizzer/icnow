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
  <a href="#get-started">Install</a> ·
  <a href="#mcp-capabilities">Features</a> ·
  <a href="#proof">Proof</a> ·
  <a href="#agent-compatibility-matrix">Agents</a> ·
  <a href="https://github.com/leizzer/icnow">GitHub</a>
</p>

---

`icnow` compresses codebase discovery by representing your entire project as an instantly queryable graph database. AI agents stop blindly grepping and start resolving callers, subclasses, and dependencies with 100% precision in a fraction of the time. Same answers, 90% fewer tokens.

> [!NOTE]
> **No extra token cost or heavy lifting for the agent.** `icnow` performs all the complex parsing, indexing, and graph traversal locally on your machine. When the agent makes an MCP call, it simply receives the exact, precise answer it needs without having to burn tokens reading massive files or doing the heavy lifting itself.

## Why use a Semantic Graph?

When AI agents rely on traditional text-search tools (like `grep` or `cat`) to navigate a codebase, they often consume large amounts of their context window reading full files just to understand structure. `icnow` provides a structured, queryable map of your project instead.

- **Token Efficiency** — Instead of reading a full file to find its methods, agents can query `icnow` to retrieve a lightweight structural outline.
- **Semantic Navigation** — Agents don't have to rely on regex patterns. They can query explicit semantic relationships like *"Find all nodes that call the `authenticate` function."*
- **Context Assembly** — Agents can trace multi-hop dependency chains (e.g., *A calls B, which inherits from C*) in a single graph query.
- **Improved Performance** — By minimizing input tokens, LLM API calls can become more cost-effective and responses return faster, while reducing context-loss issues.
- **Recurring Cost, Not One-Time** — Every LLM API call sends the entire context window to the model on every turn. A file read with `cat` or `view_file` doesn't just cost tokens once — those characters stay in context and are re-sent (and re-billed) on every subsequent message for the rest of the session. `icnow` returns only the precise answer needed (~200 chars vs ~2,000 for a full file read), keeping the context lean across the entire session. Over a 10-turn coding session, this compounds to a **~10× difference in total token spend**.

## Two Modes of Operation

`icnow` operates flexibly depending on your system setup and needs:

1. **Default Mode:** Out of the box, `icnow` scans your codebase instantly using advanced syntax analysis. It is incredibly fast, requires zero configuration, and immediately gives your agent a highly accurate map of functions, classes, and their relationships. A lightweight background watcher keeps this graph constantly up to date as you code.
2. **Deep Scan Mode:** For users who need granular, compiler-level precision (such as resolving dynamic types or complex macros), agents can invoke the `deep_scan` tool. This mode leverages the Language Server Protocol (LSP). It takes significantly longer to run and **will fail if you do not have an LSP installed and configured** for your project's language. Most workflows work perfectly in Default Mode, so you can safely ignore `deep_scan` unless your agent explicitly needs it and you have an LSP ready.

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

## ⚡ Zero-Config AST Parsing
`icnow` natively extracts functions, classes, and their exact relationships across your codebase with zero external toolchain dependencies. It uses Tree-sitter to build accurate `CALLS`, `IMPLEMENTS`, and `DEPENDS_ON` relationships on the fly!

**Currently Supported Languages:**
- **Python** *(Extracts Pydantic/Dataclass structural dependencies & type hints)*
- **Go** *(Extracts type aliases & composite types)*
- **TypeScript / JavaScript** *(Extracts Interface implementations)*
- **React (TSX / JSX)**
- **Rust**
- **Ruby**

## Get started

### 1. Install `icnow`

You can install the latest compiled release directly to your machine:

**macOS & Linux:**
```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/leizzer/icnow/releases/latest/download/icnow-installer.sh | sh
```

**Windows:**
```powershell
irm https://github.com/leizzer/icnow/releases/latest/download/icnow-installer.ps1 | iex
```

### 2. Configure your AI Agent

`icnow` comes with a built-in installer to automatically configure your favorite AI agent for global access across all your projects:

```bash
# For Antigravity (global SKILL.md setup)
icnow install-skill antigravity

# For Cursor (global User settings.json injection)
icnow install-skill cursor

# For Claude Code (global MCP registration)
icnow install-skill claude

# For OpenAI/ChatGPT (prints Custom Instructions for you to copy)
icnow install-skill openai
```

*(Note: The `install-skill` command assumes a global installation so `icnow` is available seamlessly across any codebase you work in.)*

### 3. Uninstall

To completely remove `icnow` global configurations, including the shared `~/.icnow/` directory used to store the centralized graph databases, run:

```bash
icnow uninstall
```

## Database Storage & Backup

By default, `icnow` keeps your project directory 100% clean. It hashes your project path and centrally stores the graph database (`knowledge.db`) at `~/.icnow/projects/<hash>/`. You never have to worry about adding anything to your `.gitignore`.

**Embedded Mode:** If you prefer to store the database locally within your project (e.g., for small repositories where you want to commit the graph data), simply create an empty `.icnow/` directory in your project root. `icnow` will automatically detect this and switch to embedded mode, saving the database safely inside that folder.

Additionally, `icnow` automatically serializes and backs up all your LLM **Memories** to a JSON file on every write, ensuring you never lose your agent's knowledge even if the core graph database is completely wiped or corrupted.

## 🏎️ Benchmark Proof: Grep vs. Graph

We benchmarked a standard AI agent (restricted to standard Linux tools like `grep` and `cat`) against an `icnow` agent (restricted exclusively to MCP graph queries) on a real-world onboarding task. 

| Metric | Traditional Agent (grep) | `icnow` Agent (graph) | Advantage |
|--------|-------------------|-------------------|-----------|
| **Input Burden** | 285 chars | 1,095 chars | Traditional |
| **Context Burden (Output)** | 32,483 chars | **2,828 chars** | **`icnow` (91% context savings)** |

By traversing semantic edges (`DEPENDS_ON`, `CALLS`, `IMPLEMENTS`) rather than blindly reading huge chunks of files, `icnow` retrieved the exact same answer while keeping the LLM's context window pristine. In massive codebases, this 91% context reduction prevents LLM hallucinations, avoids context-loss, and drastically cuts token costs.

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
- Work in a language entirely unsupported by `icnow` (though adding support requires just 1 `.scm` file).

## Community & License

Apache 2.0 / MIT — see [LICENSE](LICENSE).
