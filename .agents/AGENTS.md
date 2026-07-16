
# ICNOW Development Guidelines

## Modifying SKILL.md
When asked to modify or update `SKILL.md` (or its references) for the `icnow` tool, keep in mind that the SKILL file is **dynamically installed onto the user's machine** via the `icnow` binary during MCP installation. 

Therefore, any time you add a new file to the `.agents/skills/icnow/` directory or change its structure, you MUST also update `src/installer.rs` to ensure the new files are correctly compiled into the binary (using `include_str!`) and copied to the user's local `~/.gemini/config/skills/icnow/` directory on execution!

## Local Testing & Releasing
The user maintains two separate installations of `icnow`:
1. **Systemwide**: Installed via the README to the user's local path (`~/.local/bin/icnow`). This is used by external tools like Claude Code or Cursor.
2. **Antigravity (Test Build)**: Antigravity is configured to run the `icnow` MCP server directly from the compiled release binary inside this project folder (`target/release/icnow`).

When the user asks you to **"release it locally"** or **"build so we can use the latest changes locally"**, they mean you should run `cargo build --release` in this directory so that Antigravity will pick up the new binary upon restart. You do not need to install it globally for these requests.
