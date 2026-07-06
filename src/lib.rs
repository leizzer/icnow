pub mod api_handlers;
pub mod installer;
pub mod exporter;
pub mod indexer;
pub mod models;
pub mod resources;
pub mod prompts;
pub mod tools;
pub mod database;

pub use database::*;

pub static PAUSE_WATCHER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub static IS_INDEXING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

use std::sync::{Mutex, OnceLock};
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};

pub static EMBEDDING_MODEL: OnceLock<Mutex<TextEmbedding>> = OnceLock::new();

pub fn get_embedding_model() -> &'static Mutex<TextEmbedding> {
    EMBEDDING_MODEL.get_or_init(|| {
        tracing::info!("Initializing fastembed model (downloads if not present)...");
        let options = fastembed::InitOptions::new(fastembed::EmbeddingModel::AllMiniLML6V2)
            .with_show_download_progress(true);
        Mutex::new(TextEmbedding::try_new(options)
        .expect("Failed to initialize embedding model"))
    })
}
