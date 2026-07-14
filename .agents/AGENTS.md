
# ICNOW Development Guidelines

## Modifying SKILL.md
When asked to modify or update `SKILL.md` (or its references) for the `icnow` tool, keep in mind that the SKILL file is **dynamically installed onto the user's machine** via the `icnow` binary during MCP installation. 

Therefore, any time you add a new file to the `.agents/skills/icnow/` directory or change its structure, you MUST also update `src/installer.rs` to ensure the new files are correctly compiled into the binary (using `include_str!`) and copied to the user's local `~/.gemini/config/skills/icnow/` directory on execution!
