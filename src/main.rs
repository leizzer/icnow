use anyhow::Result;
use rmcp::{transport::stdio, ServiceExt};
use icnow::tools::GraphService;
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
    let db_path = current_dir.join("knowledge.db").to_string_lossy().to_string();

    // 1. Run startup scan and reconciliation
    if let Err(e) = icnow::watcher::reconcile_workspace(&current_dir, &db_path) {
        tracing::error!("Workspace reconciliation failed: {}", e);
    }

    // 2. Start live file watcher in background
    let db_path_clone = db_path.clone();
    let current_dir_clone = current_dir.clone();
    tokio::spawn(async move {
        if let Err(e) = icnow::watcher::watch_workspace(current_dir_clone, db_path_clone).await {
            tracing::error!("Watcher error: {}", e);
        }
    });

    let service = GraphService { db_path };
    let service_handle = service.serve(stdio()).await?;

    service_handle.waiting().await?;

    tracing::info!("Server has shut down.");

    Ok(())
}
