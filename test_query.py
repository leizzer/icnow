import subprocess
import json

proc = subprocess.Popen(["cargo", "run", "-q", "--bin", "icnow"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)

req = """{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "test", "version": "1.0"}}}
{"jsonrpc": "2.0", "method": "notifications/initialized"}
{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "query_graph_cypher", "arguments": {"query": "MATCH (n) WHERE n.id STARTS WITH '/Users/' RETURN n.id LIMIT 1", "project_root": "/Users/cristian/Projects/blackhole/icnow"}}}
"""
stdout, _ = proc.communicate(req)
print(stdout)
