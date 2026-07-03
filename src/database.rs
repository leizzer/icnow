use lbug::{Database, Connection, SystemConfig};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static DBS: OnceLock<Mutex<HashMap<String, &'static Database>>> = OnceLock::new();

pub fn resolve_centralized_db_path(original_db_path: &str) -> String {
    let path = std::path::Path::new(original_db_path);
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    
    let local_icnow_dir = parent.join(".icnow");
    if local_icnow_dir.is_dir() {
        return local_icnow_dir.join(path.file_name().unwrap_or_else(|| std::ffi::OsStr::new("knowledge.db"))).to_string_lossy().to_string();
    }
    
    let abs_parent = match parent.canonicalize() {
        Ok(p) => p.to_string_lossy().to_string(),
        Err(_) => parent.to_string_lossy().to_string()
    };
    
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in abs_parent.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let hash_str = format!("{:016x}", hash);
    
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let icnow_dir = std::path::Path::new(&home_dir).join(".icnow").join("projects").join(hash_str);
    
    let _ = std::fs::create_dir_all(&icnow_dir);
    
    icnow_dir.join(path.file_name().unwrap_or_else(|| std::ffi::OsStr::new("knowledge.db"))).to_string_lossy().to_string()
}

pub fn get_or_init_db(path: &str) -> Result<&'static Database, String> {
    tracing::info!("get_or_init_db called with path: {}", path);
    let remapped_path = resolve_centralized_db_path(path);
    let map = DBS.get_or_init(|| Mutex::new(HashMap::new()));
    
    let cache_key = path.to_string();
    let mut guard = map.lock().unwrap();
    if let Some(db) = guard.get(&cache_key) {
        tracing::info!("Found existing DB for path: {}", cache_key);
        return Ok(*db);
    }
    
    let path_str = remapped_path.clone();
    tracing::info!("Initializing new DB for path: {}", path_str);
    
    let mut is_fresh = false;
    if !std::path::Path::new(&path_str).exists() {
        is_fresh = true;
    }

    let db_result = Database::new(&path_str, SystemConfig::default());
    
    let db = match db_result {
        Ok(db) => db,
        Err(e) => {
            let err_msg = e.to_string();
            if err_msg.contains("Could not set lock on file") || err_msg.contains("IO exception: Could not set lock") {
                tracing::warn!("DB is locked by another process. Falling back to read-only mode with retries...");
                let ro_config = SystemConfig::default().read_only(true);
                let mut ro_res = Database::new(&path_str, ro_config.clone());
                for _ in 0..3 {
                    if let Err(ro_err) = &ro_res {
                        if ro_err.to_string().contains("Corrupted wal file") {
                            tracing::warn!("Read-only open hit WAL corruption, retrying...");
                            std::thread::sleep(std::time::Duration::from_millis(150));
                            ro_res = Database::new(&path_str, ro_config.clone());
                            continue;
                        }
                    }
                    break;
                }
                ro_res.map_err(|e2| format!("Failed to open DB in read-only mode: {}", e2))?
            } else if err_msg.contains("Corrupted wal file") {
                tracing::warn!("Corrupted WAL file detected at {}. Wiping and reinitializing...", path_str);
                let _ = std::fs::remove_file(&path_str);
                let wal_path = format!("{}.wal", path_str);
                let _ = std::fs::remove_file(&wal_path);
                is_fresh = true;
                Database::new(&path_str, SystemConfig::default())
                    .map_err(|e2| format!("Failed to open DB after wiping: {}", e2))?
            } else {
                return Err(format!("Failed to open DB: {}", e));
            }
        }
    };
    let leaked_db = Box::leak(Box::new(db));
    let conn = Connection::new(leaked_db).unwrap();
    tracing::info!("Calling init_schema");
    init_schema(&conn)?;
    
    if is_fresh {
        tracing::info!("DB is fresh. Attempting to restore memories from backup...");
        crate::api_handlers::memory::restore_all_memories(&conn, &path_str);
    }
    
    tracing::info!("Inserting DB into map");
    guard.insert(cache_key, leaked_db);
    
    tracing::info!("Returning DB");
    Ok(leaked_db)
}

pub fn open_db_connection(path: &str) -> Result<Connection<'static>, String> {
    let db = get_or_init_db(path)?;
    Connection::new(db).map_err(|e| format!("Failed to create connection: {}", e))
}

pub fn open_db_graph(path: &str) -> Result<Connection<'static>, String> {
    open_db_connection(path) // Aliased for backwards compatibility in unrefactored code
}

fn init_schema(conn: &Connection) -> Result<(), String> {
    let node_tables = vec![
        "CREATE NODE TABLE IF NOT EXISTS Symbol (id STRING, name STRING, signature STRING, docstring STRING, kind STRING, start_line INT64, end_line INT64, file STRING, line STRING, PRIMARY KEY(id))",
        "CREATE NODE TABLE IF NOT EXISTS File (id STRING, name STRING, kind STRING, last_modified INT64, PRIMARY KEY(id))",
        "CREATE NODE TABLE IF NOT EXISTS Memory (id STRING, name STRING, description STRING, keywords STRING, embedding FLOAT[384], PRIMARY KEY(id))",
    ];
    for table in node_tables {
        let _ = conn.query(table); // Ignore errors as they might just mean table already exists
    }

    let rel_tables = vec![
        "CREATE REL TABLE IF NOT EXISTS CONTAINS (FROM File TO Symbol, FROM Symbol TO Symbol, FROM File TO File, FROM Memory TO Memory, FROM Memory TO Symbol, FROM Memory TO File, FROM Symbol TO File)",
        "CREATE REL TABLE IF NOT EXISTS CALLS (FROM Symbol TO Symbol, FROM File TO Symbol, FROM Symbol TO File, FROM File TO File)",
        "CREATE REL TABLE IF NOT EXISTS DEFINES (FROM Symbol TO Symbol, FROM File TO Symbol)",
        "CREATE REL TABLE IF NOT EXISTS INHERITS (FROM Symbol TO Symbol)",
        "CREATE REL TABLE IF NOT EXISTS INSTANTIATES (FROM Symbol TO Symbol, FROM File TO Symbol, FROM Symbol TO File, FROM File TO File)",
        "CREATE REL TABLE IF NOT EXISTS REFERENCES (FROM Memory TO Memory, FROM Memory TO Symbol, FROM Memory TO File, FROM Symbol TO Symbol, FROM File TO Symbol, FROM Symbol TO File, FROM File TO File, FROM File TO Memory, FROM Symbol TO Memory)",
        "CREATE REL TABLE IF NOT EXISTS IMPORTS (FROM File TO File, FROM File TO Symbol, FROM Symbol TO File, FROM Symbol TO Symbol)",
    ];
    for table in rel_tables {
        let _ = conn.query(table); // Ignore errors
    }
    
    Ok(())
}
