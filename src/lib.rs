pub mod api_handlers;
pub mod exporter;
pub mod lsif;
pub mod models;
pub mod parser;
pub mod reconciler;
pub mod tools;
pub mod watcher;

pub fn open_db_connection(path: &str) -> Result<graphqlite::Connection, graphqlite::Error> {
    let conn = graphqlite::Connection::open(path)?;
    let _ = conn.execute("PRAGMA journal_mode=WAL;");
    let _ = conn.execute("PRAGMA busy_timeout=5000;");
    let _ = conn.execute("CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(id UNINDEXED, name, description, keywords);");
    // High-performance index for Cypher property lookups (e.g. MATCH (n {id: '...'}))
    let _ = conn.execute("CREATE INDEX IF NOT EXISTS _gql_node_props_text_val_idx ON node_props_text(key_id, value);");
    Ok(conn)
}

pub fn open_db_graph(path: &str) -> Result<graphqlite::Graph, graphqlite::Error> {
    let conn = open_db_connection(path)?;
    Ok(graphqlite::Graph::from_connection(conn))
}
