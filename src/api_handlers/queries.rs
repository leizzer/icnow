use crate::tools::{
    GetFileStructureRequest, GetSymbolImplementationRequest, GetSymbolInfoRequest,
    ListIndexedFilesRequest, QueryGraphCypherRequest, QueryGraphRequest, SearchSymbolsRequest,
};

pub fn handle_query_graph(
    _db_path: &str,
    _req: QueryGraphRequest,
) -> Result<String, String> {
    Err("SQLite queries are no longer supported. Please use Cypher with QueryGraphCypherRequest.".to_string())
}

pub fn handle_query_graph_cypher(db_path: &str, req: QueryGraphCypherRequest) -> Result<String, String> {
    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
    let mut res = conn.query(&req.query).map_err(|e| format!("Cypher query failed: {e}"))?;
    crate::tools::format_cypher_result(&mut res)
}

pub fn handle_search_symbols(db_path: &str, req: SearchSymbolsRequest) -> Result<String, String> {
    let limit = req.limit.unwrap_or(50);
    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
    
    let query_param = req.query.replace("'", "''");
    
    let mut q = format!("MATCH (n) WHERE (label(n) = 'Symbol' AND (n.name CONTAINS '{query_param}' OR n.id CONTAINS '{query_param}')) OR (label(n) = 'File' AND n.id CONTAINS '{query_param}')");
    
    if let Some(filters) = &req.kind_filter {
        if !filters.is_empty() {
            let filter_placeholders: Vec<String> = filters.iter().map(|f| format!("'{}'", f.replace("'", "''"))).collect();
            q.push_str(&format!(" AND (n.kind IN [{f}] OR label(n) IN [{f}])", f=filter_placeholders.join(", ")));
        }
    }
    
    q.push_str(&format!(" RETURN n.id AS id, label(n) AS label, n.signature AS signature, n.docstring AS docstring LIMIT {}", limit));
    
    let mut res = conn.query(&q).map_err(|e| format!("Failed to search symbols: {e}"))?;
    crate::tools::format_cypher_result(&mut res)
}

pub fn handle_get_file_structure(db_path: &str, req: GetFileStructureRequest) -> Result<String, String> {
    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
    let q = format!("MATCH (f:File {{id: '{}'}})-[:REL_CONTAINS]->(s:Symbol) RETURN s.id AS id, s.name AS name, s.kind AS label, s.signature AS signature, s.docstring AS docstring", req.file_path.replace("'", "''"));
    let mut res = conn.query(&q).map_err(|e| format!("Failed to query file structure: {e}"))?;
    crate::tools::format_cypher_result(&mut res)
}

pub fn handle_list_indexed_files(db_path: &str, _req: ListIndexedFilesRequest) -> Result<String, String> {
    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
    let mut res = conn.query("MATCH (f:File) RETURN f.id AS id ORDER BY f.id").map_err(|e| format!("Failed to list files: {e}"))?;
    crate::tools::format_cypher_result(&mut res)
}

pub fn handle_get_symbol_implementation(db_path: &str, req: GetSymbolImplementationRequest) -> Result<String, String> {
    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
    let q = format!("MATCH (s:Symbol {{id: '{}'}}) RETURN s.source_code AS source_code", req.node_id.replace("'", "''"));
    let mut res = conn.query(&q).map_err(|e| format!("Failed to get impl: {e}"))?;
    
    if let Some(row) = res.by_ref().next() {
        if let lbug::Value::String(s) = &row[0] {
            return Ok(s.clone());
        }
    }
    
    Err(format!("No implementation found for {}", req.node_id))
}

pub fn handle_get_symbol_info(db_path: &str, req: GetSymbolInfoRequest) -> Result<String, String> {
    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
    let node_id = req.node_id.replace("'", "''");
    
    let mut output = String::new();
    
    // 1. Get properties
    let q_props = format!("MATCH (s:Symbol {{id: '{}'}}) RETURN s.kind AS kind, s.signature AS signature, s.docstring AS docstring", node_id);
    match conn.query(&q_props) {
        Ok(mut res) => {
            if let Some(row) = res.next() {
                output.push_str(&format!("## Symbol: {}\n", node_id));
                output.push_str(&format!("**Kind**: {}\n", row[0].to_string()));
                if let lbug::Value::String(sig) = &row[1] {
                    if !sig.is_empty() { output.push_str(&format!("**Signature**: `{}`\n", sig)); }
                }
                if let lbug::Value::String(doc) = &row[2] {
                    if !doc.is_empty() { output.push_str(&format!("**Docstring**: {}\n", doc)); }
                }
                output.push_str("\n");
            } else {
                return Err(format!("Symbol not found: {}", req.node_id));
            }
        }
        Err(e) => return Err(format!("Failed to query properties: {e}")),
    }

    // 2. Get container
    let q_parent = format!("MATCH (parent)-[:REL_CONTAINS]->(s:Symbol {{id: '{}'}}) RETURN label(parent) AS parent_label, parent.id AS parent_id", node_id);
    if let Ok(mut res) = conn.query(&q_parent) {
        if let Some(row) = res.next() {
            output.push_str(&format!("**Container**: {} (`{}`)\n\n", row[0].to_string(), row[1].to_string()));
        }
    }

    // 3. Get outgoing dependencies (CALLS or IMPORTS)
    let q_out = format!("MATCH (s:Symbol {{id: '{}'}})-[r:CALLS|:IMPORTS]->(dep:Symbol) RETURN type(r) AS rel_type, dep.id AS target_id", node_id);
    if let Ok(mut res) = conn.query(&q_out) {
        let mut deps = Vec::new();
        while let Some(row) = res.next() {
            deps.push(format!("- [{}] -> `{}`", row[0].to_string(), row[1].to_string()));
        }
        if !deps.is_empty() {
            output.push_str("### Outgoing Dependencies (What this symbol calls/imports)\n");
            output.push_str(&deps.join("\n"));
            output.push_str("\n\n");
        }
    }

    // 4. Get incoming usages
    let q_in = format!("MATCH (caller:Symbol)-[r:CALLS|:IMPORTS]->(s:Symbol {{id: '{}'}}) RETURN type(r) AS rel_type, caller.id AS caller_id", node_id);
    if let Ok(mut res) = conn.query(&q_in) {
        let mut usages = Vec::new();
        while let Some(row) = res.next() {
            usages.push(format!("- `{}` -> [{}]", row[1].to_string(), row[0].to_string()));
        }
        if !usages.is_empty() {
            output.push_str("### Incoming Usages (What calls/imports this symbol)\n");
            output.push_str(&usages.join("\n"));
            output.push_str("\n\n");
        }
    }

    Ok(output.trim().to_string())
}
