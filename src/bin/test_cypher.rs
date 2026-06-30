use icnow::api_handlers::queries::handle_query_graph_cypher;
use icnow::tools::QueryGraphCypherRequest;

fn main() {
    let req = QueryGraphCypherRequest {
        query: "MATCH (s:Symbol) RETURN s.id, s.name, s.kind".to_string(),
        project_root: ".".to_string(),
    };
    match handle_query_graph_cypher("test_parser_db.db", req) {
        Ok(out) => println!("OUTPUT:\n{}", out),
        Err(e) => println!("ERROR: {}", e),
    }
}
