use std::thread;

#[test]
fn test_connection_limit() {
    let path = "/Users/cristian/Projects/dgapp_bkp/app/models/assessment/risk_monitor.rb";
    let db = icnow::database::get_or_init_db("test_conn_limit.db").unwrap();
    
    for i in 0..30 {
        let conn = lbug::Connection::new(db.as_ref()).unwrap();
        let _ = conn.query("MATCH (n) RETURN n LIMIT 1");
        println!("Call {}", i);
    }
}
