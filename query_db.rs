fn main() {
    let db_path = "/Users/cristian/projects/dgapp_bkp/knowledge.db";
    let conn = icnow::open_db_connection(db_path).unwrap();
    match conn.query("MATCH (n) RETURN count(n) LIMIT 1") {
        Ok(mut res) => {
            if let Some(row) = res.next() {
                println!("Count: {:?}", row[0]);
            }
        }
        Err(e) => println!("Error: {}", e),
    }
}
