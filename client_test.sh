#!/bin/bash

echo "Building MCP Server to ensure clean output..."
cargo build

# The JSON-RPC 2.0 payload exactly as the MCP protocol defines for calling a tool
PAYLOAD='{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "parse_project_file",
    "arguments": {
      "file_path": "src/models.rs"
    }
  }
}'

echo "Sending JSON-RPC request to MCP Server over STDIO:"
echo "$PAYLOAD"
echo "--------------------------------------------------"

# Pipe the JSON payload directly into the running server and capture the response
echo "$PAYLOAD" | cargo run -q --bin icnow

echo ""
echo "--------------------------------------------------"
echo "Done."
