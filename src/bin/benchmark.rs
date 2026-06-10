use std::time::{Duration, Instant};

fn run_benchmark<F>(name: &str, mut func: F, iters: u32) -> Duration
where
    F: FnMut(),
{
    // Warmup
    func();

    let start = Instant::now();
    for _ in 0..iters {
        func();
    }
    let duration = start.elapsed();
    println!("{:25} : {:?}", name, duration);
    duration
}

fn compare(name: &str, cypher_dur: Duration, sql_dur: Duration) {
    if sql_dur < cypher_dur {
        let speedup = cypher_dur.as_secs_f64() / sql_dur.as_secs_f64();
        println!("--> SQL is {:.2}x faster for {}", speedup, name);
    } else {
        let speedup = sql_dur.as_secs_f64() / cypher_dur.as_secs_f64();
        println!("--> Cypher is {:.2}x faster for {}", speedup, name);
    }
    println!();
}

fn main() {
    let db_path = "/Users/cristian/Projects/dgapp_bkp/knowledge.db";
    let conn = icnow::open_db_connection(db_path).expect("Failed to open db");
    let sqlite_conn = conn.sqlite_connection();
    let iters = 200;

    println!("Running benchmarks ({} iterations each)\n", iters);

    // ==========================================
    // Use Case 1: Direct ID Lookup (Property)
    // ==========================================
    println!("=== Use Case 1: Direct ID Lookup ===");
    let cypher_1 = "MATCH (n) WHERE n.id = '/Users/cristian/Projects/dgapp_bkp/app/models/user.rb::User' RETURN n.id, n.name";
    let cypher_dur_1 = run_benchmark("Cypher (ID Lookup)", || {
        let _ = conn.cypher(cypher_1).unwrap();
    }, iters);

    let sql_1 = "
        SELECT p_id.value, p_name.value 
        FROM nodes n 
        JOIN node_props_text p_id ON n.id = p_id.node_id AND p_id.key_id = (SELECT id FROM property_keys WHERE key = 'id') 
        LEFT JOIN node_props_text p_name ON n.id = p_name.node_id AND p_name.key_id = (SELECT id FROM property_keys WHERE key = 'name') 
        WHERE p_id.value = '/Users/cristian/Projects/dgapp_bkp/app/models/user.rb::User'
    ";
    let sql_dur_1 = run_benchmark("SQL (ID Lookup)", || {
        let mut stmt = sqlite_conn.prepare(sql_1).unwrap();
        let _count = stmt.query_map([], |row| {
            let _id: String = row.get(0)?;
            let _name: Option<String> = row.get(1)?;
            Ok(())
        }).unwrap().count();
    }, iters);
    compare("ID Lookup", cypher_dur_1, sql_dur_1);

    // ==========================================
    // Use Case 2: Substring Search (LIKE)
    // ==========================================
    println!("=== Use Case 2: Substring Search (LIKE) ===");
    let cypher_2 = "MATCH (m:Method) WHERE m.name CONTAINS 'admin' RETURN m.name";
    let cypher_dur_2 = run_benchmark("Cypher (CONTAINS)", || {
        let _ = conn.cypher(cypher_2).unwrap();
    }, iters);

    let sql_2 = "
        SELECT p_name.value
        FROM nodes n
        JOIN node_labels l ON n.id = l.node_id AND l.label = 'Method'
        JOIN node_props_text p_name ON n.id = p_name.node_id AND p_name.key_id = (SELECT id FROM property_keys WHERE key = 'name')
        WHERE p_name.value LIKE '%admin%'
    ";
    let sql_dur_2 = run_benchmark("SQL (LIKE)", || {
        let mut stmt = sqlite_conn.prepare(sql_2).unwrap();
        let _count = stmt.query_map([], |row| {
            let _name: String = row.get(0)?;
            Ok(())
        }).unwrap().count();
    }, iters);
    compare("Substring Search", cypher_dur_2, sql_dur_2);

    // ==========================================
    // Use Case 3: 2-Hop Traversal (File -> Class -> Method)
    // ==========================================
    println!("=== Use Case 3: 2-Hop Traversal ===");
    let cypher_3 = "MATCH (f:File {id: '/Users/cristian/Projects/dgapp_bkp/app/models/user.rb'})-[:REL_CONTAINS]->(c:Class)-[:HAS_METHOD]->(m:Method) RETURN m.name";
    let cypher_dur_3 = run_benchmark("Cypher (2-Hop)", || {
        let _ = conn.cypher(cypher_3).unwrap();
    }, iters);

    let sql_3 = "
        SELECT p_m_name.value
        FROM node_props_text p_f_id
        JOIN edges e1 ON e1.source_id = p_f_id.node_id AND e1.type = 'REL_CONTAINS'
        JOIN node_labels l_c ON e1.target_id = l_c.node_id AND l_c.label = 'Class'
        JOIN edges e2 ON e2.source_id = l_c.node_id AND e2.type = 'HAS_METHOD'
        JOIN node_labels l_m ON e2.target_id = l_m.node_id AND l_m.label = 'Method'
        JOIN node_props_text p_m_name ON e2.target_id = p_m_name.node_id AND p_m_name.key_id = (SELECT id FROM property_keys WHERE key = 'name')
        WHERE p_f_id.key_id = (SELECT id FROM property_keys WHERE key = 'id') 
        AND p_f_id.value = '/Users/cristian/Projects/dgapp_bkp/app/models/user.rb'
    ";
    let sql_dur_3 = run_benchmark("SQL (2-Hop)", || {
        let mut stmt = sqlite_conn.prepare(sql_3).unwrap();
        let _count = stmt.query_map([], |row| {
            let _name: String = row.get(0)?;
            Ok(())
        }).unwrap().count();
    }, iters);
    compare("2-Hop Traversal", cypher_dur_3, sql_dur_3);
}
