use anyhow::Result;
use icnow::resources::ResourceHandler;
use icnow::tools::GraphService;
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

fn resolve_db_path(current_dir: &std::path::Path) -> String {
    // Parse db_path from command-line args if provided, otherwise default to Cwd/knowledge.db
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        args[1].clone()
    } else {
        current_dir
            .join("knowledge.db")
            .to_string_lossy()
            .to_string()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "install-skill" {
        let target = &args[2];
        if let Err(e) = icnow::installer::run_installer(target) {
            eprintln!("Installation failed: {e:?}");
            std::process::exit(1);
        }
        return Ok(());
    } else if args.len() >= 2 && args[1] == "install-skill" {
        eprintln!("Usage: icnow install-skill <antigravity|claude|cursor|openai>");
        std::process::exit(1);
    } else if args.len() >= 2 && args[1] == "uninstall" {
        if let Err(e) = icnow::installer::run_uninstall() {
            eprintln!("Uninstall failed: {e:?}");
            std::process::exit(1);
        }
        return Ok(());
    }

    init_tracing();

    let current_dir = std::env::current_dir()?;
    let db_path = resolve_db_path(&current_dir);

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
