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
    
    // Parse db_path from command-line args if provided, otherwise default to Cwd/knowledge.db
    let args: Vec<String> = std::env::args().collect();
    let db_path = if args.len() > 1 {
        args[1].clone()
    } else {
        current_dir.join("knowledge.db").to_string_lossy().to_string()
    };

    let service = GraphService { db_path: db_path.clone() };
    let service_handle = service.serve(stdio()).await?;

    // Run reconciliation and watch tasks in the background to avoid blocking the MCP startup handshake
    let db_path_clone = db_path.clone();
    let current_dir_clone = current_dir.clone();
    tokio::spawn(async move {
        // Run reconciliation in blocking threadpool
        let db_path_reconcile = db_path_clone.clone();
        let current_dir_reconcile = current_dir_clone.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(e) = icnow::watcher::reconcile_workspace(&current_dir_reconcile, &db_path_reconcile) {
                tracing::error!("Workspace reconciliation failed: {}", e);
            }
        });

        // Run live file watcher
        if let Err(e) = icnow::watcher::watch_workspace(current_dir_clone, db_path_clone).await {
            tracing::error!("Watcher error: {}", e);
        }
    });

    service_handle.waiting().await?;

    tracing::info!("Server has shut down.");

    Ok(())
}
