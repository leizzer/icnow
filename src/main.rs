use anyhow::Result;
use rmcp::{transport::stdio, ServiceExt};
use tracing_subscriber::{self, EnvFilter};

pub mod models;
mod tools;
use tools::GraphService;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting icnow Graph MCP server...");

    // For now, use an in-memory db or a local file.
    let service = GraphService { db_path: "knowledge.db".to_string() };
    let service_handle = service.serve(stdio()).await?;

    service_handle.waiting().await?;

    tracing::info!("Server has shut down.");

    Ok(())
}
