use lbug::{Connection, Database, SystemConfig};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
static DBS: OnceLock<Mutex<HashMap<String, std::sync::Arc<Database>>>> = OnceLock::new();
pub fn resolve_centralized_db_path(original_db_path: &str) -> String {
    #[cfg(test)]
    {
        return original_db_path.to_string();
    }
    #[cfg(not(test))]
    {
        let path = std::path::Path::new(original_db_path);
        let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));

        let local_icnow_dir = parent.join(".icnow");
        if local_icnow_dir.is_dir() {
            return local_icnow_dir
                .join(
                    path.file_name()
                        .unwrap_or_else(|| std::ffi::OsStr::new("knowledge.db")),
                )
                .to_string_lossy()
                .to_string();
        }

        let abs_parent = match parent.canonicalize() {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => parent.to_string_lossy().to_string(),
        };

        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        abs_parent.hash(&mut hasher);
        let hash = hasher.finish();
        let hash_str = format!("{hash:016x}");

        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let icnow_dir = std::path::Path::new(&home_dir)
            .join(".icnow")
            .join("projects")
            .join(hash_str);

        let _ = std::fs::create_dir_all(&icnow_dir);

        icnow_dir
            .join(
                path.file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new("knowledge.db")),
            )
            .to_string_lossy()
            .to_string()
    }
}

pub fn get_or_init_db(path: &str) -> Result<std::sync::Arc<Database>, String> {
    tracing::info!("get_or_init_db called with path: {}", path);
    let remapped_path = resolve_centralized_db_path(path);
    let map = DBS.get_or_init(|| Mutex::new(HashMap::new()));

    let cache_key = path.to_string();
    let mut guard = map.lock().map_err(|e| format!("DB Cache Mutex poisoned: {}", e))?;
    if let Some(db) = guard.get(&cache_key) {
        tracing::info!("Found existing DB for path: {}", cache_key);
        return Ok(db.clone());
    }

    let path_str = remapped_path.clone();
    tracing::info!("Initializing new DB for path: {}", path_str);

    let mut is_fresh = false;
    if !std::path::Path::new(&path_str).exists() {
        is_fresh = true;
    }

    let cfg = SystemConfig::default().buffer_pool_size(1024 * 1024 * 1024); // 1GB buffer pool
    let db_result = Database::new(&path_str, cfg.clone());

    let db = match db_result {
        Ok(db) => db,
        Err(e) => {
            let err_msg = e.to_string();
            let err_msg_lower = err_msg.to_lowercase();
            if err_msg.contains("Could not set lock on file")
                || err_msg.contains("IO exception: Could not set lock")
            {
                tracing::warn!(
                    "DB is locked by another process. Falling back to read-only mode with retries..."
                );
                let ro_config = cfg.read_only(true);
                let mut ro_res = Database::new(&path_str, ro_config.clone());
                for _ in 0..3 {
                    if let Err(ro_err) = &ro_res {
                        let ro_err_lower = ro_err.to_string().to_lowercase();
                        if ro_err_lower.contains("corrupt") || ro_err_lower.contains("checksum") {
                            tracing::warn!("Read-only open hit WAL corruption, retrying...");
                            std::thread::sleep(std::time::Duration::from_millis(150));
                            ro_res = Database::new(&path_str, ro_config.clone());
                            continue;
                        }
                    }
                    break;
                }
                ro_res.map_err(|e2| format!("Failed to open DB in read-only mode: {e2}"))?
            } else if err_msg_lower.contains("corrupt") || err_msg_lower.contains("checksum") {
                tracing::warn!(
                    "Corrupted WAL file detected at {}. Wiping and reinitializing...",
                    path_str
                );
                let _ = std::fs::remove_file(&path_str);
                let _ = std::fs::remove_file(format!("{}.wal", path_str));
                let _ = std::fs::remove_file(format!("{}.shm", path_str));
                is_fresh = true;
                Database::new(&path_str, cfg.clone())
                    .map_err(|e2| format!("Failed to open DB after wiping: {e2}"))?
            } else {
                return Err(format!("Failed to open DB: {e}"));
            }
        }
    };
    let arc_db = std::sync::Arc::new(db);
    let conn = Connection::new(arc_db.as_ref()).unwrap();
    tracing::info!("Calling init_schema");
    init_schema(&conn)?;

    if is_fresh {
        tracing::info!("DB is fresh. Attempting to restore memories from backup...");
        crate::api_handlers::memory::restore_all_memories(&conn, &path_str);
    }

    tracing::info!("Inserting DB into map");
    guard.insert(cache_key, arc_db.clone());

    drop(conn);

    tracing::info!("Returning DB");
    Ok(arc_db)
}

