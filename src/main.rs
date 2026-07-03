use anyhow::Result;
use icnow::tools::GraphService;
use icnow::resources::ResourceHandler;
use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber::{self, EnvFilter};

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting icnow Graph MCP server...");
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

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

    icnow::watcher::ensure_watching(&current_dir, &db_path);

    service_handle.waiting().await?;

    tracing::info!("Server has shut down.");

    Ok(())
}
