use crate::tools::{
    GetFileStructureRequest, GetSymbolImplementationRequest, ListIndexedFilesRequest,
    QueryGraphCypherRequest, QueryGraphRequest, SearchSymbolsRequest,
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
