pub mod api_handlers;
pub mod exporter;
pub mod lsif;
pub mod models;
pub mod parser;
pub mod reconciler;
pub mod tools;
pub mod watcher;

use lbug::{Database, Connection, SystemConfig};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static DBS: OnceLock<Mutex<HashMap<String, &'static Database>>> = OnceLock::new();

pub fn get_or_init_db(path: &str) -> Result<&'static Database, String> {
    tracing::info!("get_or_init_db called with path: {}", path);
    let map = DBS.get_or_init(|| Mutex::new(HashMap::new()));
    
    let path_str = path.to_string();
    let mut guard = map.lock().unwrap();
    if let Some(db) = guard.get(&path_str) {
        tracing::info!("Found existing DB for path: {}", path_str);
        return Ok(*db);
    }
    
    tracing::info!("Initializing new DB for path: {}", path_str);
    let db = Database::new(path, SystemConfig::default())
        .map_err(|e| format!("Failed to open DB: {}", e))?;
        
    tracing::info!("Leaking DB and opening connection");
    let leaked_db = Box::leak(Box::new(db));
    let conn = Connection::new(leaked_db).unwrap();
    tracing::info!("Calling init_schema");
    init_schema(&conn)?;
    
    tracing::info!("Inserting DB into map");
    guard.insert(path_str, leaked_db);
    
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
        "CREATE NODE TABLE IF NOT EXISTS Symbol (id STRING, name STRING, signature STRING, docstring STRING, kind STRING, source_code STRING, file STRING, line STRING, PRIMARY KEY(id))",
        "CREATE NODE TABLE IF NOT EXISTS File (id STRING, name STRING, kind STRING, last_modified INT64, PRIMARY KEY(id))",
        "CREATE NODE TABLE IF NOT EXISTS Memory (id STRING, name STRING, description STRING, keywords STRING, PRIMARY KEY(id))",
    ];
    for table in node_tables {
        let _ = conn.query(table); // Ignore errors as they might just mean table already exists
    }

    let rel_tables = vec![
        "CREATE REL TABLE IF NOT EXISTS REL_CONTAINS (FROM File TO Symbol, FROM Symbol TO Symbol)",
        "CREATE REL TABLE IF NOT EXISTS CALLS (FROM Symbol TO Symbol)",
        "CREATE REL TABLE IF NOT EXISTS HAS_METHOD (FROM Symbol TO Symbol)",
        "CREATE REL TABLE IF NOT EXISTS LINKS_TO (FROM Memory TO Memory, FROM Memory TO Symbol, FROM Memory TO File)",
        "CREATE REL TABLE IF NOT EXISTS IMPORTS (FROM File TO File)",
    ];
    for table in rel_tables {
        let _ = conn.query(table); // Ignore errors
    }
    
    Ok(())
}

