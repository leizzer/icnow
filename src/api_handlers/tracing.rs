use crate::tools::{GetDependenciesRequest, TraceCallPathRequest, TraverseGraphRequest};

pub fn handle_traverse_graph(
    db_path: &str,
    req: TraverseGraphRequest,
) -> Result<String, String> {
    let depth = req.max_depth.unwrap_or(2);
    let safe_node_id = req.node_id.replace("'", "''");

    let query = if depth <= 1 {
        format!(
            "MATCH (n) WHERE n.id = '{safe_node_id}' MATCH (n)-[r]->(y) RETURN n.id as source, type(r) as label, y.id as target \
             UNION \
             MATCH (n) WHERE n.id = '{safe_node_id}' MATCH (y)-[r]->(n) RETURN y.id as source, type(r) as label, n.id as target"
        )
    } else {
        let depth_minus_1 = depth - 1;
        format!(
            "MATCH (n) WHERE n.id = '{safe_node_id}' MATCH (n)-[r]->(y) RETURN n.id as source, type(r) as label, y.id as target \
             UNION \
             MATCH (n) WHERE n.id = '{safe_node_id}' MATCH (y)-[r]->(n) RETURN y.id as source, type(r) as label, n.id as target \
             UNION \
             MATCH (n)-[*1..{depth_minus_1}]->(x) WHERE n.id = '{safe_node_id}' MATCH (x)-[r]->(y) RETURN x.id as source, type(r) as label, y.id as target \
             UNION \
             MATCH (n)-[*1..{depth_minus_1}]->(x) WHERE n.id = '{safe_node_id}' MATCH (y)-[r]->(x) RETURN y.id as source, type(r) as label, x.id as target \
             UNION \
             MATCH (n)<-[*1..{depth_minus_1}]-(x) WHERE n.id = '{safe_node_id}' MATCH (x)-[r]->(y) RETURN x.id as source, type(r) as label, y.id as target \
             UNION \
             MATCH (n)<-[*1..{depth_minus_1}]-(x) WHERE n.id = '{safe_node_id}' MATCH (y)-[r]->(x) RETURN y.id as source, type(r) as label, x.id as target"
        )
    };

    let conn = crate::open_db_connection(db_path)
        .map_err(|e| format!("Failed to open graph database: {e}"))?;

    let res = conn
        .cypher(&query)
        .map_err(|e| format!("Cypher query failed: {e}"))?;

    crate::tools::format_cypher_result(res)
}

pub fn handle_get_dependencies(
    db_path: &str,
    req: GetDependenciesRequest,
) -> Result<String, String> {
    let safe_node_id = req.node_id.replace("'", "''");

    let cypher_query = if req.direction == "incoming" {
        format!(
            "MATCH (source)-[r]->(target) WHERE toLower(target.id) ENDS WITH toLower('{safe_node_id}') OR toLower(target.id) = toLower('{safe_node_id}') RETURN source.id as caller, type(r) as relationship"
        )
    } else {
        format!(
            "MATCH (source)-[r]->(target) WHERE toLower(source.id) ENDS WITH toLower('{safe_node_id}') OR toLower(source.id) = toLower('{safe_node_id}') RETURN type(r) as relationship, target.id as callee"
        )
    };

    let conn = crate::open_db_connection(db_path)
        .map_err(|e| format!("Failed to open graph database: {e}"))?;

    let res = conn
        .cypher(&cypher_query)
        .map_err(|e| format!("Cypher query failed: {e}"))?;

    crate::tools::format_cypher_result(res)
}

pub fn handle_trace_call_path(
    db_path: &str,
    req: TraceCallPathRequest,
) -> Result<String, String> {
    let conn = crate::open_db_connection(db_path)
        .map_err(|e| format!("Failed to open graph database: {e}"))?;

    let max_depth = req.max_depth.unwrap_or(5);
    if max_depth > 10 {
        return Err("max_depth cannot exceed 10".into());
    }

    let mut queue = std::collections::VecDeque::new();
    queue.push_back(vec![req.start_node_id.clone()]);
    let mut visited = std::collections::HashSet::new();
    visited.insert(req.start_node_id.clone());

    let mut paths = Vec::new();

    while let Some(path) = queue.pop_front() {
        if path.len() - 1 > max_depth as usize {
            continue;
        }
        let current = path.last().unwrap();
        if current == &req.end_node_id {
            paths.push(path.clone());
            if paths.len() >= 5 {
                break;
            } // Limit to 5 paths to avoid massive output
            continue;
        }

        let safe_curr = current.replace("'", "''");
        let q = format!(
            "MATCH (s)-[r:CALLS]->(t) WHERE s.id = '{safe_curr}' RETURN t.id as target"
        );
        if let Ok(res) = conn.cypher(&q) {
            for row in res {
                if let Ok(target) = row.get::<String>("target") {
                    if !path.contains(&target) {
                        // avoid cycles
                        let mut new_path = path.clone();
                        new_path.push(target.clone());
                        queue.push_back(new_path);
                    }
                }
            }
        }
    }

    if paths.is_empty() {
        return Ok(format!(
            "No CALLS path found between {} and {} within {} hops.",
            req.start_node_id, req.end_node_id, max_depth
        ));
    }

    let mut out = format!(
        "Found {} path(s) between {} and {}:\n\n",
        paths.len(),
        req.start_node_id,
        req.end_node_id
    );
    for (i, p) in paths.iter().enumerate() {
        out.push_str(&format!("Path {}:\n", i + 1));
        out.push_str(&p.join(" -> "));
        out.push_str("\n\n");
    }

    Ok(out)
}
