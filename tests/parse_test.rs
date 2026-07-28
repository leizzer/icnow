#[test]
fn test_parse() {
    let path = "/Users/cristian/Projects/dgapp_bkp/app/models/assessment/risk_monitor.rb";
    let db = icnow::database::get_or_init_db("test.db").unwrap();
    let conn = lbug::Connection::new(db.as_ref()).unwrap();
    let _ = conn.query("BEGIN TRANSACTION");
    let res = icnow::indexer::parser::parse_file(path, &conn);
    println!("Result: {:?}", res);
    let _ = conn.query("ROLLBACK");
}
