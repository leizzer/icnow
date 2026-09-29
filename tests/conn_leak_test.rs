#[test]
fn test_connection_leak() {
    let db = icnow::database::get_or_init_db("test_leak.db").unwrap();
    
    for i in 0..1000 {
        let conn = lbug::Connection::new(db.as_ref()).unwrap();
        let _ = conn.query("MATCH (n) RETURN n LIMIT 1");
        if i % 100 == 0 {
            println!("Call {}", i);
        }
    }
    println!("Success");
}
