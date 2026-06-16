use lbug::{Database, Connection, SystemConfig};

fn main() {
    let db = Database::new("test_tx_db", SystemConfig::default()).unwrap();
    let conn = Connection::new(&db).unwrap();
    let res = conn.query("BEGIN TRANSACTION");
    println!("BEGIN TRANSACTION result: {:?}", res);
}
