use lbug::{Connection, Database, SystemConfig};

fn main() {
    let db = Database::new("test_syntax.db", SystemConfig::default()).unwrap();
    let conn = Connection::new(&db).unwrap();
    let res = conn.query("MERGE (n:TestNode {id: 'unclosed string})");
    println!("Res: {:?}", res.is_ok());
}
