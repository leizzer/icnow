use lbug::{Connection, Database, SystemConfig};
use std::thread;

fn main() {
    let db = Database::new("test_concurrent.db", SystemConfig::default()).unwrap();
    let db_ref: &'static Database = Box::leak(Box::new(db));
    
    let conn1 = Connection::new(db_ref).unwrap();
    conn1.query("CREATE NODE TABLE IF NOT EXISTS T (id INT64, PRIMARY KEY(id))").unwrap();
    
    let t = thread::spawn(move || {
        let conn2 = Connection::new(db_ref).unwrap();
        conn2.query("MERGE (n:T {id: 2})").unwrap();
    });
    
    conn1.query("BEGIN TRANSACTION").unwrap();
    conn1.query("MERGE (n:T {id: 1})").unwrap();
    
    t.join().unwrap();
    conn1.query("COMMIT").unwrap();
}
