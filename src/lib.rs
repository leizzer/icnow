pub mod models;
pub mod parser;
pub mod tools;
pub mod watcher;
pub mod reconciler;
pub mod exporter;
pub mod lsif;

pub fn open_db_connection(path: &str) -> Result<graphqlite::Connection, graphqlite::Error> {
    let conn = graphqlite::Connection::open(path)?;
    let _ = conn.execute("PRAGMA journal_mode=WAL;");
    let _ = conn.execute("PRAGMA busy_timeout=5000;");
    Ok(conn)
}

pub fn open_db_graph(path: &str) -> Result<graphqlite::Graph, graphqlite::Error> {
    let conn = open_db_connection(path)?;
    Ok(graphqlite::Graph::from_connection(conn))
}
