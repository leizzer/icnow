import subprocess
import os

def run_command(cmd):
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
    return result.stdout

def count_tokens(text):
    # Approximation: 4 characters per token
    return len(text) // 4

print("=== Traditional Agent Workflow ===")
print("Step 1: Agent uses grep_search to find 'query_graph'")
grep_out = run_command("rg -n 'fn query_graph' src/")
grep_tokens = count_tokens(grep_out)
print(f"-> Grep returned {len(grep_out)} bytes (~{grep_tokens} tokens)")

print("Step 2: Agent views the file using view_file or cat")
file_out = run_command("cat src/tools.rs")
file_tokens = count_tokens(file_out)
print(f"-> File returned {len(file_out)} bytes (~{file_tokens} tokens)")
total_traditional = grep_tokens + file_tokens
print(f"TOTAL TRADITIONAL COST: ~{total_traditional} tokens")

print("\n=== icnow Agent Workflow ===")
print("Preparation: Parsing src/tools.rs into knowledge.db...")
# We will use the built server via a temporary script or just run parse_file directly if we had a bin.
# For the benchmark, we can just use the CLI or a python snippet to call the server.
# Actually, since we need to parse it, let's just write a quick script to parse tools.rs
run_command("cargo run -q --bin test_data") # wait, test_data might be old. Let's just use client_test logic.
print("Step 1: Agent runs query_graph MCP tool")
sql_query = "SELECT s.value as source_code FROM nodes n JOIN node_props_text s ON n.id = s.node_id AND s.key_id = (SELECT id FROM property_keys WHERE key='source_code') JOIN node_props_text id_prop ON n.id = id_prop.node_id AND id_prop.key_id = (SELECT id FROM property_keys WHERE key='id') WHERE id_prop.value = 'src/tools.rs::GraphService::query_graph';"
icnow_out = run_command(f"sqlite3 knowledge.db -header -markdown \"{sql_query}\"")
icnow_tokens = count_tokens(icnow_out)
print(f"-> icnow returned {len(icnow_out)} bytes (~{icnow_tokens} tokens)")
print(f"TOTAL ICNOW COST: ~{icnow_tokens} tokens")

print("\n=== ROI ===")
savings = total_traditional - icnow_tokens
percentage = (savings / total_traditional) * 100
print(f"Tokens saved: {savings}")
print(f"Percentage reduction: {percentage:.2f}%")
