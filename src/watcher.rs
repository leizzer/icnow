use anyhow::Result;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::{LazyLock, Mutex};
use std::time::UNIX_EPOCH;
use lbug::{Connection, Value};

static WATCHED_WORKSPACES: LazyLock<Mutex<HashSet<PathBuf>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

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
                    let name_lower = name.to_lowercase();
                    if matches!(name_lower.as_str(), ".git" | ".bundle" | ".yarn" | "target" | "node" | "node_modules" | "vendor" | "tmp" | "log" | "coverage" | "public" | "dist" | "build") {
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

fn delete_file_nodes(conn: &Connection, file_path: &str) -> Result<()> {
    let path_esc = file_path.replace("'", "''");
    let query_sym = format!("MATCH (f:File {{id: '{}'}})-[:REL_CONTAINS]->(s:Symbol) DETACH DELETE s", path_esc);
    let query_file = format!("MATCH (f:File {{id: '{}'}}) DETACH DELETE f", path_esc);
    let _ = conn.query(&query_sym);
    let _ = conn.query(&query_file);
    Ok(())
}

fn get_val_str(row: &[Value], cols: &[String], name: &str) -> Option<String> {
    cols.iter().position(|c| c == name).and_then(|idx| {
        if let Value::String(s) = &row[idx] { Some(s.clone()) } else { None }
    })
}

fn get_val_int(row: &[Value], cols: &[String], name: &str) -> Option<i64> {
    cols.iter().position(|c| c == name).and_then(|idx| {
        match &row[idx] {
            Value::Int64(i) => Some(*i),
            Value::Int32(i) => Some(*i as i64),
            Value::String(s) => s.parse::<i64>().ok(),
            _ => None,
        }
    })
}

pub fn reconcile_workspace(root_dir: &Path, db_path: &str) -> Result<()> {
    tracing::info!("Reconciling workspace: {:?}", root_dir);
    let conn = crate::open_db_connection(db_path).map_err(|e| anyhow::anyhow!(e))?;

    let mut db_files = HashMap::new();
    if let Ok(mut res) = conn.query("MATCH (f:File) RETURN f.id, f.last_modified") {
        let cols = res.get_column_names();
        for row in res.by_ref() {
            if let (Some(id), Some(lm)) = (get_val_str(&row, &cols, "f.id"), get_val_int(&row, &cols, "f.last_modified")) {
                db_files.insert(id, lm as u64);
            }
        }
    }

    let mut disk_files = Vec::new();
    scan_directory(root_dir, &mut disk_files);

    let mut files_to_reindex = Vec::new();
    let mut current_disk_paths = HashSet::new();

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

    let mut files_to_delete = Vec::new();
    for db_path in db_files.keys() {
        if !current_disk_paths.contains(db_path) {
            tracing::info!("File deleted offline: {}", db_path);
            files_to_delete.push(db_path.clone());
        }
    }

    for chunk in files_to_delete.chunks(10) {
        if crate::PAUSE_WATCHER.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        let _ = conn.query("BEGIN TRANSACTION");
        for path in chunk {
            let _ = delete_file_nodes(&conn, path);
        }
        let _ = conn.query("COMMIT");
    }

    for chunk in files_to_reindex.chunks(10) {
        if crate::PAUSE_WATCHER.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        let _ = conn.query("BEGIN TRANSACTION");
        for path in chunk {
            let _ = delete_file_nodes(&conn, path);
            if let Err(e) = crate::parser::parse_file(path, &conn) {
                tracing::error!("Failed to parse file {}: {}", path, e);
            }
        }
        let _ = conn.query("COMMIT");
    }

    tracing::info!(
        "Workspace reconciliation complete. Deleted: {}, Reindexed: {}",
        files_to_delete.len(),
        files_to_reindex.len()
    );

    let db_path_clone = db_path.to_string();
    std::thread::spawn(move || {
        if let Err(e) = crate::reconciler::reconcile_imports(&db_path_clone) {
            tracing::error!("Import reconciliation failed: {}", e);
        }
    });

    Ok(())
}

fn handle_watcher_event(event: Event, conn: &Connection) {
    if crate::PAUSE_WATCHER.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    for path in event.paths {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext != "rs" && ext != "rb" && ext != "ts" && ext != "tsx" {
                continue;
            }
        } else {
            continue;
        }

        if path.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.')) {
            continue;
        }

        let has_ignored_dir = path.components().any(|comp| {
            if let Some(s) = comp.as_os_str().to_str() {
                let s_lower = s.to_lowercase();
                matches!(s_lower.as_str(), "target" | "vendor" | "node" | "node_modules" | ".git" | ".bundle" | ".yarn" | "tmp" | "log" | "coverage" | "public" | "dist" | "build")
            } else {
                false
            }
        });
        if has_ignored_dir {
            continue;
        }

        let abs_path = fs::canonicalize(&path).unwrap_or(path);
        let path_str = abs_path.to_string_lossy().to_string();
        let exists = abs_path.exists();
        
        if exists && abs_path.is_file() {
            tracing::info!("Watcher: reindexing {}", path_str);
            let _ = conn.query("BEGIN TRANSACTION");
            let _ = delete_file_nodes(conn, &path_str);
            if let Err(e) = crate::parser::parse_file(&path_str, conn) {
                tracing::error!("Failed to parse file {}: {}", path_str, e);
            }
            let _ = conn.query("COMMIT");
        } else if !exists {
            tracing::info!("Watcher: removing deleted file {}", path_str);
            let _ = conn.query("BEGIN TRANSACTION");
            let _ = delete_file_nodes(conn, &path_str);
            let _ = conn.query("COMMIT");
        }
    }
}

