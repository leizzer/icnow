extern crate openssl;

pub mod api_handlers;
pub mod database;
pub mod exporter;
pub mod indexer;
pub mod installer;
pub mod models;
pub mod prompts;
pub mod resources;
pub mod tools;

pub use database::*;

pub static PAUSE_WATCHER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub static IS_INDEXING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

use fastembed::TextEmbedding;
use std::sync::{Mutex, OnceLock};

pub static EMBEDDING_MODEL: OnceLock<Result<Mutex<TextEmbedding>, String>> = OnceLock::new();

pub fn get_embedding_model() -> Result<&'static Mutex<TextEmbedding>, String> {
    let res = EMBEDDING_MODEL.get_or_init(|| {
        tracing::info!("Initializing fastembed model (downloads if not present)...");
        let mut cache_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        cache_dir.push(".icnow");
        cache_dir.push(".fastembed_cache");

        let options = fastembed::InitOptions::new(fastembed::EmbeddingModel::AllMiniLML6V2)
            .with_show_download_progress(true)
            .with_cache_dir(cache_dir);
        
        TextEmbedding::try_new(options)
            .map(Mutex::new)
            .map_err(|e| format!("Failed to initialize embedding model: {}", e))
    });

    match res {
        Ok(m) => Ok(m),
        Err(e) => Err(e.clone()),
    }
}
