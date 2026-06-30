fn main() {
    let db = lbug::Database::new("test_db_schema", lbug::SystemConfig::default()).unwrap();
    let conn = lbug::Connection::new(&db).unwrap();
    
    let node_tables = vec![
        "CREATE NODE TABLE IF NOT EXISTS Symbol (id STRING, name STRING, signature STRING, docstring STRING, kind STRING, source_code STRING, file STRING, line STRING, PRIMARY KEY(id))",
        "CREATE NODE TABLE IF NOT EXISTS File (id STRING, name STRING, kind STRING, last_modified INT64, PRIMARY KEY(id))",
        "CREATE NODE TABLE IF NOT EXISTS Memory (id STRING, name STRING, description STRING, keywords STRING, PRIMARY KEY(id))",
    ];
    for table in node_tables {
        match conn.query(table) {
            Ok(_) => println!("Success: {}", table),
            Err(e) => println!("Error: {} -> {}", table, e),
        }
    }

    let rel_tables = vec![
        "CREATE REL TABLE IF NOT EXISTS CONTAINS (FROM File TO Symbol, FROM Symbol TO Symbol, FROM File TO File, FROM Memory TO Memory, FROM Memory TO Symbol, FROM Memory TO File, FROM Symbol TO File)",
        "CREATE REL TABLE IF NOT EXISTS CALLS (FROM Symbol TO Symbol, FROM File TO Symbol, FROM Symbol TO File, FROM File TO File)",
        "CREATE REL TABLE IF NOT EXISTS DEFINES (FROM Symbol TO Symbol, FROM File TO Symbol)",
        "CREATE REL TABLE IF NOT EXISTS REFERENCES (FROM Memory TO Memory, FROM Memory TO Symbol, FROM Memory TO File, FROM Symbol TO Symbol, FROM File TO Symbol, FROM Symbol TO File, FROM File TO File, FROM File TO Memory, FROM Symbol TO Memory)",
        "CREATE REL TABLE IF NOT EXISTS IMPORTS (FROM File TO File, FROM File TO Symbol, FROM Symbol TO File, FROM Symbol TO Symbol)",
    ];
    for table in rel_tables {
        match conn.query(table) {
            Ok(_) => println!("Success: {}", table),
            Err(e) => println!("Error: {} -> {}", table, e),
        }
    }
}
