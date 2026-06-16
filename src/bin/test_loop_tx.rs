use lbug::{Connection, Database, SystemConfig};

fn main() {
    let db = Database::new("test_loop.db", SystemConfig::default()).unwrap();
    let db_ref: &'static Database = Box::leak(Box::new(db));
    
    let conn1 = Connection::new(db_ref).unwrap();
    conn1.query("CREATE NODE TABLE IF NOT EXISTS T (id INT64, PRIMARY KEY(id))").unwrap();
    
    conn1.query("BEGIN TRANSACTION").unwrap();
    for i in 0..100000 {
        conn1.query(&format!("MERGE (n:T {{id: {}}})", i)).unwrap();
        if i % 50 == 0 {
            conn1.query("COMMIT").unwrap();
            conn1.query("BEGIN TRANSACTION").unwrap();
        }
    }
    conn1.query("COMMIT").unwrap();
}
