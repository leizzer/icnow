use anyhow::Result;
use icnow::tools::GraphService;
use icnow::resources::ResourceHandler;
use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber::{self, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting icnow Graph MCP server...");

    let current_dir = std::env::current_dir()?;

    // Parse db_path from command-line args if provided, otherwise default to Cwd/knowledge.db
    let args: Vec<String> = std::env::args().collect();
    let db_path = if args.len() > 1 {
        args[1].clone()
    } else {
        current_dir
            .join("knowledge.db")
            .to_string_lossy()
            .to_string()
    };

    let service = GraphService {
        db_path: db_path.clone(),
    };
    let resource_service = ResourceHandler::new(service);
    let service_handle = resource_service.serve(stdio()).await?;

    // Only watch current_dir if it looks like a valid project root (has .git, Cargo.toml, etc.)
    // to avoid watching the user's home directory if Claude Desktop started the server there.
    let is_valid_project = current_dir.join(".git").exists()
        || current_dir.join("Cargo.toml").exists()
        || current_dir.join("Gemfile").exists()
        || current_dir.join("package.json").exists();

    if is_valid_project {
        icnow::watcher::ensure_watching(&current_dir, &db_path);
    } else {
        tracing::info!(
            "Current directory {:?} does not appear to be a project root. Skipping automatic watcher.",
            current_dir
        );
    }

    service_handle.waiting().await?;

    tracing::info!("Server has shut down.");

    Ok(())
}
