use graphqlite::Connection;
use icnow::api_handlers::queries::handle_get_symbol_implementation;
use icnow::tools::GetSymbolImplementationRequest;

fn main() {
    let db_path = "/Users/cristian/Projects/blackhole/icnow/knowledge.db";
    let conn = Connection::open(db_path).unwrap();
    let sqlite_conn = conn.sqlite_connection();
    
    let mut stmt = sqlite_conn.prepare("
        SELECT p1.value
        FROM edges e 
        JOIN node_props_text p1 ON e.source_id = p1.node_id AND p1.key_id = (SELECT id FROM property_keys WHERE key='id')
        JOIN node_props_text src ON p1.node_id = src.node_id AND src.key_id = (SELECT id FROM property_keys WHERE key='source_code')
        LIMIT 1
    ").unwrap();
    
    let node_id: Option<String> = stmt.query_row([], |row| row.get(0)).ok();
    if let Some(id) = node_id {
        println!("Found node with edges and source code: {}", id);
        let req = GetSymbolImplementationRequest {
            node_id: id,
            project_root: None,
        };
        match handle_get_symbol_implementation(db_path, req) {
            Ok(res) => println!("RESULT:\n{}", res),
            Err(e) => println!("ERROR:\n{}", e),
        }
    } else {
        println!("No node found with edges and source code.");
    }
}
