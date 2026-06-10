pub mod api_handlers;
pub mod exporter;
pub mod lsif;
pub mod models;
pub mod parser;
pub mod reconciler;
pub mod tools;
pub mod watcher;

use lbug::{Database, Connection, SystemConfig};
use std::sync::OnceLock;

static DB: OnceLock<Database> = OnceLock::new();

pub fn get_or_init_db(path: &str) -> Result<&'static Database, String> {
    if let Some(db) = DB.get() {
        Ok(db)
    } else {
        let db = Database::new(path, SystemConfig::default())
            .map_err(|e| format!("Failed to open DB: {}", e))?;
            
        let _ = DB.set(db); // Ignore if another thread won the race
        
        let saved_db = DB.get().unwrap();
        let conn = Connection::new(saved_db).unwrap();
        init_schema(&conn)?;
        
        Ok(saved_db)
    }
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
        "CREATE NODE TABLE IF NOT EXISTS Symbol (id STRING, name STRING, signature STRING, docstring STRING, kind STRING, source_code STRING, PRIMARY KEY(id))",
        "CREATE NODE TABLE IF NOT EXISTS File (id STRING, name STRING, last_modified INT64, PRIMARY KEY(id))",
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