// Removed open_db_connection and open_db_graph as they are fundamentally incompatible
// with Arc<Database> lifetimes. Callers must do:
// let db = get_or_init_db(path)?;
// let conn = lbug::Connection::new(db.as_ref()).map_err(...)?;

fn init_schema(conn: &Connection) -> Result<(), String> {
    let node_tables = vec![
        "CREATE NODE TABLE IF NOT EXISTS Symbol (id STRING, name STRING, signature STRING, docstring STRING, kind STRING, start_line INT64, end_line INT64, file STRING, line STRING, PRIMARY KEY(id))",
        "CREATE NODE TABLE IF NOT EXISTS File (id STRING, name STRING, kind STRING, last_modified INT64, PRIMARY KEY(id))",
        "CREATE NODE TABLE IF NOT EXISTS Memory (id STRING, name STRING, description STRING, keywords STRING, embedding FLOAT[384], PRIMARY KEY(id))",
    ];
    for table in node_tables {
        if let Err(e) = conn.query(table) {
            let err_str = e.to_string();
            if !err_str.contains("already exists") {
                tracing::warn!("Failed to init node table: {}", err_str);
            }
        }
    }

    let rel_tables = vec![
        "CREATE REL TABLE IF NOT EXISTS CONTAINS (FROM File TO Symbol, FROM Symbol TO Symbol, FROM File TO File, FROM Memory TO Memory, FROM Memory TO Symbol, FROM Memory TO File, FROM Symbol TO File)",
        "CREATE REL TABLE IF NOT EXISTS CALLS (FROM Symbol TO Symbol, FROM File TO Symbol, FROM Symbol TO File, FROM File TO File)",
        "CREATE REL TABLE IF NOT EXISTS DEFINES (FROM Symbol TO Symbol, FROM File TO Symbol)",
        "CREATE REL TABLE IF NOT EXISTS INHERITS (FROM Symbol TO Symbol)",
        "CREATE REL TABLE IF NOT EXISTS INSTANTIATES (FROM Symbol TO Symbol, FROM File TO Symbol, FROM Symbol TO File, FROM File TO File)",
        "CREATE REL TABLE IF NOT EXISTS REFERENCES (FROM Memory TO Memory, FROM Memory TO Symbol, FROM Memory TO File, FROM Symbol TO Symbol, FROM File TO Symbol, FROM Symbol TO File, FROM File TO File, FROM File TO Memory, FROM Symbol TO Memory)",
        "CREATE REL TABLE IF NOT EXISTS IMPORTS (FROM File TO File, FROM File TO Symbol, FROM Symbol TO File, FROM Symbol TO Symbol)",
        "CREATE REL TABLE IF NOT EXISTS DEPENDS_ON (FROM Symbol TO Symbol, FROM File TO Symbol, FROM Symbol TO File, FROM File TO File)",
    ];
    for table in rel_tables {
        if let Err(e) = conn.query(table) {
            let err_str = e.to_string();
            if !err_str.contains("already exists") {
                tracing::warn!("Failed to init rel table: {}", err_str);
            }
        }
    }

    Ok(())
}
