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
    let q = format!("MATCH (n {{id: '{}'}}) RETURN n.id LIMIT 1", crate::models::escape_cypher_string(id));
    if let Ok(mut res) = conn.query(&q) {
        res.by_ref().next().is_some()
    } else {
        false
    }
}

fn resolve_target_id(id: &str, db_path: &str) -> String {
    if id.starts_with("memory::") {
        return id.to_string();
    }

    let parts: Vec<&str> = id.split("::").collect();
    if parts.is_empty() {
        return id.to_string();
    }

    let first_part = parts[0];
    let first_path = std::path::Path::new(first_part);
    if first_path.is_relative() {
        if let Some(db_dir) = std::path::Path::new(db_path).parent() {
            let abs_path = db_dir.join(first_part);
            let resolved_first = if let Ok(canon) = abs_path.canonicalize() {
                canon.to_string_lossy().to_string()
            } else {
                abs_path.to_string_lossy().to_string()
            };

            let mut new_parts = vec![resolved_first.as_str()];
            new_parts.extend_from_slice(&parts[1..]);
            return new_parts.join("::");
        }
    }

    id.to_string()
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
                let mut found = false;
                let escaped_target = crate::models::escape_cypher_string(target);
                let q_search = format!("MATCH (n) WHERE n.name = '{}' RETURN n.id LIMIT 1", escaped_target);
                if let Ok(mut res) = conn.query(&q_search) {
                    if let Some(row) = res.by_ref().next() {
                        if let Value::String(matched_id) = &row[0] {
                            resolved_links.push(matched_id.clone());
                            found = true;
                        }
                    }
                }
                if !found {
                    return Err(format!("Link target not found: '{target}' (also tried resolving to '{resolved}' and searching by name). Please make sure the code node has been scanned/indexed or the memory node exists."));
                }
            }
        }
    }

    let mut props = HashMap::new();
    props.insert("name".to_string(), req.name.clone());
    props.insert("description".to_string(), req.description.clone());
    let keywords_str = req.keywords.join(", ");
    props.insert("keywords".to_string(), keywords_str.clone());

    let text_to_embed = format!("{} {} {}", req.name, req.description, keywords_str);
    let model = crate::get_embedding_model();
    let mut model_lock = model.lock().unwrap();
    let embeddings = model_lock.embed(vec![text_to_embed], None).map_err(|e| format!("Failed to generate embedding: {}", e))?;
    if let Some(emb) = embeddings.into_iter().next() {
        let emb_str = format!("{:?}", emb);
        props.insert("embedding".to_string(), emb_str);
    }

    let mem_node = crate::models::Node {
        id: req.id.clone(),
        label: "Memory".to_string(),
        kind: "Memory".to_string(),
        properties: props,
    };
    mem_node.save(&conn).map_err(|e| format!("Failed to save memory node: {e}"))?;

    let q_delete_links = format!("MATCH (m:Memory {{id: '{}'}})-[r]->() DELETE r", crate::models::escape_cypher_string(&req.id));
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

    let q = format!("MATCH (m:Memory {{id: '{}'}}) RETURN m.name AS name, m.description AS description, m.keywords AS keywords", crate::models::escape_cypher_string(&req.id));
    let mut res = conn.query(&q).map_err(|e| format!("Failed to query memory: {e}"))?;

    let cols = res.get_column_names();
    let row = match res.by_ref().next() {
        Some(r) => r,
        None => return Err(format!("Memory node '{}' not found.", req.id)),
    };

    let name = get_str(&row, &cols, "name");
    let description = get_str(&row, &cols, "description");
    let keywords = get_str(&row, &cols, "keywords");

    let l_q = format!("MATCH (m:Memory {{id: '{}'}})-[r]->(target) RETURN target.id AS target_id, target.name AS target_name, struct_extract(r, '_LABEL') AS rel_type, label(target) AS target_labels", crate::models::escape_cypher_string(&req.id));
    let mut sub_concepts = Vec::new();
    let mut code_nodes = Vec::new();

    let mut links_res = conn.query(&l_q).map_err(|e| format!("Failed to query links: {e}"))?;
    let l_cols = links_res.get_column_names();
    for l_row in links_res.by_ref() {
        let target_id = get_str(&l_row, &l_cols, "target_id");
        let target_name = get_str(&l_row, &l_cols, "target_name");
        let rel_type = get_str(&l_row, &l_cols, "rel_type");
        let kind = get_str(&l_row, &l_cols, "target_labels");

        let mut display_name = target_name;
        if display_name.is_empty() {
            display_name = target_id.clone();
        }

        if target_id.starts_with("memory::") {
            sub_concepts.push(format!(
                "* [**{display_name}**]({target_id}) - Relationship: `{rel_type}`"
            ));
        } else {
            code_nodes.push(format!(
                "* **{kind}** (`{display_name}`) [id: `{target_id}`] - Relationship: `{rel_type}`"
            ));
        }
    }

    let mut output = format!(
        "# Memory: {}\n\n**ID**: `{}`\n**Keywords**: `{}`\n\n## Description\n{}\n",
        name, req.id, keywords, description
    );

    if !sub_concepts.is_empty() {
        output.push_str("\n## Related Sub-Concepts\n");
        output.push_str(&sub_concepts.join("\n"));
        output.push('\n');
    }

    if !code_nodes.is_empty() {
        output.push_str("\n## Connected Code Elements\n");
        output.push_str(&code_nodes.join("\n"));
        output.push('\n');
    }

    Ok(output)
}

pub fn handle_search_memories(db_path: &str, req: SearchMemoriesRequest) -> Result<String, String> {
    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
    
    let limit = 10;
    
    let model = crate::get_embedding_model();
    let mut model_lock = model.lock().unwrap();
    let query_embeddings = model_lock.embed(vec![req.query.clone()], None).map_err(|e| format!("Failed to generate embedding: {}", e))?;
    let query_vector_str = if let Some(emb) = query_embeddings.into_iter().next() {
        format!("{:?}", emb)
    } else {
        return Err("Failed to generate embedding vector".to_string());
    };
    
    let q = format!("MATCH (m:Memory) WITH m, array_cosine_similarity(m.embedding, {}) AS sim WHERE sim > 0.3 RETURN m.id AS id, m.name AS name, m.description AS description, m.keywords AS keywords ORDER BY sim DESC LIMIT {}", query_vector_str, limit);
    
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
