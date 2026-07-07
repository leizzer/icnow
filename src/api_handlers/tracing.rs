use crate::tools::{GetDependenciesRequest, TraceCallPathRequest};

pub fn handle_trace_call_path(db_path: &str, req: TraceCallPathRequest) -> Result<String, String> {
    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;

    let max_depth = req.max_depth.unwrap_or(5);

    let q = if !req.end_node_id.is_empty() {
        format!(
            "MATCH (s:Symbol {{id: '{}'}})-[e:CALLS*1..{}]->(t:Symbol {{id: '{}'}}) RETURN e",
            req.start_node_id.replace("'", "''"),
            max_depth,
            req.end_node_id.replace("'", "''")
        )
    } else {
        format!(
            "MATCH (s:Symbol {{id: '{}'}})-[e:CALLS*1..{}]->(t:Symbol) RETURN e, t.id AS target_id",
            req.start_node_id.replace("'", "''"),
            max_depth
        )
    };

    let mut res = conn
        .query(&q)
        .map_err(|e| format!("Tracing query failed: {e}"))?;
    crate::tools::format_cypher_result(&mut res)
}

pub fn handle_get_dependencies(
    db_path: &str,
    req: GetDependenciesRequest,
) -> Result<String, String> {
    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;

    let q = if req.direction == "incoming" {
        format!(
            "MATCH (t:Symbol)-[:CALLS|:IMPORTS|:DEPENDS_ON|:INHERITS|:INSTANTIATES]->(s:Symbol {{id: '{}'}}) RETURN t.id AS target_id, t.name AS name, t.kind AS kind",
            req.node_id.replace("'", "''")
        )
    } else {
        format!(
            "MATCH (s:Symbol {{id: '{}'}})-[:CALLS|:IMPORTS|:DEPENDS_ON|:INHERITS|:INSTANTIATES]->(t:Symbol) RETURN t.id AS target_id, t.name AS name, t.kind AS kind",
            req.node_id.replace("'", "''")
        )
    };

    let mut res = conn
        .query(&q)
        .map_err(|e| format!("Dependency query failed: {e}"))?;
    crate::tools::format_cypher_result(&mut res)
}

pub fn handle_traverse_graph(
    db_path: &str,
    req: crate::tools::TraverseGraphRequest,
) -> Result<String, String> {
    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
    let max_depth = req.max_depth.unwrap_or(2);
    let q = format!(
        "MATCH (s:Symbol {{id: '{}'}})-[r*1..{}]-(t) RETURN r, t.id AS target_id LIMIT 100",
        req.node_id.replace("'", "''"),
        max_depth
    );
    let mut res = conn
        .query(&q)
        .map_err(|e| format!("Failed to traverse graph: {e}"))?;
    crate::tools::format_cypher_result(&mut res)
}
