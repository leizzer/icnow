use anyhow::{Context, Result};
use std::process::Command;

pub fn run_installer(target: &str) -> Result<()> {
    match target {
        "antigravity" => install_antigravity(),
        "claude" => install_claude(),
        "cursor" => install_cursor(),
        "openai" => install_openai(),
        _ => {
            tracing::error!(
                "Unknown installation target: {}. Supported targets: antigravity, claude, cursor, openai",
                target
            );
            std::process::exit(1);
        }
    }
}

fn install_antigravity() -> Result<()> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let target_dir = home.join(".gemini/config/skills/icnow");
    std::fs::create_dir_all(&target_dir).context("Failed to create skill directory")?;

    let target_file = target_dir.join("SKILL.md");
    let content = include_str!("../.agents/skills/icnow/SKILL.md");

    std::fs::write(&target_file, content).context("Failed to write SKILL.md")?;
    tracing::info!(
        "Successfully installed icnow skill to {}",
        target_file.display()
    );
    Ok(())
}

fn install_claude() -> Result<()> {
    tracing::info!("Registering icnow MCP server with Claude Code globally...");

    // claude mcp add icnow "icnow"
    let status = Command::new("claude")
        .arg("mcp")
        .arg("add")
        .arg("icnow")
        .arg("icnow")
        .status()
        .context(
            "Failed to run 'claude' command. Make sure Claude Code is installed and in your PATH.",
        )?;

    if status.success() {
        tracing::info!("Successfully registered icnow with Claude Code!");
        Ok(())
    } else {
        anyhow::bail!("claude mcp add command failed with exit code: {status}");
    }
}

fn install_cursor() -> Result<()> {
    // Cursor settings are stored in different places depending on the OS.
    // For now, we will locate the global settings.json and append icnow instructions to cursor.general.rulesForAi
    let home = dirs::home_dir().context("Could not determine home directory")?;

    let settings_path = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Cursor/User/settings.json")
    } else if cfg!(target_os = "linux") {
        home.join(".config/Cursor/User/settings.json")
    } else if cfg!(target_os = "windows") {
        home.join("AppData/Roaming/Cursor/User/settings.json")
    } else {
        anyhow::bail!("Unsupported OS for automatic Cursor installation");
    };

    if !settings_path.exists() {
        anyhow::bail!(
            "Cursor settings.json not found at {}. Have you installed Cursor?",
            settings_path.display()
        );
    }

    let settings_content =
        std::fs::read_to_string(&settings_path).context("Failed to read Cursor settings.json")?;

    let mut json: serde_json::Value = serde_json::from_str(&settings_content)
        .context("Failed to parse Cursor settings.json as JSON")?;

    let rules_key = "cursor.general.rulesForAi";
    let icnow_instruction = "When you need to explore this codebase, use the `icnow` MCP server to query the architecture, read code, and search for symbols.";

    if let Some(obj) = json.as_object_mut() {
        if let Some(existing_rules) = obj.get(rules_key).and_then(|v| v.as_str()) {
            if !existing_rules.contains("icnow") {
                let new_rules = format!("{existing_rules}\n\n{icnow_instruction}");
                obj.insert(rules_key.to_string(), serde_json::Value::String(new_rules));
            } else {
                tracing::info!("icnow rules already exist in Cursor settings!");
                return Ok(());
            }
        } else {
            obj.insert(
                rules_key.to_string(),
                serde_json::Value::String(icnow_instruction.to_string()),
            );
        }
    } else {
        anyhow::bail!("Cursor settings.json is not a valid JSON object");
    }

    let updated_content =
        serde_json::to_string_pretty(&json).context("Failed to serialize updated settings.json")?;

    std::fs::write(&settings_path, updated_content)
        .context("Failed to write updated settings.json")?;

    tracing::info!(
        "Successfully added icnow instructions to global Cursor settings at {}",
        settings_path.display()
    );

    Ok(())
}

fn install_openai() -> Result<()> {
    // OpenAI does not have a local config file to inject MCP servers or Custom Instructions automatically.
    // We will just print the custom instructions to the terminal for the user.
    let instructions = "
---
# icnow Configuration
To effectively navigate this codebase, use the `icnow` MCP server (if supported by your client). 
`icnow` provides a 360-degree context of code definitions, incoming dependencies, and call paths.
---";
    println!(
        "\nOpenAI/ChatGPT doesn't have a local configuration file for tools yet. \nPlease paste the following into your Custom Instructions or Agent setup:\n{instructions}"
    );
    Ok(())
}

pub fn run_uninstall() -> Result<()> {
    let home = dirs::home_dir().context("Could not determine home directory")?;

    // Remove the ~/.icnow directory
    let icnow_dir = home.join(".icnow");
    if icnow_dir.exists() {
        tracing::info!(
            "Removing global icnow data directory: {}",
            icnow_dir.display()
        );
        if let Err(e) = std::fs::remove_dir_all(&icnow_dir) {
            tracing::warn!("Failed to completely remove {}: {}", icnow_dir.display(), e);
        } else {
            tracing::info!("Successfully removed {}", icnow_dir.display());
        }
    } else {
        tracing::info!(
            "No global icnow data directory found at {}",
            icnow_dir.display()
        );
    }

    // Remove the ~/.gemini/config/skills/icnow directory
    let antigravity_skill_dir = home.join(".gemini/config/skills/icnow");
    if antigravity_skill_dir.exists() {
        tracing::info!(
            "Removing Antigravity skill directory: {}",
            antigravity_skill_dir.display()
        );
        if let Err(e) = std::fs::remove_dir_all(&antigravity_skill_dir) {
            tracing::warn!(
                "Failed to completely remove {}: {}",
                antigravity_skill_dir.display(),
                e
            );
        } else {
            tracing::info!("Successfully removed {}", antigravity_skill_dir.display());
        }
    } else {
        tracing::info!(
            "No Antigravity skill directory found at {}",
            antigravity_skill_dir.display()
        );
    }

    // Attempt to remove Claude MCP server
    tracing::info!(
        "Attempting to remove Claude MCP server (this is safe to fail if not installed)..."
    );
    let _ = Command::new("claude")
        .arg("mcp")
        .arg("remove")
        .arg("icnow")
        .status();

    tracing::info!("Uninstall complete.");
    Ok(())
}
