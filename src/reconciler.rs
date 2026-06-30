use anyhow::Result;
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

    let query = "MATCH (f:File)-[r]->(i:Symbol) WHERE struct_extract(r, '_LABEL') = 'REL_CONTAINS' AND i.kind = 'Import' RETURN f.id, i.id, i.name";

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
    let mut symbol_edges_to_create = Vec::new();

    for (f_id, i_id, i_name) in import_rows {
        let mut symbol_name = None;
        let mut source_path = i_name.clone();
        if let Some((sym, src)) = i_name.split_once(" FROM '") {
            symbol_name = Some(sym.to_string());
            source_path = src.trim_end_matches('\'').to_string();
        }

        let mut resolved_target = None;
        let is_ts_js = f_id.ends_with(".ts") || f_id.ends_with(".tsx") || f_id.ends_with(".js") || f_id.ends_with(".jsx");
        let is_rust = f_id.ends_with(".rs");
        let is_ruby = f_id.ends_with(".rb");

        let is_local_path = source_path.starts_with('.') || source_path.starts_with('/') || source_path.starts_with("@/") || source_path.starts_with("~/");
        let is_rust_path = source_path.contains("::") || source_path.starts_with("crate") || source_path.starts_with("super");

        if is_ts_js && !is_local_path {
            // It's likely a node module (e.g. 'react', 'lodash'). Skip to avoid false positive links to local files.
            continue;
        }

        let clean_name = source_path
            .trim_start_matches("./")
            .trim_start_matches("../")
            .trim_start_matches("@/")
            .trim_start_matches("~/");

        let normalized_clean = if std::path::MAIN_SEPARATOR == '\\' {
            clean_name.replace('/', "\\")
        } else {
            clean_name.to_string()
        };

        for kf in &known_files {
            let sep = std::path::MAIN_SEPARATOR;
            
            if is_rust && is_rust_path {
                let rust_path = normalized_clean
                    .replace("crate::", "")
                    .replace("super::", "")
                    .replace("::", &sep.to_string());
                
                if kf.ends_with(&format!("{sep}{rust_path}.rs")) || kf.ends_with(&format!("{sep}{rust_path}{sep}mod.rs")) {
                    resolved_target = Some(kf.clone());
                    break;
                }
            }

            let is_match = kf.ends_with(&format!("{sep}{normalized_clean}.rb"))
                || kf.ends_with(&format!("{sep}{normalized_clean}.ts"))
                || kf.ends_with(&format!("{sep}{normalized_clean}.tsx"))
                || kf.ends_with(&format!("{sep}{normalized_clean}.js"))
                || kf.ends_with(&format!("{sep}{normalized_clean}.jsx"))
                || kf.ends_with(&format!("{sep}{normalized_clean}.rs"))
                || kf.ends_with(&format!("{sep}{normalized_clean}{sep}index.ts"))
                || kf.ends_with(&format!("{sep}{normalized_clean}{sep}index.tsx"))
                || kf.ends_with(&format!("{sep}{normalized_clean}{sep}index.js"))
                || kf.ends_with(&format!("{sep}{normalized_clean}{sep}index.jsx"));

            if is_match {
                resolved_target = Some(kf.clone());
                break;
            }

            if is_ruby {
                let path = std::path::Path::new(kf);
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if stem == clean_name && !clean_name.contains('/') && !clean_name.contains('\\') {
                        resolved_target = Some(kf.clone());
                        break;
                    }
                }
            }
        }

        if let Some(target) = resolved_target {
            edges_to_create.push((f_id.clone(), target.clone()));
            if let Some(sym) = symbol_name {
                symbol_edges_to_create.push((f_id.clone(), sym, target));
            }
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

    let mut sym_created_count = 0;
    for (src, sym_name, target_file) in symbol_edges_to_create {
        // Find the exported symbol in the target file and link the source file directly to it
        let edge_query = format!("MATCH (s:File {{id: '{}'}}), (t:Symbol {{name: '{}', file: '{}'}}) MERGE (s)-[:IMPORTS]->(t)", 
            src.replace("'", "''"), 
            sym_name.replace("'", "''"), 
            target_file.replace("'", "''")
        );
        if conn.query(&edge_query).is_ok() {
            sym_created_count += 1;
        }

        // Advanced Import Resolution: Resolve floating function calls
        // If the source file has an Unresolved call with the same name, link the caller directly to the imported symbol
        let resolve_calls_query = format!(
            "MATCH (caller)-[:CALLS]->(u:Symbol {{kind: 'unresolved_symbol', name: '{}', file: '{}'}}), (t:Symbol {{name: '{}', file: '{}'}}) \
             MERGE (caller)-[:CALLS]->(t) \
             DETACH DELETE u",
            sym_name.replace("'", "''"),
            src.replace("'", "''"),
            sym_name.replace("'", "''"),
            target_file.replace("'", "''")
        );
        let _ = conn.query(&resolve_calls_query);
    }

    for i_id in to_delete {
        let escaped = i_id.replace("'", "''");
        let _ = conn.query(&format!("MATCH (n:Symbol {{id: '{}'}}) DETACH DELETE n", escaped));
    }

    tracing::info!("Import reconciliation complete. Created {} IMPORTS edges.", created_count);
    Ok(())
}
