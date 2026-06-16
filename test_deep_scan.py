import json
import subprocess
import sys

def send_request(proc, method, params=None, req_id=1):
    req = {
        "jsonrpc": "2.0",
        "id": req_id,
        "method": method,
    }
    if params is not None:
        req["params"] = params
    
    msg = json.dumps(req) + "\n"
    proc.stdin.write(msg.encode('utf-8'))
    proc.stdin.flush()

    while True:
        line = proc.stdout.readline()
        if not line:
            return None
        line_str = line.decode('utf-8').strip()
        if not line_str:
            continue
        if line_str.startswith("{"):
            try:
                parsed = json.loads(line_str)
                if "id" in parsed and parsed["id"] == req_id:
                    return parsed
            except Exception as e:
                pass

def main():
    subprocess.run(["rm", "-rf", "/Users/cristian/Projects/dgapp_bkp/knowledge.db*"])

    proc = subprocess.Popen(
        ["./target/release/icnow"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=sys.stderr
    )

    send_request(proc, "initialize", {
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {"name": "test", "version": "1.0.0"}
    }, req_id=1)
    
    proc.stdin.write(b'{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
    proc.stdin.flush()

    print("Running deep_scan...")
    resp = send_request(proc, "tools/call", {
        "name": "deep_scan",
        "arguments": {
            "project_root": "/Users/cristian/Projects/dgapp_bkp"
        }
    }, req_id=3)
    print("Response from deep_scan:", json.dumps(resp, indent=2))

    proc.terminate()

if __name__ == "__main__":
    main()
