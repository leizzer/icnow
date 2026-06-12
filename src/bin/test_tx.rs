use lbug::{Connection, Database, SystemConfig};

fn main() {
    let db = Database::new("test_tx.db", SystemConfig::default()).unwrap();
    let conn = Connection::new(&db).unwrap();
    conn.query("CREATE NODE TABLE IF NOT EXISTS TestNode (id INT64, PRIMARY KEY(id))").unwrap();
    
    conn.query("BEGIN TRANSACTION").unwrap();
    let res = conn.query("MERGE (n:TestNode {id: 1})");
    println!("MERGE result: {:?}", res.is_ok());
    conn.query("COMMIT").unwrap();
}
