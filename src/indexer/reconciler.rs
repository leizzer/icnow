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
    if crate::IS_INDEXING.load(std::sync::atomic::Ordering::SeqCst) {
        tracing::info!("Skipping import reconciliation because deep scan is currently running.");
        return Ok(());
    }
    tracing::info!("Starting background import reconciliation...");
    let conn = crate::open_db_connection(db_path).map_err(|e| anyhow::anyhow!(e))?;

    let query = "MATCH (f:File)-[r]->(i:Symbol) WHERE struct_extract(r, '_LABEL') = 'CONTAINS' AND i.kind = 'Import' RETURN f.id, i.id, i.name";

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
        let is_ts_js = f_id.ends_with(".ts")
            || f_id.ends_with(".tsx")
            || f_id.ends_with(".js")
            || f_id.ends_with(".jsx");
        let is_rust = f_id.ends_with(".rs");
        let is_ruby = f_id.ends_with(".rb");

        let is_local_path = source_path.starts_with('.')
            || source_path.starts_with('/')
            || source_path.starts_with("@/")
            || source_path.starts_with("~/");
        let is_rust_path = source_path.contains("::")
            || source_path.starts_with("crate")
            || source_path.starts_with("super");

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

                if kf.ends_with(&format!("{sep}{rust_path}.rs"))
                    || kf.ends_with(&format!("{sep}{rust_path}{sep}mod.rs"))
                {
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
                    if stem == clean_name && !clean_name.contains('/') && !clean_name.contains('\\')
                    {
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
        let edge_query = format!(
            "MATCH (s:File {{id: '{}'}}), (t:File {{id: '{}'}}) MERGE (s)-[:IMPORTS]->(t)",
            src.replace("'", "''"),
            target.replace("'", "''")
        );
        if conn.query(&edge_query).is_ok() {
            created_count += 1;
        }
    }

    let mut _sym_created_count = 0;
    for (src, sym_name, target_file) in symbol_edges_to_create {
        // Find the exported symbol in the target file and link the source file directly to it
        let edge_query = format!(
            "MATCH (s:File {{id: '{}'}}), (t:Symbol {{name: '{}', file: '{}'}}) MERGE (s)-[:IMPORTS]->(t)",
            src.replace("'", "''"),
            sym_name.replace("'", "''"),
            target_file.replace("'", "''")
        );
        if conn.query(&edge_query).is_ok() {
            _sym_created_count += 1;
        }

        // Advanced Import Resolution: Resolve floating function calls
        let resolve_calls_query_sym = format!(
            "MATCH (caller:Symbol)-[:CALLS]->(u:Symbol {{kind: 'unresolved_symbol', name: '{}', file: '{}'}}) MATCH (t:Symbol {{name: '{}', file: '{}'}}) \
             CREATE (caller)-[:CALLS]->(t) \
             DETACH DELETE u",
            sym_name.replace("'", "''"),
            src.replace("'", "''"),
            sym_name.replace("'", "''"),
            target_file.replace("'", "''")
        );
        let _ = conn.query(&resolve_calls_query_sym);
        let resolve_calls_query_file = format!(
            "MATCH (caller:File)-[:CALLS]->(u:Symbol {{kind: 'unresolved_symbol', name: '{}', file: '{}'}}) MATCH (t:Symbol {{name: '{}', file: '{}'}}) \
             CREATE (caller)-[:CALLS]->(t) \
             DETACH DELETE u",
            sym_name.replace("'", "''"),
            src.replace("'", "''"),
            sym_name.replace("'", "''"),
            target_file.replace("'", "''")
        );
        let _ = conn.query(&resolve_calls_query_file);

        // Advanced Import Resolution: Resolve floating instantiations
        let resolve_inst_query_sym = format!(
            "MATCH (caller:Symbol)-[:INSTANTIATES]->(u:Symbol {{kind: 'unresolved_symbol', name: '{}', file: '{}'}}) MATCH (t:Symbol {{name: '{}', file: '{}'}}) \
             CREATE (caller)-[:INSTANTIATES]->(t) \
             DETACH DELETE u",
            sym_name.replace("'", "''"),
            src.replace("'", "''"),
            sym_name.replace("'", "''"),
            target_file.replace("'", "''")
        );
        let _ = conn.query(&resolve_inst_query_sym);
        let resolve_inst_query_file = format!(
            "MATCH (caller:File)-[:INSTANTIATES]->(u:Symbol {{kind: 'unresolved_symbol', name: '{}', file: '{}'}}) MATCH (t:Symbol {{name: '{}', file: '{}'}}) \
             CREATE (caller)-[:INSTANTIATES]->(t) \
             DETACH DELETE u",
            sym_name.replace("'", "''"),
            src.replace("'", "''"),
            sym_name.replace("'", "''"),
            target_file.replace("'", "''")
        );
        let _ = conn.query(&resolve_inst_query_file);

        // Advanced Import Resolution: Resolve floating dependencies
        let resolve_depends_query_sym = format!(
            "MATCH (caller:Symbol)-[:DEPENDS_ON]->(u:Symbol {{kind: 'unresolved_symbol', name: '{}', file: '{}'}}) MATCH (t:Symbol {{name: '{}', file: '{}'}}) \
             CREATE (caller)-[:DEPENDS_ON]->(t) \
             DETACH DELETE u",
            sym_name.replace("'", "''"),
            src.replace("'", "''"),
            sym_name.replace("'", "''"),
            target_file.replace("'", "''")
        );
        let _ = conn.query(&resolve_depends_query_sym);
        let resolve_depends_query_file = format!(
            "MATCH (caller:File)-[:DEPENDS_ON]->(u:Symbol {{kind: 'unresolved_symbol', name: '{}', file: '{}'}}) MATCH (t:Symbol {{name: '{}', file: '{}'}}) \
             CREATE (caller)-[:DEPENDS_ON]->(t) \
             DETACH DELETE u",
            sym_name.replace("'", "''"),
            src.replace("'", "''"),
            sym_name.replace("'", "''"),
            target_file.replace("'", "''")
        );
        let _ = conn.query(&resolve_depends_query_file);

        // Advanced Import Resolution: Resolve floating imports
        let resolve_imports_query_sym = format!(
            "MATCH (caller:Symbol)-[:IMPORTS]->(u:Symbol {{kind: 'unresolved_symbol', name: '{}', file: '{}'}}) MATCH (t:Symbol {{name: '{}', file: '{}'}}) \
             CREATE (caller)-[:IMPORTS]->(t) \
             DETACH DELETE u",
            sym_name.replace("'", "''"),
            src.replace("'", "''"),
            sym_name.replace("'", "''"),
            target_file.replace("'", "''")
        );
        let _ = conn.query(&resolve_imports_query_sym);
        let resolve_imports_query_file = format!(
            "MATCH (caller:File)-[:IMPORTS]->(u:Symbol {{kind: 'unresolved_symbol', name: '{}', file: '{}'}}) MATCH (t:Symbol {{name: '{}', file: '{}'}}) \
             CREATE (caller)-[:IMPORTS]->(t) \
             DETACH DELETE u",
            sym_name.replace("'", "''"),
            src.replace("'", "''"),
            sym_name.replace("'", "''"),
            target_file.replace("'", "''")
        );
        let _ = conn.query(&resolve_imports_query_file);
    }

    for i_id in to_delete {
        let escaped = i_id.replace("'", "''");
        let _ = conn.query(&format!(
            "MATCH (n:Symbol {{id: '{escaped}'}}) DETACH DELETE n"
        ));
    }

    tracing::info!(
        "Import reconciliation complete. Created {} IMPORTS edges.",
        created_count
    );
    Ok(())
}


