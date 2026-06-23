fn main() {
    let db = kuzu::Database::new("/Users/cristian/Projects/dgapp_bkp/knowledge.db").unwrap();
    let conn = kuzu::Connection::new(&db).unwrap();
    let mut res = conn.query("MATCH (s:Symbol {id: '/Users/cristian/Projects/dgapp_bkp/app/models/user.rb::User'})-[r:REL_CONTAINS|:HAS_METHOD]->(child:Symbol) RETURN type(r), child.id, child.kind LIMIT 10").unwrap();
    while let Some(row) = res.next() {
        println!("{:?}", row);
    }
}
