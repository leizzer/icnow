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
tool_req = {
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/call",
    "params": {
        "name": "parse_project_file",
        "arguments": {
            "file_path": "src/models.rs"
        }
    }
}
print("\n=> Calling 'parse_project_file' tool for src/models.rs")
print("<= Response:", json.dumps(send_req(proc, tool_req), indent=2))

# Cleanup
proc.terminate()
print("\nDone.")
