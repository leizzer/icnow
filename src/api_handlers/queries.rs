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

    let conn = crate::open_db_connection(db_path)
        .map_err(|e| format!("Failed to open graph database: {e}"))?;
    let sqlite_conn = conn.sqlite_connection();

    let mut sql = "SELECT p_id.value AS id, l.label AS label, p_sig.value AS signature, p_doc.value AS docstring \
                   FROM nodes n \
                   JOIN node_labels l ON n.id = l.node_id \
                   JOIN node_props_text p_id ON n.id = p_id.node_id AND p_id.key_id = (SELECT id FROM property_keys WHERE key = 'id') \
                   LEFT JOIN node_props_text p_name ON n.id = p_name.node_id AND p_name.key_id = (SELECT id FROM property_keys WHERE key = 'name') \
                   LEFT JOIN node_props_text p_sig ON n.id = p_sig.node_id AND p_sig.key_id = (SELECT id FROM property_keys WHERE key = 'signature') \
                   LEFT JOIN node_props_text p_doc ON n.id = p_doc.node_id AND p_doc.key_id = (SELECT id FROM property_keys WHERE key = 'docstring') \
                   WHERE ".to_string();

    let query_param = format!("%{}%", req.query);
    let ends_with_param = format!("%::{}", req.query);

    let mut conditions = Vec::new();
    conditions.push(
        "((l.label <> 'File' AND (p_name.value LIKE ?1 OR p_id.value LIKE ?2)) OR (l.label = 'File' AND p_id.value LIKE ?1))".to_string()
    );

    if let Some(filters) = &req.kind_filter {
        if !filters.is_empty() {
            let filter_placeholders: Vec<String> = filters
                .iter()
                .map(|f| format!("'{}'", f.replace("'", "''")))
                .collect();
            conditions.push(format!("l.label IN ({})", filter_placeholders.join(", ")));
        }
    }

    sql.push_str(&conditions.join(" AND "));
    sql.push_str(&format!(
        " ORDER BY \
         CASE \
           WHEN l.label = 'Class' THEN 1 \
           WHEN l.label = 'Module' THEN 2 \
           WHEN l.label = 'Struct' THEN 3 \
           WHEN l.label = 'Interface' THEN 4 \
           WHEN l.label = 'Method' THEN 5 \
           WHEN l.label = 'Function' THEN 6 \
           WHEN l.label = 'File' THEN 7 \
           ELSE 8 \
         END, p_id.value LIMIT {limit}"
    ));

    let mut stmt = sqlite_conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare SQL query: {e}"))?;

    let mut rows = stmt
        .query(rusqlite::params![query_param, ends_with_param])
        .map_err(|e| format!("Query execution failed: {e}"))?;

    let mut out = "| id | label | signature | docstring |\n| --- | --- | --- | --- |\n".to_string();
    let mut count = 0;
    while let Some(row) = rows.next().map_err(|e| format!("Failed to fetch row: {e}"))? {
        let id: String = row.get(0).map_err(|e| format!("Failed to get id: {e}"))?;
        let label: String = row.get(1).map_err(|e| format!("Failed to get label: {e}"))?;
        let signature: String = row.get(2).unwrap_or_default();
        let docstring: String = row.get(3).unwrap_or_default();
        let label_arr = format!("[\"{label}\"]");
        let safe_sig = signature.replace("|", "\\|").replace("\n", " ");
        let safe_doc = docstring.replace("|", "\\|").replace("\n", " ");
        let safe_doc_trunc = if safe_doc.len() > 100 { format!("{}...", &safe_doc[..97]) } else { safe_doc };
        out.push_str(&format!("| {id} | {label_arr} | `{safe_sig}` | {safe_doc_trunc} |\n"));
        count += 1;
    }

    if count == 0 {
        return Ok("No columns returned.".to_string());
    }

    Ok(out)
}

