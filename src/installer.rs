use anyhow::{Context, Result};
use std::process::Command;

/// Returns the absolute path of the currently running icnow binary.
fn resolve_binary_path() -> Result<String> {
    let exe = std::env::current_exe().context("Could not determine current executable path")?;
    Ok(exe.to_string_lossy().to_string())
}

/// Ensures ~/.local/bin is registered in the system PATH for ALL processes,
/// including GUI apps like Claude Desktop or Cursor.
///
/// - macOS: writes /etc/paths.d/icnow (requires sudo)
/// - Linux: appends to ~/.profile
fn ensure_path_registered() {
    let bin_dir = match dirs::home_dir() {
        Some(h) => h.join(".local/bin"),
        None => return,
    };
    let bin_dir_str = bin_dir.to_string_lossy().to_string();

    if cfg!(target_os = "macos") {
        let paths_d = std::path::Path::new("/etc/paths.d/icnow");
        if !paths_d.exists() {
            println!("Registering ~/.local/bin in /etc/paths.d/icnow (requires sudo)...");
            let status = Command::new("sudo")
                .arg("sh")
                .arg("-c")
                .arg(format!("echo '{}' > /etc/paths.d/icnow", bin_dir_str))
                .status();
            match status {
                Ok(s) if s.success() => {
                    println!("✓ /etc/paths.d/icnow created — ~/.local/bin is now in PATH for all apps (including GUI apps).");
                }
                _ => {
                    eprintln!("⚠ Could not write /etc/paths.d/icnow. GUI apps may not find icnow automatically.");
                    eprintln!("  You can fix this manually by running:");
                    eprintln!("  echo '{}' | sudo tee /etc/paths.d/icnow", bin_dir_str);
                }
            }
        } else {
            println!("✓ /etc/paths.d/icnow already exists — PATH is configured.");
        }
    } else if cfg!(target_os = "linux") {
        if let Some(home) = dirs::home_dir() {
            let profile = home.join(".profile");
            let export_line = format!("\nexport PATH=\"{}:$PATH\"  # added by icnow installer", bin_dir_str);
            let already_set = std::fs::read_to_string(&profile)
                .map(|c| c.contains(&bin_dir_str))
                .unwrap_or(false);
            if !already_set {
                if let Err(e) = std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(&profile)
                    .and_then(|mut f| { use std::io::Write; f.write_all(export_line.as_bytes()) })
                {
                    eprintln!("⚠ Could not update ~/.profile: {e}");
                } else {
                    println!("✓ Added ~/.local/bin to ~/.profile — restart your session to apply.");
                }
            } else {
                println!("✓ ~/.local/bin already in ~/.profile.");
            }
        }
    }
}

