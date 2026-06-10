use crate::tools::{GetMemoryRequest, ListMemoriesRequest, SaveMemoryRequest, SearchMemoriesRequest};
use lbug::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct MemoryLink {
    pub target_id: String,
    pub target_name: String,
    pub rel_type: String,
    pub target_type: String,
}

#[derive(Serialize, Deserialize)]
pub struct GetMemoryResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub links: Vec<MemoryLink>,
}

#[derive(Serialize, Deserialize)]
pub struct MemorySearchResult {
    pub id: String,
    pub name: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub snippet: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct MemoryListItem {
    pub id: String,
    pub name: String,
}

fn node_exists(conn: &lbug::Connection, id: &str) -> bool {
    let q = format!("MATCH (n {{id: '{}'}}) RETURN n.id LIMIT 1", id.replace("'", "''"));
    if let Ok(mut res) = conn.query(&q) {
        res.by_ref().next().is_some()
    } else {
        false
    }
}

fn resolve_target_id(id: &str, db_path: &str) -> String {
    let mut resolved = id.to_string();
    if resolved.starts_with('/') && !resolved.contains("::") {
        let abs_path = std::fs::canonicalize(resolved.clone()).unwrap_or_else(|_| std::path::PathBuf::from(resolved.clone()));
        resolved = abs_path.to_string_lossy().to_string();
    }
    resolved
}

fn get_str(row: &[Value], cols: &[String], name: &str) -> String {
    cols.iter().position(|c| c == name).and_then(|idx| {
        if let Value::String(s) = &row[idx] { Some(s.clone()) } else { None }
    }).unwrap_or_default()
}

pub fn handle_save_memory(db_path: &str, req: SaveMemoryRequest) -> Result<String, String> {
    if !req.id.starts_with("memory::") {
        return Err("Memory ID must start with 'memory::' prefix. E.g. 'memory::user_auth_flow'".to_string());
    }

    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;

    let mut resolved_links = Vec::with_capacity(req.links.len());
    for target in &req.links {
        if node_exists(&conn, target) {
            resolved_links.push(target.clone());
        } else {
            let resolved = resolve_target_id(target, db_path);
            if node_exists(&conn, &resolved) {
                resolved_links.push(resolved);
            } else {
                return Err(format!("Link target not found: '{target}' (also tried resolving to '{resolved}'). Please make sure the code node has been scanned/indexed or the memory node exists."));
            }
        }
    }

    let mut props = HashMap::new();
    props.insert("name".to_string(), req.name.clone());
    props.insert("description".to_string(), req.description.clone());
    let keywords_str = req.keywords.join(", ");
    props.insert("keywords".to_string(), keywords_str.clone());

    let mem_node = crate::models::Node {
        id: req.id.clone(),
        label: "Memory".to_string(),
        kind: "Memory".to_string(),
        properties: props,
    };
    mem_node.save(&conn).map_err(|e| format!("Failed to save memory node: {e}"))?;

    let q_delete_links = format!("MATCH (m:Memory {{id: '{}'}})-[r]->() DELETE r", req.id.replace("'", "''"));
    let _ = conn.query(&q_delete_links);

    for target_id in &resolved_links {
        let rel = req.link_type.as_deref().unwrap_or_else(|| {
            if target_id.starts_with("memory::") { "LINKS_TO" } else { "LINKS_TO" }
        });
        
        let edge = crate::models::Edge {
            id: format!("{}::{}::{}", req.id, rel, target_id),
            source: req.id.clone(),
            target: target_id.clone(),
            label: rel.to_string(),
            properties: HashMap::new(),
        };
        edge.save(&conn).map_err(|e| format!("Failed to link {} to {}: {}", req.id, target_id, e))?;
    }

    Ok(format!("Memory node '{}' saved successfully with {} links.", req.id, resolved_links.len()))
}

pub fn handle_get_memory(db_path: &str, req: GetMemoryRequest) -> Result<String, String> {
    if !req.id.starts_with("memory::") {
        return Err("Memory ID must start with 'memory::' prefix".to_string());
    }

    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;

    let q = format!("MATCH (m:Memory {{id: '{}'}}) RETURN m.name AS name, m.description AS description, m.keywords AS keywords", req.id.replace("'", "''"));
    let mut res = conn.query(&q).map_err(|e| format!("Failed to query memory: {e}"))?;

    let cols = res.get_column_names();
    let row = match res.by_ref().next() {
        Some(r) => r,
        None => return Err(format!("Memory node '{}' not found.", req.id)),
    };

    let name = get_str(&row, &cols, "name");
    let description = get_str(&row, &cols, "description");
    let keywords = get_str(&row, &cols, "keywords");

    let l_q = format!("MATCH (m:Memory {{id: '{}'}})-[r]->(target) RETURN target.id AS target_id, target.name AS target_name, type(r) AS rel_type, label(target) AS target_labels", req.id.replace("'", "''"));
    let mut links = Vec::new();
    if let Ok(mut links_res) = conn.query(&l_q) {
        let l_cols = links_res.get_column_names();
        for l_row in links_res.by_ref() {
            links.push(MemoryLink {
                target_id: get_str(&l_row, &l_cols, "target_id"),
                target_name: get_str(&l_row, &l_cols, "target_name"),
                rel_type: get_str(&l_row, &l_cols, "rel_type"),
                target_type: get_str(&l_row, &l_cols, "target_labels"),
            });
        }
    }

    let mem_res = GetMemoryResponse {
        id: req.id,
        name,
        description,
        keywords: keywords.split(", ").filter(|s| !s.is_empty()).map(|s| s.to_string()).collect(),
        links,
    };

    Ok(serde_json::to_string_pretty(&mem_res).unwrap())
}

pub fn handle_search_memories(db_path: &str, req: SearchMemoriesRequest) -> Result<String, String> {
    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
    
    let limit = 10;
    let q_term = req.query.replace("'", "''");
    
    // Naive Cypher CONTAINS search
    let q = format!("MATCH (m:Memory) WHERE m.name CONTAINS '{q_term}' OR m.description CONTAINS '{q_term}' OR m.keywords CONTAINS '{q_term}' RETURN m.id AS id, m.name AS name, m.description AS description, m.keywords AS keywords LIMIT {limit}");
    
    let mut res = conn.query(&q).map_err(|e| format!("Failed to search memories: {e}"))?;
    let cols = res.get_column_names();
    let mut results = Vec::new();
    
    for row in res.by_ref() {
        results.push(MemorySearchResult {
            id: get_str(&row, &cols, "id"),
            name: get_str(&row, &cols, "name"),
            description: get_str(&row, &cols, "description"),
            keywords: get_str(&row, &cols, "keywords").split(", ").filter(|s| !s.is_empty()).map(|s| s.to_string()).collect(),
            snippet: None,
        });
    }

    Ok(serde_json::to_string_pretty(&results).unwrap())
}

pub fn handle_list_memories(db_path: &str, _req: ListMemoriesRequest) -> Result<String, String> {
    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
    
    let mut res = conn.query("MATCH (m:Memory) RETURN m.id AS id, m.name AS name").map_err(|e| format!("Failed to list memories: {e}"))?;
    let cols = res.get_column_names();
    let mut results = Vec::new();
    
    for row in res.by_ref() {
        results.push(MemoryListItem {
            id: get_str(&row, &cols, "id"),
            name: get_str(&row, &cols, "name"),
        });
    }
    
    Ok(serde_json::to_string_pretty(&results).unwrap())
}
