extern crate openssl;

use anyhow::Result;
use clap::{Parser, Subcommand};
use icnow::resources::ResourceHandler;
use icnow::tools::GraphService;
use rmcp::{transport::stdio, ServiceExt};
use tracing_subscriber::{self, EnvFilter};

#[derive(Parser)]
#[command(name = "icnow", version, about = "ICNOW: Interactive Code Network & Object Workspace MCP server")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to the knowledge.db file or directory
    db_path: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Install an MCP skill for a specific AI assistant
    InstallSkill {
        /// Target AI assistant (antigravity, claude, cursor, openai)
        target: String,
    },
    /// Uninstall the ICNOW MCP server and skills
    Uninstall,
    /// Update the ICNOW installation
    Update,
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting icnow Graph MCP server...");
}

fn resolve_db_path(current_dir: &std::path::Path, provided_path: Option<String>) -> String {
    if let Some(path) = provided_path {
        path
    } else {
        current_dir
            .join("knowledge.db")
            .to_string_lossy()
            .to_string()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::InstallSkill { target }) => {
            if let Err(e) = icnow::installer::run_installer(&target) {
                eprintln!("Installation failed: {e:?}");
                std::process::exit(1);
            }
            return Ok(());
        }
        Some(Commands::Uninstall) => {
            if let Err(e) = icnow::installer::run_uninstall() {
                eprintln!("Uninstall failed: {e:?}");
                std::process::exit(1);
            }
            return Ok(());
        }
        Some(Commands::Update) => {
            if let Err(e) = icnow::installer::run_update() {
                eprintln!("Update failed: {e:?}");
                std::process::exit(1);
            }
            return Ok(());
        }
        None => {}
    }

    init_tracing();

    let current_dir = std::env::current_dir()?;
    let db_path = resolve_db_path(&current_dir, cli.db_path);

    let service = GraphService {
        db_path: db_path.clone(),
    };
    let resource_service = ResourceHandler::new(service);
    let service_handle = resource_service.serve(stdio()).await?;

    icnow::indexer::watcher::ensure_watching(&current_dir, &db_path);

    service_handle.waiting().await?;

    tracing::info!("Server has shut down.");

    Ok(())
}
