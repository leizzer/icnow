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

pub static EMBEDDING_MODEL: OnceLock<Mutex<TextEmbedding>> = OnceLock::new();

pub fn get_embedding_model() -> &'static Mutex<TextEmbedding> {
    EMBEDDING_MODEL.get_or_init(|| {
        tracing::info!("Initializing fastembed model (downloads if not present)...");
        let mut cache_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        cache_dir.push(".icnow");
        cache_dir.push(".fastembed_cache");

        let options = fastembed::InitOptions::new(fastembed::EmbeddingModel::AllMiniLML6V2)
            .with_show_download_progress(true)
            .with_cache_dir(cache_dir);
        Mutex::new(TextEmbedding::try_new(options).expect("Failed to initialize embedding model"))
    })
}
