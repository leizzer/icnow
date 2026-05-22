use std::path::{Path, PathBuf};
use std::fs;
use std::time::UNIX_EPOCH;
use std::sync::mpsc::channel;
use std::sync::{LazyLock, Mutex};
use std::collections::HashSet;
use anyhow::Result;
use graphqlite::{Connection, Graph};
use notify::{Watcher, RecommendedWatcher, RecursiveMode, Event};

static WATCHED_WORKSPACES: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(|| {
    Mutex::new(HashSet::new())
});

pub fn ensure_watching(project_root: &Path, db_path: &str) {
    let canonical = match fs::canonicalize(project_root) {
        Ok(p) => p,
        Err(_) => project_root.to_path_buf(),
    };
    
    {
        let mut watched = WATCHED_WORKSPACES.lock().unwrap();
        if watched.contains(&canonical) {
            return;
        }
        watched.insert(canonical.clone());
    }
    
    tracing::info!("Registering new workspace to watch: {:?}", canonical);
    let db_path_clone = db_path.to_string();
    let root_clone = canonical.clone();
    
    tokio::spawn(async move {
        let db_path_reconcile = db_path_clone.clone();
        let root_reconcile = root_clone.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(e) = reconcile_workspace(&root_reconcile, &db_path_reconcile) {
                tracing::error!("Workspace reconciliation failed for {:?}: {}", root_reconcile, e);
            }
        });
        
        if let Err(e) = watch_workspace(root_clone.clone(), db_path_clone).await {
            tracing::error!("Watcher error for {:?}: {}", root_clone, e);
        }
    });
}

fn scan_directory(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let path_str = path.to_string_lossy();
                if path_str.contains("/.git") || 
                   path_str.contains("/target") || 
                   path_str.contains("/node_modules") || 
                   path_str.contains("/vendor") {
                    continue;
                }
                scan_directory(&path, files);
            } else if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext == "rs" || ext == "rb" || ext == "ts" || ext == "tsx" {
                        files.push(path);
                    }
                }
            }
        }
    }
}

pub fn reconcile_workspace(root_dir: &Path, db_path: &str) -> Result<()> {
    tracing::info!("Reconciling workspace: {:?}", root_dir);
    let conn = Connection::open(db_path)?;
    
    // 1. Get current DB files using Cypher
    let mut db_files = std::collections::HashMap::new();
    if let Ok(res) = conn.cypher("MATCH (f:File) RETURN f.id, f.last_modified") {
        for row in &res {
            if let (Ok(id), Ok(lm)) = (row.get::<String>("f.id"), row.get::<String>("f.last_modified")) {
                if let Ok(lm_val) = lm.parse::<u64>() {
                    db_files.insert(id, lm_val);
                }
            }
        }
    }
    
    // 2. Scan disk
    let mut disk_files = Vec::new();
    scan_directory(root_dir, &mut disk_files);
    
    let mut files_to_reindex = Vec::new();
    let mut current_disk_paths = std::collections::HashSet::new();
    
    for file_path in disk_files {
        if let Ok(canonical) = fs::canonicalize(&file_path) {
            let path_str = canonical.to_string_lossy().to_string();
            current_disk_paths.insert(path_str.clone());
            
            let mut mtime = 0;
            if let Ok(metadata) = fs::metadata(&canonical) {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                        mtime = duration.as_secs();
                    }
                }
            }
            
            if let Some(&db_mtime) = db_files.get(&path_str) {
                if mtime > db_mtime {
                    tracing::info!("File modified offline: {}", path_str);
                    files_to_reindex.push(path_str);
                }
            } else {
                tracing::info!("New file found offline: {}", path_str);
                files_to_reindex.push(path_str);
            }
        }
    }
    
    // 3. Find deleted files
    let mut files_to_delete = Vec::new();
    for db_path in db_files.keys() {
        if !current_disk_paths.contains(db_path) {
            tracing::info!("File deleted offline: {}", db_path);
            files_to_delete.push(db_path.clone());
        }
    }
    
    // 4. Perform deletions using Cypher
    for path in &files_to_delete {
        let escaped = path.replace("'", "''");
        let q = format!("MATCH (n) WHERE n.id = '{escaped}' OR n.file = '{escaped}' DETACH DELETE n");
        if let Err(e) = conn.cypher(&q) {
            tracing::error!("Failed to delete stale node: {}", e);
        }
    }
    
    // 5. Perform re-indexing
    let graph = Graph::open(db_path)?;
    for path in &files_to_reindex {
        let escaped = path.replace("'", "''");
        let q = format!("MATCH (n) WHERE n.id = '{escaped}' OR n.file = '{escaped}' DETACH DELETE n");
        let _ = conn.cypher(&q);
        
        if let Err(e) = crate::parser::parse_file(path, &graph) {
            tracing::error!("Failed to parse file {}: {}", path, e);
        }
    }
    
    tracing::info!("Workspace reconciliation complete. Deleted: {}, Reindexed: {}", files_to_delete.len(), files_to_reindex.len());
    
    // Trigger cross-file import reconciliation
    let db_path_clone = db_path.to_string();
    std::thread::spawn(move || {
        if let Err(e) = crate::reconciler::reconcile_imports(&db_path_clone) {
            tracing::error!("Import reconciliation failed: {}", e);
        }
    });
    
    Ok(())
}

