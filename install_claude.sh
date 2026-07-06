#!/bin/bash

CLAUDE_CONFIG="$HOME/Library/Application Support/Claude/claude_desktop_config.json"
ICNOW_BIN="$(pwd)/target/release/icnow"

echo "Building icnow..."
cargo build --release

if [ ! -f "$CLAUDE_CONFIG" ]; then
    echo "Creating Claude config at $CLAUDE_CONFIG..."
    mkdir -p "$(dirname "$CLAUDE_CONFIG")"
    echo '{"mcpServers": {}}' > "$CLAUDE_CONFIG"
fi

echo "Adding icnow to Claude Desktop config..."

# Use python to safely update the JSON config
python3 -c "
import json
import sys

config_path = sys.argv[1]
bin_path = sys.argv[2]

try:
    with open(config_path, 'r') as f:
        config = json.load(f)
except Exception:
    config = {}

if 'mcpServers' not in config:
    config['mcpServers'] = {}

config['mcpServers']['icnow'] = {
    'command': bin_path,
    'args': []
}

with open(config_path, 'w') as f:
    json.dump(config, f, indent=2)

print('Success!')
" "$CLAUDE_CONFIG" "$ICNOW_BIN"

echo "Done! Please restart Claude Desktop to load the icnow MCP server."