pub fn reconcile_unresolved_symbols(conn: &lbug::Connection) -> Result<()> {
    tracing::info!("Starting global resolution for unresolved symbols...");

    // Find all unresolved symbols
    let query = "MATCH (u:Symbol {kind: 'unresolved_symbol'}) RETURN u.id, u.name, u.file";
    let mut res = match conn.query(query) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to fetch unresolved symbols: {}", e);
            return Err(anyhow::anyhow!(e));
        }
    };

    let cols = res.get_column_names();
    let mut unresolved_rows = Vec::new();
    for row in res.by_ref() {
        if let (Some(u_id), Some(u_name), Some(u_file)) = (
            crate::indexer::reconciler::get_str_val(&row, &cols, "u.id"),
            crate::indexer::reconciler::get_str_val(&row, &cols, "u.name"),
            crate::indexer::reconciler::get_str_val(&row, &cols, "u.file"),
        ) {
            unresolved_rows.push((u_id, u_name, u_file));
        }
    }

    if unresolved_rows.is_empty() {
        return Ok(());
    }

    // Map symbol names to all known definition nodes
    let mut symbol_defs = std::collections::HashMap::new();
    if let Ok(mut defs_res) = conn.query("MATCH (s:Symbol) WHERE s.kind <> 'unresolved_symbol' RETURN s.id, s.name, s.file") {
        let def_cols = defs_res.get_column_names();
        for row in defs_res.by_ref() {
            if let (Some(s_id), Some(s_name), Some(s_file)) = (
                crate::indexer::reconciler::get_str_val(&row, &def_cols, "s.id"),
                crate::indexer::reconciler::get_str_val(&row, &def_cols, "s.name"),
                crate::indexer::reconciler::get_str_val(&row, &def_cols, "s.file"),
            ) {
                // Ignore method names that contain '::' or '.' as the exact match usually works on the base name
                let base_name = if s_name.contains("::") {
                    s_name.split("::").last().unwrap_or(&s_name).to_string()
                } else if s_name.contains('.') {
                    s_name.split('.').last().unwrap_or(&s_name).to_string()
                } else {
                    s_name.clone()
                };
                symbol_defs.entry(base_name).or_insert_with(Vec::new).push((s_id, s_file));
            }
        }
    }

    let mut resolved_count = 0;
    for (u_id, u_name, u_file) in unresolved_rows {
        if let Some(matches) = symbol_defs.get(&u_name) {
            let mut best_match = None;
            
            // Prefer matches in the exact same file
            for (s_id, s_file) in matches {
                if *s_file == u_file {
                    best_match = Some(s_id.clone());
                    break;
                }
            }
            
            // Or prefer single unique match globally
            if best_match.is_none() && matches.len() == 1 {
                best_match = Some(matches[0].0.clone());
            }

            if let Some(target_id) = best_match {
                // Re-wire CALLS, INSTANTIATES, DEPENDS_ON, INHERITS, IMPORTS edges
                for edge_type in &["CALLS", "INSTANTIATES", "DEPENDS_ON", "INHERITS", "IMPORTS"] {
                    let rewire_query_sym = format!(
                        "MATCH (caller:Symbol)-[:{edge_type}]->(u:Symbol {{id: '{}'}}) MATCH (t:Symbol {{id: '{}'}}) CREATE (caller)-[:{edge_type}]->(t)",
                        u_id.replace("'", "''"),
                        target_id.replace("'", "''")
                    );
                    let _ = conn.query(&rewire_query_sym);
                    let rewire_query_file = format!(
                        "MATCH (caller:File)-[:{edge_type}]->(u:Symbol {{id: '{}'}}) MATCH (t:Symbol {{id: '{}'}}) CREATE (caller)-[:{edge_type}]->(t)",
                        u_id.replace("'", "''"),
                        target_id.replace("'", "''")
                    );
                    let _ = conn.query(&rewire_query_file);
                }
                
                // Delete the unresolved node
                let delete_query = format!("MATCH (u:Symbol {{id: '{}'}}) DETACH DELETE u", u_id.replace("'", "''"));
                let _ = conn.query(&delete_query);
                
                resolved_count += 1;
            }
        }
    }

    tracing::info!("Global unresolved symbol resolution complete. Resolved {} symbols.", resolved_count);
    Ok(())
}