fn handle_watcher_event(event: Event, conn: &Connection, graph: &Graph) {
    for path in event.paths {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext != "rs" && ext != "rb" && ext != "ts" && ext != "tsx" {
                continue;
            }
        } else {
            continue;
        }
        
        // Skip hidden paths
        if path.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.')) {
            continue;
        }
        // Skip target, vendor, node_modules, etc.
        let path_str_lower = path.to_string_lossy().to_string();
        if path_str_lower.contains("/target/") || path_str_lower.contains("/vendor/") || path_str_lower.contains("/node_modules/") {
            continue;
        }
        
        let abs_path = match fs::canonicalize(&path) {
            Ok(p) => p,
            Err(_) => path,
        };
        let path_str = abs_path.to_string_lossy().to_string();
        
        let exists = abs_path.exists();
        if exists && abs_path.is_file() {
            tracing::info!("Watcher: reindexing {}", path_str);
            let escaped = path_str.replace("'", "''");
            let q = format!("MATCH (n) WHERE n.id = '{escaped}' OR n.file = '{escaped}' DETACH DELETE n");
            let _ = conn.cypher(&q);
            
            if let Err(e) = crate::parser::parse_file(&path_str, graph) {
                tracing::error!("Failed to parse file {}: {}", path_str, e);
            }
        } else if !exists {
            tracing::info!("Watcher: removing deleted file {}", path_str);
            let escaped = path_str.replace("'", "''");
            let q = format!("MATCH (n) WHERE n.id = '{escaped}' OR n.file = '{escaped}' DETACH DELETE n");
            let _ = conn.cypher(&q);
        }
    }
}

pub async fn watch_workspace(root_dir: PathBuf, db_path: String) -> Result<()> {
    tracing::info!("Starting background file watcher for {:?}", root_dir);
    
    tokio::task::spawn_blocking(move || {
        let (tx, rx) = channel();
        let mut watcher = match RecommendedWatcher::new(tx, notify::Config::default()) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("Failed to create watcher: {}", e);
                return;
            }
        };
        if let Err(e) = watcher.watch(&root_dir, RecursiveMode::Recursive) {
            tracing::error!("Failed to start watch: {}", e);
            return;
        }
        
        let graph = match Graph::open(&db_path) {
            Ok(g) => g,
            Err(e) => {
                tracing::error!("Watcher failed to open DB: {}", e);
                return;
            }
        };
        let conn = match Connection::open(&db_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Watcher failed to open connection: {}", e);
                return;
            }
        };
        
        tracing::info!("Live file watcher active.");
        for res in rx {
            match res {
                Ok(event) => {
                    handle_watcher_event(event, &conn, &graph);
                }
                Err(e) => {
                    tracing::error!("Watcher event error: {}", e);
                }
            }
        }
    });
    
    Ok(())
}
