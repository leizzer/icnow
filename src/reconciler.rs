use anyhow::Result;
use graphqlite::{Connection, Graph};
use std::collections::HashMap;

pub fn reconcile_imports(db_path: &str) -> Result<()> {
    tracing::info!("Starting background import reconciliation...");
    let conn = crate::open_db_connection(db_path)?;
    let graph = crate::open_db_graph(db_path)?;
    
    // Fetch all Import nodes that have a CONTAINS edge from a File
    let query = "MATCH (f:File)-[r]->(i:Import) WHERE type(r) = 'REL_CONTAINS' RETURN f.id, i.id, i.name";
    
    let res = match conn.cypher(query) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Reconciler query failed: {}", e);
            return Err(anyhow::anyhow!(e));
        }
    };
    
    // Cache all known files to avoid per-import queries
    let mut known_files = Vec::new();
    if let Ok(file_res) = conn.cypher("MATCH (f:File) RETURN f.id") {
        for row in &file_res {
            if let Ok(f_id) = row.get::<String>("f.id") {
                known_files.push(f_id);
            }
        }
    }
    
    let mut to_delete = Vec::new();
    let mut edges_to_create = Vec::new();
    
    for row in &res {
        if let (Ok(f_id), Ok(i_id), Ok(i_name)) = (
            row.get::<String>("f.id"), 
            row.get::<String>("i.id"), 
            row.get::<String>("i.name")
        ) {
            let clean_name = i_name.trim_start_matches("./").trim_start_matches("../");
            
            let normalized_clean = if std::path::MAIN_SEPARATOR == '\\' {
                clean_name.replace('/', "\\")
            } else {
                clean_name.to_string()
            };
            
            let mut resolved_target = None;
            for kf in &known_files {
                // Exact path match
                let sep = std::path::MAIN_SEPARATOR;
                if kf.ends_with(&format!("{}{}.rb", sep, normalized_clean)) ||
                   kf.ends_with(&format!("{}{}.ts", sep, normalized_clean)) ||
                   kf.ends_with(&format!("{}{}.tsx", sep, normalized_clean)) ||
                   kf.ends_with(&format!("{}{}.rs", sep, normalized_clean)) {
                   resolved_target = Some(kf.clone());
                   break;
                }
                
                // Fallback: file stem match (if import is just "user_model")
                let path = std::path::Path::new(kf);
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if stem == clean_name && !clean_name.contains('/') && !clean_name.contains('\\') {
                        resolved_target = Some(kf.clone());
                        break;
                    }
                }
            }
            
            if let Some(target) = resolved_target {
                edges_to_create.push((f_id.clone(), target));
                to_delete.push(i_id.clone());
            }
        }
    }
    
    let mut created_count = 0;
    for (src, target) in edges_to_create {
        let edge_id = format!("{}::IMPORTS::{}", src, target);
        let edge = crate::models::Edge {
            id: edge_id,
            source: src,
            target: target,
            label: "IMPORTS".to_string(),
            properties: HashMap::new(),
        };
        if edge.save(&graph).is_ok() {
            created_count += 1;
        }
    }
    
    for i_id in to_delete {
        let escaped = i_id.replace("'", "''");
        let _ = conn.cypher(&format!("MATCH (n) WHERE n.id = '{}' DETACH DELETE n", escaped));
    }
    
    tracing::info!("Import reconciliation complete. Created {} IMPORTS edges.", created_count);
    Ok(())
}
