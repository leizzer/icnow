use anyhow::Result;
use std::collections::HashMap;
use lbug::Value;

fn get_str_val(row: &[Value], cols: &[String], name: &str) -> Option<String> {
    cols.iter().position(|c| c == name).and_then(|idx| {
        if let Value::String(s) = &row[idx] {
            Some(s.clone())
        } else {
            None
        }
    })
}

pub fn reconcile_imports(db_path: &str) -> Result<()> {
    tracing::info!("Starting background import reconciliation...");
    let conn = crate::open_db_connection(db_path)
        .map_err(|e| anyhow::anyhow!(e))?;

    let query = "MATCH (f:File)-[r]->(i:Symbol) WHERE type(r) = 'REL_CONTAINS' AND i.kind = 'Import' RETURN f.id, i.id, i.name";

    let mut res = match conn.query(query) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Reconciler query failed: {}", e);
            return Err(anyhow::anyhow!(e));
        }
    };
    
    let cols = res.get_column_names();
    let mut import_rows = Vec::new();
    for row in res.by_ref() {
        if let (Some(f_id), Some(i_id), Some(i_name)) = (
            get_str_val(&row, &cols, "f.id"),
            get_str_val(&row, &cols, "i.id"),
            get_str_val(&row, &cols, "i.name"),
        ) {
            import_rows.push((f_id, i_id, i_name));
        }
    }

    let mut known_files = Vec::new();
    if let Ok(mut file_res) = conn.query("MATCH (f:File) RETURN f.id") {
        let file_cols = file_res.get_column_names();
        for row in file_res.by_ref() {
            if let Some(f_id) = get_str_val(&row, &file_cols, "f.id") {
                known_files.push(f_id);
            }
        }
    }

    let mut to_delete = Vec::new();
    let mut edges_to_create = Vec::new();

    for (f_id, i_id, i_name) in import_rows {
        let clean_name = i_name.trim_start_matches("./").trim_start_matches("../");
        let normalized_clean = if std::path::MAIN_SEPARATOR == '\\' {
            clean_name.replace('/', "\\")
        } else {
            clean_name.to_string()
        };

        let mut resolved_target = None;
        for kf in &known_files {
            let sep = std::path::MAIN_SEPARATOR;
            if kf.ends_with(&format!("{sep}{normalized_clean}.rb"))
                || kf.ends_with(&format!("{sep}{normalized_clean}.ts"))
                || kf.ends_with(&format!("{sep}{normalized_clean}.tsx"))
                || kf.ends_with(&format!("{sep}{normalized_clean}.rs"))
            {
                resolved_target = Some(kf.clone());
                break;
            }

            let path = std::path::Path::new(kf);
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if stem == clean_name && !clean_name.contains('/') && !clean_name.contains('\\')
                {
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

    let mut created_count = 0;
    for (src, target) in edges_to_create {
        let edge_query = format!("MATCH (s:File {{id: '{}'}}), (t:File {{id: '{}'}}) MERGE (s)-[:IMPORTS]->(t)", src.replace("'", "''"), target.replace("'", "''"));
        if conn.query(&edge_query).is_ok() {
            created_count += 1;
        }
    }

    for i_id in to_delete {
        let escaped = i_id.replace("'", "''");
        let _ = conn.query(&format!("MATCH (n:Symbol {{id: '{}'}}) DETACH DELETE n", escaped));
    }

    tracing::info!("Import reconciliation complete. Created {} IMPORTS edges.", created_count);
    Ok(())
}
