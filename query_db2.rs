fn main() {
    let db_path = "/Users/cristian/projects/dgapp_bkp/knowledge.db";
    let conn = icnow::open_db_connection(db_path).unwrap();
    match conn.query("MATCH (n) RETURN COUNT(n)") {
        Ok(mut res) => {
            if let Some(row) = res.next() {
                println!("Nodes: {:?}", row[0]);
            }
        }
        Err(e) => println!("Error: {}", e),
    }
    match conn.query("MATCH ()-[e]->() RETURN COUNT(e)") {
        Ok(mut res) => {
            if let Some(row) = res.next() {
                println!("Edges: {:?}", row[0]);
            }
        }
        Err(e) => println!("Error: {}", e),
    }
}
