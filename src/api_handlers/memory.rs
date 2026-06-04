use crate::tools::{GetMemoryRequest, ListMemoriesRequest, SaveMemoryRequest, SearchMemoriesRequest};
use std::collections::HashMap;

fn node_exists(conn: &rusqlite::Connection, id: &str) -> bool {
    let id_key_id: Option<i64> = conn
        .query_row("SELECT id FROM property_keys WHERE key = 'id'", [], |row| {
            row.get(0)
        })
        .ok();

    if let Some(key_id) = id_key_id {
        let exists: Option<i64> = conn
            .query_row(
                "SELECT node_id FROM node_props_text WHERE key_id = ? AND value = ?",
                (key_id, id),
                |row| row.get(0),
            )
            .ok();
        exists.is_some()
    } else {
        false
    }
}

fn resolve_target_id(target: &str, db_path: &str) -> String {
    if target.starts_with("memory::") {
        return target.to_string();
    }

    let parts: Vec<&str> = target.split("::").collect();
    if parts.is_empty() {
        return target.to_string();
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

    target.to_string()
}

pub fn handle_save_memory(db_path: &str, req: SaveMemoryRequest) -> Result<String, String> {
    if !req.id.starts_with("memory::") {
        return Err(
            "Memory ID must start with 'memory::' prefix (e.g., 'memory::stripe_webhooks')"
                .to_string(),
        );
    }

    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
    let graph = crate::open_db_graph(db_path).map_err(|e| format!("Failed to open DB graph: {e}"))?;

    let sqlite_conn = conn.sqlite_connection();

    let mut resolved_links = Vec::with_capacity(req.links.len());
    for target in &req.links {
        if node_exists(sqlite_conn, target) {
            resolved_links.push(target.clone());
        } else {
            let resolved = resolve_target_id(target, db_path);
            if node_exists(sqlite_conn, &resolved) {
                resolved_links.push(resolved);
            } else {
                return Err(format!(
                    "Link target not found: '{target}' (also tried resolving to '{resolved}'). Please make sure the code node has been scanned/indexed or the memory node exists."
                ));
            }
        }
    }

    let mut props = HashMap::new();
    props.insert("name".to_string(), req.name.clone());
    props.insert("description".to_string(), req.description.clone());
    let keywords_str = req.keywords.join(", ");
    props.insert("keywords".to_string(), keywords_str.clone());

    graph
        .upsert_node(&req.id, props, "Memory")
        .map_err(|e| format!("Failed to save memory node: {e}"))?;

    conn.cypher_builder("MATCH (m:Memory {id: $id})-[r]->() DELETE r")
        .param("id", req.id.as_str())
        .run()
        .map_err(|e| format!("Failed to clear old links: {e}"))?;

    for target_id in &resolved_links {
        let rel = req.link_type.as_deref().unwrap_or_else(|| {
            if target_id.starts_with("memory::") {
                "SUB_CONCEPT"
            } else {
                "EXPLAINS"
            }
        });
        graph
            .upsert_edge(&req.id, target_id, HashMap::<String, String>::new(), rel)
            .map_err(|e| format!("Failed to link {} to {}: {}", req.id, target_id, e))?;
    }

    sqlite_conn.execute(
        "INSERT OR REPLACE INTO memory_fts (id, name, description, keywords) VALUES (?, ?, ?, ?)",
        (&req.id, &req.name, &req.description, &keywords_str),
    ).map_err(|e| format!("Failed to update FTS index: {e}"))?;

    Ok(format!(
        "Memory node '{}' saved successfully with {} links.",
        req.id,
        resolved_links.len()
    ))
}

pub fn handle_get_memory(db_path: &str, req: GetMemoryRequest) -> Result<String, String> {
    if !req.id.starts_with("memory::") {
        return Err("Memory ID must start with 'memory::' prefix".to_string());
    }

    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;

    let query = "MATCH (m:Memory {id: $id}) RETURN m.name AS name, m.description AS description, m.keywords AS keywords";
    let res = conn
        .cypher_builder(query)
        .param("id", req.id.as_str())
        .run()
        .map_err(|e| format!("Failed to query memory: {e}"))?;

    if res.is_empty() {
        return Err(format!("Memory node '{}' not found.", req.id));
    }

    let row = &res[0];
    let name = row.get::<String>("name").unwrap_or_default();
    let description = row.get::<String>("description").unwrap_or_default();
    let keywords = row.get::<String>("keywords").unwrap_or_default();

    let links_query = "MATCH (m:Memory {id: $id})-[r]->(target) RETURN target.id AS target_id, target.name AS target_name, type(r) AS rel_type, labels(target) AS target_labels";
    let links_res = conn
        .cypher_builder(links_query)
        .param("id", req.id.as_str())
        .run()
        .map_err(|e| format!("Failed to query links: {e}"))?;

    let mut sub_concepts = Vec::new();
    let mut code_nodes = Vec::new();

    for l_row in &links_res {
        if let (Ok(t_id), Ok(rel_type)) = (
            l_row.get::<String>("target_id"),
            l_row.get::<String>("rel_type"),
        ) {
            let t_name = l_row.get::<String>("target_name").unwrap_or_default();
            let labels_str = l_row.get::<String>("target_labels").unwrap_or_default();
            let labels: Vec<String> = serde_json::from_str(&labels_str).unwrap_or_default();
            let kind = labels.first().map(|s| s.as_str()).unwrap_or("Code");

            let mut display_name = t_name;
            if display_name.is_empty() {
                display_name = t_id.clone();
            }

            if t_id.starts_with("memory::") {
                sub_concepts.push(format!(
                    "* [**{display_name}**]({t_id}) - Relationship: `{rel_type}`"
                ));
            } else {
                code_nodes.push(format!("* **{kind}** (`{display_name}`) [id: `{t_id}`] - Relationship: `{rel_type}`"));
            }
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
    let sqlite_conn = conn.sqlite_connection();

    let cleaned_query = req
        .query
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect::<String>();

    let mut stmt = sqlite_conn.prepare(
        "SELECT id, name, description, keywords, rank FROM memory_fts WHERE memory_fts MATCH ? ORDER BY rank LIMIT 10"
    ).map_err(|e| format!("Failed to prepare search statement: {e}"))?;

    let rows = stmt
        .query_map([&cleaned_query], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let description: String = row.get(2)?;
            let keywords: String = row.get(3)?;
            Ok((id, name, description, keywords))
        })
        .map_err(|e| format!("Search query execution failed: {e}"))?;

    let mut results = Vec::new();
    for r_res in rows {
        if let Ok((id, name, desc, keywords)) = r_res {
            let short_desc = if desc.len() > 150 {
                format!("{}...", &desc[..150].replace('\n', " "))
            } else {
                desc.replace('\n', " ")
            };
            results.push(format!("* [**{name}**]({id}) - `{id}`\n  * Description: {short_desc}\n  * Keywords: `{keywords}`"));
        }
    }

    if results.is_empty() {
        return Ok("No matching memory nodes found.".to_string());
    }

    Ok(format!(
        "# Search Results for: '{}'\n\n{}",
        req.query,
        results.join("\n\n")
    ))
}

pub fn handle_list_memories(db_path: &str, _req: ListMemoriesRequest) -> Result<String, String> {
    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;

    let query = "MATCH (m:Memory) RETURN m.id AS id, m.name AS name, m.keywords AS keywords ORDER BY m.name";
    let res = conn
        .cypher(query)
        .map_err(|e| format!("Failed to query memories list: {e}"))?;

    if res.is_empty() {
        return Ok("No memory nodes have been registered in the database yet. You can create one using the `save_memory` tool.".to_string());
    }

    let mut results = Vec::new();
    for row in &res {
        if let (Ok(id), Ok(name)) = (row.get::<String>("id"), row.get::<String>("name")) {
            let keywords = row.get::<String>("keywords").unwrap_or_default();
            results.push(format!(
                "* [**{name}**]({id}) - `{id}` (Keywords: `{keywords}`)"
            ));
        }
    }

    Ok(format!(
        "# Registered System Concepts & Memories\n\nUse the `get_memory` tool with the ID to retrieve a full architectural look-ahead map for any concept.\n\n{}",
        results.join("\n")
    ))
}
