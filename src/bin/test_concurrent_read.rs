use lbug::{Connection, Database, SystemConfig};
use std::thread;

fn main() {
    let db = Database::new("test_concurrent_read.db", SystemConfig::default()).unwrap();
    let db_ref: &'static Database = Box::leak(Box::new(db));
    
    let conn1 = Connection::new(db_ref).unwrap();
    conn1.query("CREATE NODE TABLE IF NOT EXISTS T (id INT64, PRIMARY KEY(id))").unwrap();
    
    let t = thread::spawn(move || {
        let conn2 = Connection::new(db_ref).unwrap();
        loop {
            let _ = conn2.query("MATCH (n:T) RETURN n LIMIT 1");
        }
    });
    
    for _ in 0..100 {
        conn1.query("BEGIN TRANSACTION").unwrap();
        conn1.query("MERGE (n:T {id: 1})").unwrap();
        conn1.query("COMMIT").unwrap();
    }
    
    // We don't join t to let it run concurrently
    println!("Done");
}
