use icnow::api_handlers::queries::handle_query_graph_cypher;
use icnow::tools::QueryGraphCypherRequest;

fn main() {
    let req = QueryGraphCypherRequest {
        query: "MATCH (s) RETURN count(s)".to_string(),
        project_root: "/Users/cristian/Projects/dgapp_bkp".to_string(),
    };
    match handle_query_graph_cypher("/Users/cristian/Projects/dgapp_bkp/knowledge.db", req) {
        Ok(out) => println!("OUTPUT:\n{}", out),
        Err(e) => println!("ERROR: {}", e),
    }
}
