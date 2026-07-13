use crate::tools::{GetDependenciesRequest, TraceCallPathRequest};

pub fn handle_trace_call_path(db_path: &str, req: TraceCallPathRequest) -> Result<String, String> {
    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;

    let max_depth = req.max_depth.unwrap_or(5);

    let abs_start = crate::tools::absolute_node_id(&req.start_node_id);
    let q = if !req.end_node_id.is_empty() {
        let abs_end = crate::tools::absolute_node_id(&req.end_node_id);
        format!(
            "MATCH (s:Symbol {{id: '{}'}})-[e:CALLS*1..{}]->(t:Symbol {{id: '{}'}}) RETURN e",
            abs_start.replace("'", "''"),
            max_depth,
            abs_end.replace("'", "''")
        )
    } else {
        format!(
            "MATCH (s:Symbol {{id: '{}'}})-[e:CALLS*1..{}]->(t:Symbol) RETURN e, t.id AS target_id",
            abs_start.replace("'", "''"),
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

    let abs_node = crate::tools::absolute_node_id(&req.node_id);
    let q = if req.direction == "incoming" {
        format!(
            "MATCH (t:Symbol)-[:CALLS|:IMPORTS|:DEPENDS_ON|:INHERITS|:INSTANTIATES]->(s:Symbol {{id: '{}'}}) RETURN t.id AS target_id, t.name AS name, t.kind AS kind",
            abs_node.replace("'", "''")
        )
    } else {
        format!(
            "MATCH (s:Symbol {{id: '{}'}})-[:CALLS|:IMPORTS|:DEPENDS_ON|:INHERITS|:INSTANTIATES]->(t:Symbol) RETURN t.id AS target_id, t.name AS name, t.kind AS kind",
            abs_node.replace("'", "''")
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
    let abs_node = crate::tools::absolute_node_id(&req.node_id);
    let q = format!(
        "MATCH (s:Symbol {{id: '{}'}})-[r*1..{}]-(t) RETURN r, t.id AS target_id LIMIT 100",
        abs_node.replace("'", "''"),
        max_depth
    );
    let mut res = conn
        .query(&q)
        .map_err(|e| format!("Failed to traverse graph: {e}"))?;
    crate::tools::format_cypher_result(&mut res)
}
