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
    
    tokio::task::spawn_blocking(move || {
        run_watcher_lifecycle(root_clone, db_path_clone);
    });
}

fn scan_directory(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name == ".git" || name == "target" || name == "node_modules" || name == "vendor" {
                        continue;
                    }
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
    let conn = crate::open_db_connection(db_path)?;
    
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
    let graph = crate::open_db_graph(db_path)?;
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
        let has_ignored_dir = path.components().any(|comp| {
            if let Some(s) = comp.as_os_str().to_str() {
                let s_lower = s.to_lowercase();
                s_lower == "target" || s_lower == "vendor" || s_lower == "node_modules" || s_lower == ".git"
            } else {
                false
            }
        });
        if has_ignored_dir {
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

fn is_process_alive(pid: u32) -> bool {
    if cfg!(windows) {
        if let Ok(output) = std::process::Command::new("tasklist")
            .args(&["/FI", &format!("PID eq {}", pid)])
            .output() 
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.contains(&pid.to_string())
        } else {
            false
        }
    } else {
        std::process::Command::new("kill")
            .args(&["-0", &pid.to_string()])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

fn run_watcher_loop(root_dir: PathBuf, db_path: String) -> Result<()> {
    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;
    watcher.watch(&root_dir, RecursiveMode::Recursive)?;
    
    let graph = crate::open_db_graph(&db_path)?;
    let conn = crate::open_db_connection(&db_path)?;
    
    tracing::info!("Live file watcher active for PID {}.", std::process::id());
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
    Ok(())
}

pub fn run_watcher_lifecycle(root_dir: PathBuf, db_path: String) {
    let pid = std::process::id();
    let mut is_active_watcher = false;
    
    loop {
        let conn = match crate::open_db_connection(&db_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Lock manager failed to open DB: {}", e);
                std::thread::sleep(std::time::Duration::from_secs(5));
                continue;
            }
        };
        
        let _ = conn.execute("CREATE TABLE IF NOT EXISTS icnow_watcher_lock (id INTEGER PRIMARY KEY, pid INTEGER, last_heartbeat INTEGER);");
        
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        
        let mut current_lock: Option<(u32, u64)> = None;
        if let Ok(res) = conn.sqlite_connection().query_row(
            "SELECT pid, last_heartbeat FROM icnow_watcher_lock WHERE id = 1",
            [],
            |row| {
                let lock_pid: i32 = row.get(0)?;
                let heartbeat: i64 = row.get(1)?;
                Ok((lock_pid as u32, heartbeat as u64))
            }
        ) {
            current_lock = Some(res);
        }
        
        let can_acquire = match current_lock {
            None => true,
            Some((lock_pid, heartbeat)) => {
                if lock_pid == pid {
                    true
                } else {
                    let is_stale = now - heartbeat >= 15;
                    let is_dead = !is_process_alive(lock_pid);
                    is_stale || is_dead
                }
            }
        };
        
        if can_acquire {
            let res = conn.execute(&format!(
                "INSERT OR REPLACE INTO icnow_watcher_lock (id, pid, last_heartbeat) VALUES (1, {}, {})",
                pid, now
            ));
            
            if res.is_ok() {
                if !is_active_watcher {
                    tracing::info!("Acquired watcher lock for PID {}. Activating file watcher.", pid);
                    is_active_watcher = true;
                    
                    if let Err(e) = reconcile_workspace(&root_dir, &db_path) {
                        tracing::error!("Workspace reconciliation failed: {}", e);
                    }
                    
                    let root_dir_clone = root_dir.clone();
                    let db_path_clone = db_path.clone();
                    std::thread::spawn(move || {
                        if let Err(e) = run_watcher_loop(root_dir_clone, db_path_clone) {
                            tracing::error!("Watcher loop failed: {}", e);
                        }
                    });
                } else {
                    let _ = conn.execute(&format!(
                        "UPDATE icnow_watcher_lock SET last_heartbeat = {} WHERE pid = {}",
                        now, pid
                    ));
                }
            } else {
                if is_active_watcher {
                    tracing::warn!("Failed to update heartbeat. Releasing lock status.");
                    is_active_watcher = false;
                }
            }
        } else {
            if is_active_watcher {
                tracing::warn!("Lock stolen by another process. Stepping down as active watcher.");
                is_active_watcher = false;
            }
        }
        
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}