pub fn run_installer(target: &str) -> Result<()> {
    // Always ensure ~/.local/bin is in PATH system-wide before configuring any agent.
    ensure_path_registered();

    match target {
        "antigravity" => install_antigravity(),
        "claude" => install_claude(),
        "cursor" => install_cursor(),
        "openai" => install_openai(),
        _ => {
            eprintln!(
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
    println!("File modified: {}", target_file.display());

    let references_dir = target_dir.join("references");
    std::fs::create_dir_all(&references_dir).context("Failed to create references directory")?;

    let cypher_target = references_dir.join("cypher_examples.md");
    let cypher_content = include_str!("../.agents/skills/icnow/references/cypher_examples.md");
    std::fs::write(&cypher_target, cypher_content).context("Failed to write cypher_examples.md")?;

    let tools_target = references_dir.join("tool_arsenal.md");
    let tools_content = include_str!("../.agents/skills/icnow/references/tool_arsenal.md");
    std::fs::write(&tools_target, tools_content).context("Failed to write tool_arsenal.md")?;

    // Also inject the MCP server configuration into mcp_config.json
    let mcp_config_path = home.join(".gemini/config/mcp_config.json");
    
    let mut json: serde_json::Value = if mcp_config_path.exists() {
        let content = std::fs::read_to_string(&mcp_config_path)
            .context("Failed to read mcp_config.json")?;
        serde_json::from_str(&content).context("Failed to parse mcp_config.json as JSON")?
    } else {
        serde_json::json!({ "mcpServers": {} })
    };

    if let Some(obj) = json.as_object_mut() {
        let mcp_servers = obj
            .entry("mcpServers".to_string())
            .or_insert_with(|| serde_json::json!({}));
            
        if let Some(servers_obj) = mcp_servers.as_object_mut() {
            if !servers_obj.contains_key("icnow") {
                // Use the absolute binary path so GUI apps (Antigravity IDE, etc.)
                // can find icnow regardless of their restricted $PATH.
                let bin_path = resolve_binary_path().unwrap_or_else(|_| "icnow".to_string());
                servers_obj.insert(
                    "icnow".to_string(),
                    serde_json::json!({
                        "command": bin_path
                    }),
                );
                
                let updated_content = serde_json::to_string_pretty(&json)
                    .context("Failed to serialize updated mcp_config.json")?;
                
                std::fs::write(&mcp_config_path, updated_content)
                    .context("Failed to write updated mcp_config.json")?;
                    
                println!("File modified: {}", mcp_config_path.display());
            } else {
                println!("icnow is already registered in {}", mcp_config_path.display());
            }
        }
    }

    Ok(())
}

fn install_claude() -> Result<()> {
    println!("Registering icnow MCP server with Claude Code globally...");

    // Use the absolute binary path so Claude Code (a GUI app) can find icnow
    // even though GUI apps on macOS/Linux don't inherit the shell's $PATH.
    let bin_path = resolve_binary_path().unwrap_or_else(|_| "icnow".to_string());
    println!("Using binary path: {bin_path}");

    let output = Command::new("claude")
        .arg("mcp")
        .arg("add")
        .arg("--scope")
        .arg("user")
        .arg("icnow")
        .arg(&bin_path)
        .output()
        .context(
            "Failed to run 'claude' command. Make sure Claude Code is installed and in your PATH.",
        )?;

    if output.status.success() {
        println!("Successfully registered icnow with Claude Code!");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stderr.contains("already exists") || stdout.contains("already exists") {
            println!("icnow is already registered with Claude Code.");
            Ok(())
        } else {
            anyhow::bail!(
                "claude mcp add command failed with exit code: {}. Error: {}",
                output.status,
                stderr
            );
        }
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
                println!("icnow rules already exist in Cursor settings!");
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

    println!(
        "File modified: {}",
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

    // Remove the icnow binary from ~/.local/bin
    let bin_path = home.join(".local/bin/icnow");
    if bin_path.exists() {
        println!("Removing binary: {}", bin_path.display());
        if let Err(e) = std::fs::remove_file(&bin_path) {
            eprintln!("Failed to remove {}: {}", bin_path.display(), e);
        } else {
            println!("Successfully removed {}", bin_path.display());
        }
    } else {
        println!("No binary found at {}", bin_path.display());
    }

    // macOS: remove /etc/paths.d/icnow
    if cfg!(target_os = "macos") {
        let paths_d = std::path::Path::new("/etc/paths.d/icnow");
        if paths_d.exists() {
            println!("Removing /etc/paths.d/icnow (requires sudo)...");
            let status = Command::new("sudo").arg("rm").arg("/etc/paths.d/icnow").status();
            match status {
                Ok(s) if s.success() => println!("Successfully removed /etc/paths.d/icnow"),
                _ => eprintln!("⚠ Could not remove /etc/paths.d/icnow. You may need to run: sudo rm /etc/paths.d/icnow"),
            }
        } else {
            println!("No /etc/paths.d/icnow found.");
        }
    }

    // Linux: remove the PATH export from ~/.profile
    if cfg!(target_os = "linux") {
        let profile = home.join(".profile");
        if profile.exists() {
            if let Ok(content) = std::fs::read_to_string(&profile) {
                let local_bin = home.join(".local/bin").to_string_lossy().to_string();
                let cleaned: String = content
                    .lines()
                    .filter(|l| !(l.contains(&local_bin) && l.contains("icnow installer")))
                    .collect::<Vec<_>>()
                    .join("\n");
                if cleaned != content {
                    if let Err(e) = std::fs::write(&profile, cleaned) {
                        eprintln!("⚠ Could not clean ~/.profile: {e}");
                    } else {
                        println!("Removed icnow PATH entry from ~/.profile");
                    }
                } else {
                    println!("No icnow entry found in ~/.profile");
                }
            }
        }
    }

    // Remove the ~/.icnow directory
    let icnow_dir = home.join(".icnow");
    if icnow_dir.exists() {
        println!(
            "Removing global icnow data directory: {}",
            icnow_dir.display()
        );
        if let Err(e) = std::fs::remove_dir_all(&icnow_dir) {
            eprintln!("Failed to completely remove {}: {}", icnow_dir.display(), e);
        } else {
            println!("Successfully removed {}", icnow_dir.display());
        }
    } else {
        println!(
            "No global icnow data directory found at {}",
            icnow_dir.display()
        );
    }

    // Remove the ~/.gemini/config/skills/icnow directory
    let antigravity_skill_dir = home.join(".gemini/config/skills/icnow");
    if antigravity_skill_dir.exists() {
        println!(
            "Removing Antigravity skill directory: {}",
            antigravity_skill_dir.display()
        );
        if let Err(e) = std::fs::remove_dir_all(&antigravity_skill_dir) {
            eprintln!(
                "Failed to completely remove {}: {}",
                antigravity_skill_dir.display(),
                e
            );
        } else {
            println!("Successfully removed {}", antigravity_skill_dir.display());
        }
    } else {
        println!(
            "No Antigravity skill directory found at {}",
            antigravity_skill_dir.display()
        );
    }

    // Attempt to remove Claude MCP server
    println!(
        "Attempting to remove Claude MCP server (this is safe to fail if not installed)..."
    );
    let _ = Command::new("claude")
        .arg("mcp")
        .arg("remove")
        .arg("icnow")
        .status();

    println!("Uninstall complete.");
    Ok(())
}

pub fn run_update() -> Result<()> {
    println!("Updating icnow to the latest version...");

    if cfg!(target_os = "windows") {
        // Windows: use PowerShell to download and run the installer script
        let url = "https://github.com/leizzer/icnow/releases/latest/download/icnow-installer.ps1";
        println!("Downloading installer from {url}");
        let status = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-Command")
            .arg(format!("irm '{url}' | iex"))
            .status()
            .context("Failed to run PowerShell. Make sure PowerShell is installed and in your PATH.")?;

        if status.success() {
            println!("icnow updated successfully!");
        } else {
            anyhow::bail!("Update failed with exit code: {status}");
        }
    } else {
        // macOS / Linux: use curl + sh
        let url = "https://github.com/leizzer/icnow/releases/latest/download/icnow-installer.sh";
        println!("Downloading installer from {url}");

        // Check curl is available
        let curl_check = Command::new("curl").arg("--version").output();
        if curl_check.is_err() {
            anyhow::bail!("'curl' is required but was not found in PATH. Please install curl and try again.");
        }

        let status = Command::new("sh")
            .arg("-c")
            .arg(format!(
                "curl --proto '=https' --tlsv1.2 -LsSf '{url}' | sh"
            ))
            .status()
            .context("Failed to run the update script.")?;

        if status.success() {
            println!("icnow updated successfully!");
        } else {
            anyhow::bail!("Update failed with exit code: {status}");
        }
    }

    Ok(())
}
