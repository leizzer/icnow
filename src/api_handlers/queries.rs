use crate::tools::{
    GetFileStructureRequest, GetSymbolImplementationRequest, ListIndexedFilesRequest,
    QueryGraphCypherRequest, QueryGraphRequest, SearchSymbolsRequest,
};

pub fn handle_query_graph(
    db_path: &str,
    req: QueryGraphRequest,
) -> Result<String, String> {
    let output = std::process::Command::new("sqlite3")
        .arg(db_path)
        .arg("-header")
        .arg("-markdown")
        .arg(&req.query)
        .output()
        .map_err(|e| format!("Failed to execute query: {e}"))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Query failed: {err}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn handle_query_graph_cypher(
    db_path: &str,
    req: QueryGraphCypherRequest,
) -> Result<String, String> {
    let conn = crate::open_db_connection(db_path)
        .map_err(|e| format!("Failed to open graph database: {e}"))?;

    let res = conn
        .cypher(&req.query)
        .map_err(|e| format!("Cypher query failed: {e}"))?;

    crate::tools::format_cypher_result(res)
}

pub fn handle_search_symbols(
    db_path: &str,
    req: SearchSymbolsRequest,
) -> Result<String, String> {
    let limit = req.limit.unwrap_or(50);
    let safe_query = req.query.replace("'", "''");

    let resolved_project_root = req
        .project_root
        .clone()
        .or_else(|| crate::tools::infer_project_root(db_path))
        .unwrap_or_default();

    let project_root_norm = resolved_project_root.replace('\\', "/");
    let project_root_with_slash = if project_root_norm.is_empty() {
        "".to_string()
    } else if project_root_norm.ends_with('/') {
        project_root_norm
    } else {
        format!("{project_root_norm}/")
    };

    let safe_project_root = project_root_with_slash.replace("'", "''");

    let mut cypher_query = format!(
        "MATCH (n) WHERE \
         (NOT 'File' IN labels(n) AND (toLower(n.name) CONTAINS toLower('{safe_query}') OR toLower(replace(replace(n.id, '\\\\', '/'), '{safe_project_root}', '')) ENDS WITH toLower('::{safe_query}'))) \
         OR \
         ('File' IN labels(n) AND toLower(replace(replace(n.id, '\\\\', '/'), '{safe_project_root}', '')) CONTAINS toLower('{safe_query}'))"
    );

    if let Some(filters) = &req.kind_filter {
        if !filters.is_empty() {
            let conditions: Vec<String> = filters
                .iter()
                .map(|f| format!("'{}' IN labels(n)", f.replace("'", "''")))
                .collect();
            cypher_query.push_str(&format!(" AND ({})", conditions.join(" OR ")));
        }
    }

    cypher_query.push_str(&format!(
        " RETURN n.id as id, labels(n) as label ORDER BY \
         CASE \
           WHEN 'Class' IN labels(n) THEN 1 \
           WHEN 'Module' IN labels(n) THEN 2 \
           WHEN 'Struct' IN labels(n) THEN 3 \
           WHEN 'Interface' IN labels(n) THEN 4 \
           WHEN 'Method' IN labels(n) THEN 5 \
           WHEN 'Function' IN labels(n) THEN 6 \
           WHEN 'File' IN labels(n) THEN 7 \
           ELSE 8 \
         END, n.id LIMIT {limit}"
    ));

    let conn = crate::open_db_connection(db_path)
        .map_err(|e| format!("Failed to open graph database: {e}"))?;

    let res = conn
        .cypher(&cypher_query)
        .map_err(|e| format!("Cypher query failed: {e}"))?;

    crate::tools::format_cypher_result(res)
}

pub fn handle_get_file_structure(
    db_path: &str,
    req: GetFileStructureRequest,
) -> Result<String, String> {
    let safe_file_path = req.file_path.replace("'", "''");

    let conn = crate::open_db_connection(db_path)
        .map_err(|e| format!("Failed to open graph database: {e}"))?;

    let nodes_query = format!(
        "MATCH (f)-[r]->(n) WHERE f.id = '{safe_file_path}' AND type(r) = 'REL_CONTAINS' RETURN n.id as id, n.name as name, labels(n) as label"
    );
    let res_nodes = conn
        .cypher(&nodes_query)
        .map_err(|e| format!("Nodes query failed: {e}"))?;

    let mut nodes: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();
    for row in res_nodes {
        if let (Ok(id), Ok(name), Ok(label)) = (
            row.get::<String>("id"),
            row.get::<String>("name"),
            row.get::<String>("label"),
        ) {
            let clean_label = label
                .replace("[\"", "")
                .replace("\"]", "")
                .replace("[", "")
                .replace("]", "");
            nodes.insert(id, (name, clean_label));
        }
    }

    let edges_query = format!(
        "MATCH (s)-[:HAS_METHOD]->(m) WHERE s.id STARTS WITH '{safe_file_path}::' AND m.id STARTS WITH '{safe_file_path}::' RETURN s.id as parent, m.id as child"
    );
    let res_edges = conn
        .cypher(&edges_query)
        .map_err(|e| format!("Edges query failed: {e}"))?;

    let mut children_map: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut child_set: std::collections::HashSet<String> = std::collections::HashSet::new();

    for row in res_edges {
        if let (Ok(p), Ok(c)) = (row.get::<String>("parent"), row.get::<String>("child")) {
            children_map.entry(p).or_default().push(c.clone());
            child_set.insert(c);
        }
    }

    if nodes.is_empty() {
        return Ok(format!("No symbols found in file {}", req.file_path));
    }

    let mut out = format!("File Structure for `{}`:\n\n", req.file_path);
    let mut root_nodes: Vec<&String> = nodes.keys().filter(|k| !child_set.contains(*k)).collect();
    root_nodes.sort();

    for root_id in root_nodes {
        if let Some((name, label)) = nodes.get(root_id) {
            out.push_str(&format!("- {label} `{name}` ({root_id})\n"));
            if let Some(children) = children_map.get(root_id) {
                let mut sorted_children = children.clone();
                sorted_children.sort();
                for child_id in sorted_children {
                    if let Some((cname, clabel)) = nodes.get(&child_id) {
                        out.push_str(&format!("  - {clabel} `{cname}` ({child_id})\n"));
                    }
                }
            }
        }
    }

    Ok(out)
}

pub fn handle_list_indexed_files(
    db_path: &str,
    _req: ListIndexedFilesRequest,
) -> Result<String, String> {
    let cypher_query = "MATCH (n:File) RETURN n.id as FilePath ORDER BY FilePath";

    let conn = crate::open_db_connection(db_path)
        .map_err(|e| format!("Failed to open graph database: {e}"))?;

    let res = conn
        .cypher(cypher_query)
        .map_err(|e| format!("Cypher query failed: {e}"))?;

    crate::tools::format_cypher_result(res)
}

pub fn handle_get_symbol_implementation(
    db_path: &str,
    req: GetSymbolImplementationRequest,
) -> Result<String, String> {
    let safe_node_id = req.node_id.replace("'", "''");
    let cypher =
        format!("MATCH (n) WHERE n.id = '{safe_node_id}' RETURN n.source_code as source_code");

    let conn =
        crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
    let res = conn
        .cypher(&cypher)
        .map_err(|e| format!("Query failed: {e}"))?;

    let mut out = String::new();
    for row in res {
        if let Ok(src) = row.get::<String>("source_code") {
            out = src;
            break;
        }
    }

    if out.is_empty() {
        return Err(format!(
            "Source code not found for node '{}'. It might not be a structure/method, or the file lacks source mapping.",
            req.node_id
        ));
    }

    Ok(out)
}
