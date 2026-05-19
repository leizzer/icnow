import subprocess
import json
import sys

def send_req(proc, req):
    line = json.dumps(req) + "\n"
    proc.stdin.write(line.encode('utf-8'))
    proc.stdin.flush()
    # Read response
    while True:
        res = proc.stdout.readline()
        if not res:
            return None
        res_str = res.decode('utf-8').strip()
        # Ignore tracing logs that might get mixed into stdout by accident (usually stderr, but just in case)
        if res_str.startswith("{"):
            try:
                return json.loads(res_str)
            except:
                pass
        else:
            print("SERVER LOG:", res_str)

print("Starting MCP Server...")
proc = subprocess.Popen(["cargo", "run", "-q", "--bin", "icnow"], 
                        stdin=subprocess.PIPE, 
                        stdout=subprocess.PIPE,
                        stderr=subprocess.PIPE) # hide stderr logs to keep clean

# 1. Initialize Handshake
init_req = {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {"name": "antigravity", "version": "1.0.0"}
    }
}
print("\n=> Sending 'initialize'")
print("<= Response:", json.dumps(send_req(proc, init_req), indent=2))

# 2. Initialized Notification
init_notif = {
    "jsonrpc": "2.0",
    "method": "notifications/initialized"
}
proc.stdin.write((json.dumps(init_notif) + "\n").encode('utf-8'))
proc.stdin.flush()
print("\n=> Sent 'notifications/initialized'")

# 3. Call Tool
print("\n=> Calling 'parse_project_file' tool for webhooks_controller.rb")
parse_req = {
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/call",
    "params": {
        "name": "parse_project_file",
        "arguments": {
            "file_path": "/Users/cristian/Projects/dgapp_bkp/app/controllers/api/v2/webhooks_controller.rb"
        }
    }
}
proc.stdin.write((json.dumps(parse_req) + "\n").encode('utf-8'))
proc.stdin.flush()

response = proc.stdout.readline()
print("<= Response:", json.dumps(json.loads(response), indent=2))

print("\n=> Calling 'query_graph' tool to extract source code for Edge::save")
query_req = {
    "jsonrpc": "2.0",
    "id": 3,
    "method": "tools/call",
    "params": {
        "name": "query_graph",
        "arguments": {
            "query": "SELECT s.value as source_code FROM nodes n JOIN node_props_text s ON n.id = s.node_id AND s.key_id = (SELECT id FROM property_keys WHERE key='source_code') JOIN node_props_text id_prop ON n.id = id_prop.node_id AND id_prop.key_id = (SELECT id FROM property_keys WHERE key='id') WHERE id_prop.value = 'src/models.rs::Edge::save';"
        }
    }
}
proc.stdin.write((json.dumps(query_req) + "\n").encode('utf-8'))
proc.stdin.flush()

response = proc.stdout.readline()
print("<= Response:", json.dumps(json.loads(response), indent=2))

print("\n=> Calling 'traverse_graph' tool for webhooks_controller.rb starting node")
traverse_req = {
    "jsonrpc": "2.0",
    "id": 4,
    "method": "tools/call",
    "params": {
        "name": "traverse_graph",
        "arguments": {
            "node_id": "/Users/cristian/Projects/dgapp_bkp/app/controllers/api/v2/webhooks_controller.rb",
            "max_depth": 3
        }
    }
}
proc.stdin.write((json.dumps(traverse_req) + "\n").encode('utf-8'))
proc.stdin.flush()

response = proc.stdout.readline()
print("<= Response:", json.dumps(json.loads(response), indent=2))

print("\n=> Calling 'query_graph_cypher' tool with a native Cypher MATCH query")
cypher_req = {
    "jsonrpc": "2.0",
    "id": 5,
    "method": "tools/call",
    "params": {
        "name": "query_graph_cypher",
        "arguments": {
            "query": "MATCH (c:Class)-[:HAS_METHOD]->(m:Method) RETURN c.id, m.id LIMIT 3"
        }
    }
}
proc.stdin.write((json.dumps(cypher_req) + "\n").encode('utf-8'))
proc.stdin.flush()

response = proc.stdout.readline()
print("<= Response:", json.dumps(json.loads(response), indent=2))

# Cleanup
proc.terminate()
print("\nDone.")
