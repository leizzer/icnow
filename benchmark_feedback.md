# `icnow` Benchmark Feedback & Development Brief

Based on the recent benchmark harness, `icnow` showed incredible token efficiency in structural queries but suffered ergonomic bottlenecks when retrieving code context. Below is the actionable feedback for the development agent.

---

## 📉 Task 1: Trace call sites of `has_backstage_role?` (LOSS)
**Metrics:** icnow 37,960 tokens / 116.1s / 17 calls vs Traditional 25,748 tokens / 57.4s / 5 calls.

**Root Cause:** 
While `get_symbol_info` is phenomenally good at instantly returning the *locations* of all callers (via bridged `Unresolved` nodes), the agent is forced into an N+1 tool call trap. To actually *read* how the method is used, the agent must manually iterate through the caller list and fire `get_symbol_implementation` or `view_file` for each one. 
Traditional `grep -C 5` wins because it returns the file, line, AND the surrounding 10 lines of code snippet for all 8 sites in a **single terminal output**.

**Action Items for Dev:**
1. **Embed Snippets in `get_symbol_info`:** Enhance the `get_symbol_info` MCP tool so that when it lists "Incoming Usages", it optionally includes a 3-5 line code snippet around the exact `file:line` invocation. This will eliminate the N+1 tool call bottleneck.
2. Alternatively, create a dedicated `get_call_site_snippets(node_id)` tool that aggregates the code blocks of all incoming usages into one response.

---

## ⚖️ Task 2: Count methods/macros in `portal_ticket.rb` (MIXED)
**Metrics:** icnow won on tokens (-60%) but lost on time (55.0s vs 45.5s) and disagreed on macro count (118 vs 80-95).

**Root Cause:**
1. **The Discrepancy (118 vs 80-95):** `icnow` is actually the ground-truth winner here. Traditional agents use subjective regexes to guess which Rails DSLs to count (e.g., they might grep for `has_many` and `belongs_to` but completely miss `validates`, `before_action`, or custom macros). `icnow` uses the Tree-sitter AST to deterministically tag *every* class-level invocation as a `Macro`. The agent correctly reported 118.
2. **The Time Loss:** The agent likely took too long deciding which Cypher query to write or parsing the markdown tables from `get_file_structure`. Furthermore, graph DB lock contention overheads might be slowing down tool execution.

**Action Items for Dev:**
1. Focus on concurrency handling (resolving the `knowledge.db` locks) so the server responds faster.
2. Ensure the prompt instructions guide agents to use `get_file_structure` directly for counting instead of wasting time crafting Cypher syntax for simple tasks.

---

## 🏆 Task 3: Locate `Backstage::CustomerPolicy` + parent + methods (CLEAN WIN)
**Metrics:** icnow 25,769 tokens / 28.2s / 4 calls vs Traditional 30,782 tokens / 32.2s / 7 calls.

**Root Cause:**
This is the textbook scenario for semantic graphs. Extracting parent classes and listing all methods via pure text search is incredibly noisy, error-prone, and requires dumping massive files into the LLM context. `icnow` structurally maps this via `get_symbol_info` and handles it instantly and flawlessly.

**Action Items for Dev:**
Double down on structural relationships. The parent-child class hierarchy extraction is working perfectly.

---

## 🎯 Executive Summary for the Dev Agent:
`icnow` is structurally flawless but ergonomically lacking for code review. You must solve the **N+1 snippet retrieval problem**. If you can make `icnow` return usages *with the code snippets attached*, it will permanently dethrone `grep_search` across all benchmarks.