pub fn handle_get_file_structure(
    db_path: &str,
    req: GetFileStructureRequest,
) -> Result<String, String> {
    let conn = crate::open_db_connection(db_path)
        .map_err(|e| format!("Failed to open graph database: {e}"))?;
    let sqlite_conn = conn.sqlite_connection();

    let file_node_rowid: Option<i64> = sqlite_conn
        .query_row(
            "SELECT node_id FROM node_props_text WHERE key_id = (SELECT id FROM property_keys WHERE key = 'id') AND value = ?",
            [&req.file_path],
            |row| row.get(0),
        )
        .ok();

    let file_node_rowid = match file_node_rowid {
        Some(rid) => rid,
        None => return Ok(format!("No symbols found in file {}", req.file_path)),
    };

    let mut stmt_nodes = sqlite_conn
        .prepare(
            "SELECT p_id.value AS id, p_name.value AS name, l.label AS label, p_sig.value AS signature, p_doc.value AS docstring \
             FROM edges e \
             JOIN node_labels l ON e.target_id = l.node_id \
             JOIN node_props_text p_id ON e.target_id = p_id.node_id AND p_id.key_id = (SELECT id FROM property_keys WHERE key = 'id') \
             LEFT JOIN node_props_text p_name ON e.target_id = p_name.node_id AND p_name.key_id = (SELECT id FROM property_keys WHERE key = 'name') \
             LEFT JOIN node_props_text p_sig ON e.target_id = p_sig.node_id AND p_sig.key_id = (SELECT id FROM property_keys WHERE key = 'signature') \
             LEFT JOIN node_props_text p_doc ON e.target_id = p_doc.node_id AND p_doc.key_id = (SELECT id FROM property_keys WHERE key = 'docstring') \
             WHERE e.source_id = ?1 AND e.type = 'REL_CONTAINS'",
        )
        .map_err(|e| format!("Failed to prepare nodes statement: {e}"))?;

    let mut rows_nodes = stmt_nodes
        .query([file_node_rowid])
        .map_err(|e| format!("Failed to query nodes: {e}"))?;

    let mut nodes = std::collections::HashMap::new();
    while let Some(row) = rows_nodes.next().map_err(|e| format!("Failed to fetch node: {e}"))? {
        let id: String = row.get(0).map_err(|e| format!("Failed to get id: {e}"))?;
        let name: String = row.get(1).unwrap_or_default();
        let label: String = row.get(2).map_err(|e| format!("Failed to get label: {e}"))?;
        let sig: String = row.get(3).unwrap_or_default();
        let doc: String = row.get(4).unwrap_or_default();
        nodes.insert(id, (name, label, sig, doc));
    }

    if nodes.is_empty() {
        return Ok(format!("No symbols found in file {}", req.file_path));
    }

    let mut stmt_edges = sqlite_conn
        .prepare(
            "SELECT p_source_id.value AS parent, p_target_id.value AS child \
             FROM edges e \
             JOIN node_props_text p_source_id ON e.source_id = p_source_id.node_id AND p_source_id.key_id = (SELECT id FROM property_keys WHERE key = 'id') \
             JOIN node_props_text p_target_id ON e.target_id = p_target_id.node_id AND p_target_id.key_id = (SELECT id FROM property_keys WHERE key = 'id') \
             WHERE e.type = 'HAS_METHOD' \
               AND p_source_id.value LIKE ?1 \
               AND p_target_id.value LIKE ?1",
        )
        .map_err(|e| format!("Failed to prepare edges statement: {e}"))?;

    let file_path_prefix = format!("{}::%", req.file_path);
    let mut rows_edges = stmt_edges
        .query([&file_path_prefix])
        .map_err(|e| format!("Failed to query edges: {e}"))?;

    let mut children_map: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut child_set: std::collections::HashSet<String> = std::collections::HashSet::new();

    while let Some(row) = rows_edges.next().map_err(|e| format!("Failed to fetch edge: {e}"))? {
        let p: String = row.get(0).map_err(|e| format!("Failed to get parent: {e}"))?;
        let c: String = row.get(1).map_err(|e| format!("Failed to get child: {e}"))?;
        children_map.entry(p).or_default().push(c.clone());
        child_set.insert(c);
    }

    let mut out = format!("File Structure for `{}`:\n\n", req.file_path);
    let mut root_nodes: Vec<&String> = nodes.keys().filter(|k| !child_set.contains(*k)).collect();
    root_nodes.sort();

    for root_id in root_nodes {
        if let Some((name, label, sig, doc)) = nodes.get(root_id) {
            let sig_fmt = if sig.is_empty() { name.clone() } else { sig.clone() };
            let doc_fmt = if doc.is_empty() { "".to_string() } else { format!(" - {}", doc.replace("\n", " ")) };
            out.push_str(&format!("- {label} `{sig_fmt}` ({root_id}){doc_fmt}\n"));
            if let Some(children) = children_map.get(root_id) {
                let mut sorted_children = children.clone();
                sorted_children.sort();
                for child_id in sorted_children {
                    if let Some((cname, clabel, csig, cdoc)) = nodes.get(&child_id) {
                        let csig_fmt = if csig.is_empty() { cname.clone() } else { csig.clone() };
                        let cdoc_fmt = if cdoc.is_empty() { "".to_string() } else { format!(" - {}", cdoc.replace("\n", " ")) };
                        out.push_str(&format!("  - {clabel} `{csig_fmt}` ({child_id}){cdoc_fmt}\n"));
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
    let conn = crate::open_db_connection(db_path)
        .map_err(|e| format!("Failed to open graph database: {e}"))?;
    let sqlite_conn = conn.sqlite_connection();

    let mut stmt = sqlite_conn
        .prepare(
            "SELECT p.value AS FilePath \
             FROM node_labels l \
             JOIN node_props_text p ON l.node_id = p.node_id AND p.key_id = (SELECT id FROM property_keys WHERE key = 'id') \
             WHERE l.label = 'File' \
             ORDER BY FilePath",
        )
        .map_err(|e| format!("Failed to prepare SQL query: {e}"))?;

    let mut rows = stmt
        .query([])
        .map_err(|e| format!("Query execution failed: {e}"))?;

    let mut out = "| FilePath |\n| --- |\n".to_string();
    let mut count = 0;
    while let Some(row) = rows.next().map_err(|e| format!("Failed to fetch row: {e}"))? {
        let file_path: String = row.get(0).map_err(|e| format!("Failed to get FilePath: {e}"))?;
        out.push_str(&format!("| {file_path} |\n"));
        count += 1;
    }

    if count == 0 {
        return Ok("No columns returned.".to_string());
    }

    Ok(out)
}

pub fn handle_get_symbol_implementation(
    db_path: &str,
    req: GetSymbolImplementationRequest,
) -> Result<String, String> {
    let conn = crate::open_db_connection(db_path)
        .map_err(|e| format!("Failed to open graph database: {e}"))?;
    let sqlite_conn = conn.sqlite_connection();

    let node_rowid: Option<i64> = sqlite_conn
        .query_row(
            "SELECT node_id FROM node_props_text WHERE key_id = (SELECT id FROM property_keys WHERE key = 'id') AND value = ?1",
            [&req.node_id],
            |row| row.get(0),
        )
        .ok();

    if node_rowid.is_none() {
        return Err(format!("Node '{}' not found in database.", req.node_id));
    }
    let node_rowid = node_rowid.unwrap();

    let mut stmt = sqlite_conn
        .prepare(
            "SELECT value \
             FROM node_props_text \
             WHERE node_id = ?1 AND key_id = (SELECT id FROM property_keys WHERE key = 'source_code')",
        )
        .map_err(|e| format!("Failed to prepare SQL query: {e}"))?;

    let mut out = String::new();
    if let Ok(src) = stmt.query_row([node_rowid], |row| row.get::<_, String>(0)) {
        out = src;
    }

    if out.is_empty() {
        return Err(format!(
            "Source code not found for node '{}'. It might not be a structure/method, or the file lacks source mapping.",
            req.node_id
        ));
    }

    let mut stmt_edges = sqlite_conn
        .prepare(
            "SELECT p_source.value, p_target.value, e.type, e.source_id \
             FROM edges e \
             JOIN node_props_text p_source ON e.source_id = p_source.node_id AND p_source.key_id = (SELECT id FROM property_keys WHERE key = 'id') \
             JOIN node_props_text p_target ON e.target_id = p_target.node_id AND p_target.key_id = (SELECT id FROM property_keys WHERE key = 'id') \
             WHERE e.source_id = ?1 OR e.target_id = ?1",
        )
        .map_err(|e| format!("Failed to prepare edges query: {e}"))?;

    let mut edges_str = String::new();
    let mut rows_edges = stmt_edges
        .query([node_rowid])
        .map_err(|e| format!("Edges query failed: {e}"))?;

    while let Some(row) = rows_edges.next().ok().flatten() {
        if let (Ok(source), Ok(target), Ok(rel_type), Ok(source_id)) = (
            row.get::<_, String>(0),
            row.get::<_, String>(1),
            row.get::<_, String>(2),
            row.get::<_, i64>(3),
        ) {
            if source_id == node_rowid {
                edges_str.push_str(&format!("- {} -> {}\n", rel_type, target));
            } else {
                edges_str.push_str(&format!("- <- {} from {}\n", rel_type, source));
            }
        }
    }

    if !edges_str.is_empty() {
        out.push_str("\n\n--- Related Metadata ---\n");
        out.push_str(&edges_str);
    }

    Ok(out)
}
