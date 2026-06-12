use lbug::{Connection, Database, SystemConfig};

fn main() {
    let db = Database::new("test_tx.db", SystemConfig::default()).unwrap();
    let conn = Connection::new(&db).unwrap();
    conn.query("CREATE NODE TABLE IF NOT EXISTS TestNode (id INT64, PRIMARY KEY(id))").unwrap();
    
    match conn.query("BEGIN TRANSACTION") {
        Ok(_) => println!("BEGIN TRANSACTION worked"),
        Err(e) => println!("Error BEGIN TRANSACTION: {}", e),
    }
    
    match conn.query("COMMIT") {
        Ok(_) => println!("COMMIT worked"),
        Err(e) => println!("Error COMMIT: {}", e),
    }

    match conn.query("BEGIN WRITE TRANSACTION") {
        Ok(_) => println!("BEGIN WRITE TRANSACTION worked"),
        Err(e) => println!("Error BEGIN WRITE TRANSACTION: {}", e),
    }
}
