<div align="center"><pre>
  ██╗ ██████╗███╗   ██╗██████╗ ██╗    ██╗
  ██║██╔════╝████╗  ██║██╔═══██╗██║    ██║
  ██║██║     ██╔██╗ ██║██║   ██║██║ █╗ ██║
  ██║██║     ██║╚██╗██║██║   ██║██║███╗██║
  ██║╚██████╗██║ ╚████║╚██████╔╝╚███╔███╔╝
  ╚═╝ ╚═════╝╚═╝  ╚═══╝ ╚═════╝  ╚══╝╚══╝ 
      The code knowledge graph MCP server for AI agents
</pre></div>

<p align="center"><strong>10–80x faster codebase traversal · Rust · Ladybug · AST parsing · Local-first · Zero-config</strong></p>

<p align="center">
  <a href="https://crates.io/crates/icnow"><img src="https://img.shields.io/crates/v/icnow.svg" alt="Crates.io"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT"></a>
</p>

<p align="center">
  <a href="#get-started-60-seconds">Install</a> ·
  <a href="#proof">Proof</a> ·
  <a href="#agent-compatibility-matrix">Agents</a> ·
  <a href="https://github.com/leizzer/icnow">GitHub</a>
</p>

---

`icnow` compresses codebase discovery by representing your entire project as an instantly queryable graph database. AI agents stop blindly grepping and start resolving callers, subclasses, and dependencies with 100% precision in a fraction of the time. Same answers, 90% fewer tokens.

## What it does

- **MCP server** — Native Model Context Protocol server exposing `search_symbols`, `get_symbol_info`, `deep_scan`, and `query_graph_cypher`.
- **AST Parsing** — Tree-sitter natively extracts functions, classes, and imports across Rust, Ruby, TypeScript, and more—with zero external toolchain dependencies.
- **Graph Database** — Ladybug powers lightning-fast openCypher edge traversals.
- **Replaces Grep** — Agents query structured edges (`CALLS`, `INHERITS`, `IMPORTS`) instead of hallucinating regexes.
- **Isolated & Local** — Stores `knowledge.db` entirely locally inside your project root. 

## How it works (30 seconds)

```
 Your agent / IDE
   (Claude Code, Cursor, Antigravity, Aider…)
        │   MCP tool calls · cypher queries
        ▼
    ┌────────────────────────────────────────────────────┐
    │  icnow   (runs locally in your project root)       │
    │  ────────────────────────────────────────────────  │
    │  DeepScan  →  Tree-sitter AST  →  knowledge.db     │
    │                    ├─ Node Extraction (Symbols)    │
    │                    ├─ Edge Resolution (Imports)    │
    │                    └─ Ladybug Graph Engine         │
    │                                                    │
    │  search_symbols · get_symbol_info · query_graph    │
    └────────────────────────────────────────────────────┘
        │   structured JSON graph responses
        ▼
 LLM provider  (Anthropic · OpenAI · Bedrock · …)
```

- **DeepScan** — Parses the codebase asynchronously in 50-item transaction chunks.
- **Node Extraction** — Automatically identifies `File`, `Function`, `Class`, and `Model` nodes.
- **Edge Resolution** — Draws the `CALLS`, `INHERITS`, `IMPORTS`, and `CONTAINS` lines connecting the project.
- **openCypher Engine** — Traverses the graph at millisecond speeds.

## Get started (60 seconds)

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

<details>
<summary><b>Cypher Querying Guide</b></summary>

We use **openCypher** via Ladybug as the primary graph query language for retrieving relationships, code patterns, and semantic dependencies.

**List Classes and Their Methods:**
```cypher
MATCH (c:Class)-[:CONTAINS]->(m:Method)
RETURN c.id, m.id
LIMIT 10
```

**Find Callers of a Function:**
```cypher
MATCH (caller)-[r:CALLS]->(callee)
WHERE callee.id = 'src/auth/user.ts::User#verify_token'
RETURN caller.id, label(r)
```

**Find Dependencies of a File:**
```cypher
MATCH (file:File)-[r:IMPORTS]->(dep)
WHERE file.id = 'src/main.rs'
RETURN dep.id
```
</details>

<details>
<summary><b>Data Schema</b></summary>

### Nodes
Nodes represent files, functions, classes, models, or imports.
- **`id`**: Must be a globally unique string (e.g., `src/models.rs::Node` or `src/auth/user.ts::User`).
- **`label`**: The domain-level type (e.g., `Function`, `Struct`, `File`, `Model`).
- **`kind`**: The specific AST syntax item (e.g., `function_item`, `class_declaration`).

### Edges
Edges represent the relationships between two nodes.
- **`source` / `target`**: Exact String `id`s of the connected Nodes.
- **`label`**: The relationship type (e.g., `CALLS`, `IMPORTS`, `REFERENCES`, `BELONGS_TO`, `CONTAINS`, `INHERITS`).

</details>

## Community & License

Apache 2.0 / MIT — see [LICENSE](LICENSE).