#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    unsafe {
        let res = libc::kill(pid as libc::pid_t, 0);
        res == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}

#[cfg(not(unix))]
fn is_process_alive(pid: u32) -> bool {
    if cfg!(windows) {
        if let Ok(output) = std::process::Command::new("tasklist").args(&["/FI", &format!("PID eq {}", pid)]).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.contains(&pid.to_string())
        } else { false }
    } else { false }
}

pub fn run_watcher_lifecycle(root_dir: PathBuf, db_path: String) {
    let pid = std::process::id();
    let mut is_active_watcher = false;
    #[allow(unused_assignments)]
    let mut _active_watcher: Option<RecommendedWatcher> = None;

    loop {
        if crate::PAUSE_WATCHER.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_secs(5));
            continue;
        }

        let conn = match crate::open_db_connection(&db_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Lock manager failed to open DB: {}", e);
                std::thread::sleep(std::time::Duration::from_secs(5));
                continue;
            }
        };

        let _ = conn.query("CREATE NODE TABLE IF NOT EXISTS WatcherLock (id INT64, pid INT64, last_heartbeat INT64, PRIMARY KEY(id))");

        let now = std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;

        let mut current_lock: Option<(u32, i64)> = None;
        if let Ok(mut res) = conn.query("MATCH (l:WatcherLock {id: 1}) RETURN l.pid, l.last_heartbeat") {
            let cols = res.get_column_names();
            if let Some(row) = res.by_ref().next() {
                if let (Some(l_pid), Some(l_hb)) = (get_val_int(&row, &cols, "l.pid"), get_val_int(&row, &cols, "l.last_heartbeat")) {
                    current_lock = Some((l_pid as u32, l_hb));
                }
            }
        }

        let can_acquire = match current_lock {
            None => true,
            Some((lock_pid, heartbeat)) => {
                if lock_pid == pid { true } else {
                    let is_stale = now - heartbeat >= 15;
                    let is_dead = !is_process_alive(lock_pid);
                    is_stale || is_dead
                }
            }
        };

        if can_acquire {
            let query = format!("MERGE (l:WatcherLock {{id: 1}}) ON MATCH SET l.pid = {}, l.last_heartbeat = {} ON CREATE SET l.pid = {}, l.last_heartbeat = {}", pid, now, pid, now);
            let res = conn.query(&query);

            if res.is_ok() {
                if !is_active_watcher {
                    tracing::info!("Acquired watcher lock for PID {}. Activating file watcher.", pid);
                    is_active_watcher = true;

                    if let Err(e) = reconcile_workspace(&root_dir, &db_path) {
                        tracing::error!("Workspace reconciliation failed: {}", e);
                    }

                    let (tx, rx) = channel();
                    let mut watcher = match RecommendedWatcher::new(tx, notify::Config::default()) {
                        Ok(w) => w,
                        Err(e) => {
                            tracing::error!("Failed to create watcher: {}", e);
                            is_active_watcher = false;
                            std::thread::sleep(std::time::Duration::from_secs(5));
                            continue;
                        }
                    };
                    if let Err(e) = watcher.watch(&root_dir, RecursiveMode::Recursive) {
                        tracing::error!("Failed to watch root dir: {}", e);
                        is_active_watcher = false;
                        std::thread::sleep(std::time::Duration::from_secs(5));
                        continue;
                    }
                    _active_watcher = Some(watcher);

                    let db_path_clone = db_path.clone();
                    std::thread::spawn(move || {
                        let conn = match crate::open_db_connection(&db_path_clone) {
                            Ok(c) => c,
                            Err(e) => {
                                tracing::error!("Watcher thread failed to open connection: {}", e);
                                return;
                            }
                        };
                        tracing::info!("Live file watcher active for PID {}.", std::process::id());
                        loop {
                            if let Ok(res) = rx.recv_timeout(std::time::Duration::from_secs(5)) {
                                match res {
                                    Ok(event) => {
                                        if !crate::PAUSE_WATCHER.load(std::sync::atomic::Ordering::SeqCst) {
                                            handle_watcher_event(event, &conn);
                                        }
                                    }
                                    Err(e) => tracing::error!("watch error: {:?}", e),
                                }
                            }
                        }
                        tracing::info!("Watcher loop thread exiting cleanly.");
                    });
                }
            } else if !is_active_watcher {
                tracing::warn!("Failed to initially acquire watcher lock in DB: {:?}", res.err());
            } else {
                tracing::warn!("Failed to update watcher heartbeat in DB (transient lock?): {:?}", res.err());
            }
        } else if is_active_watcher {
            tracing::warn!("Lock stolen by another process. Stepping down as active watcher.");
            is_active_watcher = false;
            _active_watcher = None; 
        }

        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}
