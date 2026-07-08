use crate::tools::{
    CoverageCheckRequest, GetFileStructureRequest, GetSymbolImplementationRequest,
    GetSymbolInfoRequest, ListIndexedFilesRequest, QueryGraphCypherRequest, SearchSymbolsRequest,
};

pub fn handle_query_graph_cypher(
    db_path: &str,
    req: QueryGraphCypherRequest,
) -> Result<String, String> {
    if crate::IS_INDEXING.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(
            "Database is currently indexing. Please wait a few seconds and try again.".to_string(),
        );
    }
    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
    let mut res = conn
        .query(&req.query)
        .map_err(|e| format!("Cypher query failed: {e}"))?;
    crate::tools::format_cypher_result(&mut res)
}

pub fn handle_search_symbols(db_path: &str, req: SearchSymbolsRequest) -> Result<String, String> {
    if crate::IS_INDEXING.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(
            "Database is currently indexing. Please wait a few seconds and try again.".to_string(),
        );
    }

    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;

    let query_param = req.query.replace("'", "''");

    let mut q = format!(
        "MATCH (n:Symbol) WHERE n.kind <> 'unresolved_symbol' AND (n.name CONTAINS '{query_param}' OR (n.id CONTAINS '{query_param}' AND ('{query_param}' CONTAINS '/' OR '{query_param}' CONTAINS '::')))"
    );

    if let Some(filters) = &req.kind_filter {
        if !filters.is_empty() {
            let filter_placeholders: Vec<String> = filters
                .iter()
                .map(|f| format!("'{}'", f.replace("'", "''")))
                .collect();
            q.push_str(&format!(
                " AND (n.kind IN [{f}] OR label(n) IN [{f}])",
                f = filter_placeholders.join(", ")
            ));
        }
    }

    let limit_clause = if let Some(limit) = req.limit {
        format!(" LIMIT {limit}")
    } else {
        String::new()
    };

    q.push_str(&format!(" RETURN DISTINCT n.id AS id, n.kind AS label, n.signature AS signature, n.docstring AS docstring{limit_clause}"));

    let mut res = conn
        .query(&q)
        .map_err(|e| format!("Failed to search symbols: {e}"))?;

    let cols = res.get_column_names();
    if cols.is_empty() {
        return Ok("No columns returned.".to_string());
    }

    let mut out = format!("| {} |\n", cols.join(" | "));
    out.push_str(&format!(
        "| {} |\n",
        cols.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
    ));

    let mut seen = std::collections::HashSet::new();
    let mut row_count = 0;

    for row in res.by_ref() {
        let mut row_vals = Vec::new();
        for (i, _col) in cols.iter().enumerate() {
            let val_str = match &row[i] {
                lbug::Value::String(s) => s.clone(),
                lbug::Value::Int64(i) => i.to_string(),
                lbug::Value::Int32(i) => i.to_string(),
                lbug::Value::Double(f) => f.to_string(),
                lbug::Value::Bool(b) => b.to_string(),
                lbug::Value::Null(_) => "null".to_string(),
                _ => "?".to_string(),
            };
            row_vals.push(val_str);
        }

        // Deduplicate based on file_path (from id), label, and signature
        let id = &row_vals[0];
        let file_path = id.split("::").next().unwrap_or(id);
        let label = &row_vals[1];
        let signature = &row_vals[2];
        let dedup_key = format!("{}|{}|{}", file_path, label, signature);

        if seen.insert(dedup_key) {
            out.push_str(&format!("| {} |\n", row_vals.join(" | ")));
            row_count += 1;
        }
    }

    if row_count == 0 {
        return Ok(format!(
            "{out}\nNo matches found. Hint: The target files might not be indexed yet or might be stale. Use the 'coverage_check' tool to verify if the relevant directories are in the graph."
        ));
    }
    Ok(out)
}

pub fn handle_get_file_structure(
    db_path: &str,
    req: GetFileStructureRequest,
) -> Result<String, String> {
    if crate::IS_INDEXING.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(
            "Database is currently indexing. Please wait a few seconds and try again.".to_string(),
        );
    }
    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
    let q = format!(
        "MATCH (f:File {{id: '{}'}})-[:CONTAINS]->(s:Symbol) RETURN s.id AS id, s.kind AS label, s.signature AS signature",
        req.file_path.replace("'", "''")
    );
    let res = conn
        .query(&q)
        .map_err(|e| format!("Failed to query file structure: {e}"))?;
    let mut out = String::new();
    for row in res {
        let id = row[0].to_string();
        let kind = row[1].to_string();
        let sig = row[2].to_string();
        out.push_str(&format!(
            "- [{}] {} `{}`\n",
            kind,
            id,
            sig.replace("\n", " ")
        ));
    }
    if out.is_empty() {
        return Ok("No symbols found in this file.".to_string());
    }
    Ok(out)
}

pub fn handle_list_indexed_files(
    db_path: &str,
    _req: ListIndexedFilesRequest,
) -> Result<String, String> {
    if crate::IS_INDEXING.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(
            "Database is currently indexing. Please wait a few seconds and try again.".to_string(),
        );
    }
    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
    let mut res = conn
        .query("MATCH (f:File) RETURN f.id AS id ORDER BY f.id")
        .map_err(|e| format!("Failed to list files: {e}"))?;
    crate::tools::format_cypher_result(&mut res)
}

pub fn handle_get_symbol_implementation(
    db_path: &str,
    req: GetSymbolImplementationRequest,
) -> Result<String, String> {
    if crate::IS_INDEXING.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(
            "Database is currently indexing. Please wait a few seconds and try again.".to_string(),
        );
    }
    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
    let q = format!(
        "MATCH (s:Symbol {{id: '{}'}}) RETURN s.file, s.start_line, s.end_line",
        req.node_id.replace("'", "''")
    );
    let mut res = conn
        .query(&q)
        .map_err(|e| format!("Failed to get impl: {e}"))?;

    if let Some(row) = res.by_ref().next() {
        let file_path = match &row[0] {
            lbug::Value::String(s) => s.clone(),
            _ => return Err("Invalid file path".to_string()),
        };
        let start_line = match &row[1] {
            lbug::Value::Int64(i) => *i as usize,
            lbug::Value::Int32(i) => *i as usize,
            _ => 0, // Fallback if old schema
        };
        let end_line = match &row[2] {
            lbug::Value::Int64(i) => *i as usize,
            lbug::Value::Int32(i) => *i as usize,
            _ => 0, // Fallback if old schema
        };

        if start_line == 0 || end_line == 0 {
            return Err(
                "No line pointers found. Database might be using the old schema.".to_string(),
            );
        }

        let file_contents = std::fs::read_to_string(&file_path)
            .map_err(|e| format!("Failed to read file {file_path}: {e}"))?;
        let lines: Vec<&str> = file_contents.lines().collect();

        let pad_start = start_line.saturating_sub(2).max(1);
        let pad_end = (end_line + 2).min(lines.len());

        if pad_start > lines.len() {
            return Err("Start line out of bounds".to_string());
        }

        let snippet = lines[pad_start - 1..pad_end].join("\n");
        return Ok(snippet);
    }

    Err(format!("No implementation found for {}", req.node_id))
}

pub fn handle_get_symbol_info(db_path: &str, req: GetSymbolInfoRequest) -> Result<String, String> {
    if crate::IS_INDEXING.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(
            "Database is currently indexing. Please wait a few seconds and try again.".to_string(),
        );
    }
    if req.node_id.starts_with('/') && !req.node_id.contains("::") {
        return Err(format!(
            "Error: '{}' is a File ID, not a Symbol ID. To view the contents or structural outline of this file, use 'get_file_structure' instead.",
            req.node_id
        ));
    }

    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
    let node_id = crate::models::escape_cypher_string(&req.node_id);

    let mut output = String::new();

    let mut symbol_name = String::new();

    // 1. Get properties
    let q_props = format!(
        "MATCH (s:Symbol {{id: '{node_id}'}}) RETURN s.kind AS kind, s.signature AS signature, s.docstring AS docstring, s.name AS name"
    );
    match conn.query(&q_props) {
        Ok(mut res) => {
            if let Some(row) = res.next() {
                output.push_str(&format!("## Symbol: {node_id}\n"));
                output.push_str(&format!("**Kind**: {}\n", row[0]));
                if let lbug::Value::String(sig) = &row[1] {
                    if !sig.is_empty() {
                        output.push_str(&format!("**Signature**: `{sig}`\n"));
                    }
                }
                if let lbug::Value::String(doc) = &row[2] {
                    if !doc.is_empty() {
                        output.push_str(&format!("**Docstring**: {doc}\n"));
                    }
                }
                if let lbug::Value::String(name_val) = &row[3] {
                    symbol_name = name_val.clone();
                }
                output.push('\n');
            } else {
                return Err(format!("Symbol not found: {}", req.node_id));
            }
        }
        Err(e) => return Err(format!("Failed to query properties: {e}")),
    }

    // 2. Get container
    let q_parent = format!(
        "MATCH (parent)-[:CONTAINS]->(s:Symbol {{id: '{node_id}'}}) RETURN label(parent) AS parent_label, parent.id AS parent_id"
    );
    if let Ok(mut res) = conn.query(&q_parent) {
        if let Some(row) = res.next() {
            output.push_str(&format!("**Container**: {} (`{}`)\n\n", row[0], row[1]));
        }
    }

    // 3. Get outgoing dependencies (CALLS or IMPORTS or INHERITS)
    let q_out = format!(
        "MATCH (s:Symbol {{id: '{node_id}'}})-[r:CALLS|:IMPORTS|:INHERITS]->(dep:Symbol) RETURN label(r) AS rel_type, dep.id AS target_id"
    );
    if let Ok(res) = conn.query(&q_out) {
        let mut deps = Vec::new();
        for row in res {
            deps.push(format!("- [{}] -> `{}`", row[0], row[1]));
        }
        if !deps.is_empty() {
            output.push_str("### Outgoing Dependencies (What this symbol calls/imports)\n");
            output.push_str(&deps.join("\n"));
            output.push_str("\n\n");
        }
    }

    // 4. Get incoming usages
    let base_name = symbol_name.split("::").last().unwrap_or(&symbol_name);
    let dot_base = format!(".{base_name}");
    let colon_base = format!("::{base_name}");

    let q_in = format!(
        "MATCH (caller:Symbol)-[r:CALLS|:IMPORTS|:INHERITS]->(t:Symbol) 
         WHERE t.id = '{node_id}' 
         OR (t.kind = 'unresolved_symbol' AND label(r) = 'CALLS' AND (t.name = '{base_name}' OR t.name ENDS WITH '{dot_base}' OR t.name ENDS WITH '{colon_base}')) 
         RETURN label(r) AS rel_type, caller.id AS caller_id, t.file AS file, t.line AS line LIMIT 100"
    );
    if let Ok(res) = conn.query(&q_in) {
        let mut usages = Vec::new();
        for row in res {
            let rel_type = row[0].to_string();
            let caller_id = row[1].to_string();
            let file = if let lbug::Value::String(f) = &row[2] {
                f.clone()
            } else {
                String::new()
            };
            let line = if let lbug::Value::String(l) = &row[3] {
                l.clone()
            } else {
                String::new()
            };

            if !file.is_empty() && !line.is_empty() {
                let mut snippet_str = String::new();
                if let Ok(line_num) = line.parse::<usize>() {
                    if let Some(snippet) = extract_snippet(&file, line_num, 2) {
                        snippet_str = format!("\n```\n{snippet}\n```");
                    }
                }
                usages.push(format!(
                    "- `{caller_id}` -> [{rel_type}] at {file}:{line}{snippet_str}"
                ));
            } else {
                usages.push(format!("- `{caller_id}` -> [{rel_type}]"));
            }
        }
        if !usages.is_empty() {
            output.push_str("### Incoming Usages (What calls/imports this symbol)\n");
            output.push_str(&usages.join("\n"));
            output.push_str("\n\n");
        }
    }

    // 5. Get children (CONTAINS or DEFINES)
    let q_children = format!(
        "MATCH (s:Symbol {{id: '{node_id}'}})-[r:CONTAINS|:DEFINES]->(child:Symbol) RETURN label(r) AS rel_type, child.id AS child_id, child.kind AS child_kind"
    );
    if let Ok(res) = conn.query(&q_children) {
        let mut children = Vec::new();
        for row in res {
            let _rel = row[0].to_string();
            let c_id = row[1].to_string();
            let c_kind = row[2].to_string();
            children.push(format!("- `{c_id}` ({c_kind})"));
        }
        if !children.is_empty() {
            output.push_str(&format!("### Contains ({} items)\n", children.len()));
            output.push_str(&children.join("\n"));
            output.push_str("\n\n");
        }
    }

    Ok(output.trim().to_string())
}

fn extract_snippet(file_path: &str, line_num: usize, context_lines: usize) -> Option<String> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let file = File::open(file_path).ok()?;
    let reader = BufReader::new(file);
    let mut snippet = String::new();

    let start_line = line_num.saturating_sub(context_lines).max(1);
    let end_line = line_num + context_lines;

    for (i, line_res) in reader.lines().enumerate() {
        let current_line = i + 1;
        if current_line > end_line {
            break;
        }
        if current_line >= start_line {
            if let Ok(line) = line_res {
                snippet.push_str(&format!("{current_line:4} | {line}\n"));
            }
        }
    }

    if snippet.is_empty() {
        None
    } else {
        Some(snippet.trim_end().to_string())
    }
}

fn scan_directory_recursively(dir: &std::path::Path, files: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name != ".git" && name != "node_modules" && name != "target" && name != "vendor"
                {
                    scan_directory_recursively(&path, files);
                }
            } else if let Some(ext) = path.extension() {
                if ext == "rb" || ext == "rs" || ext == "ts" || ext == "tsx" {
                    files.push(
                        path.canonicalize()
                            .unwrap_or(path)
                            .to_string_lossy()
                            .to_string(),
                    );
                }
            }
        }
    }
}

pub fn handle_coverage_check(db_path: &str, req: CoverageCheckRequest) -> Result<String, String> {
    if crate::IS_INDEXING.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(
            "Database is currently indexing. Please wait a few seconds and try again.".to_string(),
        );
    }
    let conn = crate::open_db_connection(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;

    let dir_path = std::path::Path::new(&req.directory_path);
    if !dir_path.exists() || !dir_path.is_dir() {
        return Err(format!(
            "Directory '{}' does not exist or is not a directory.",
            req.directory_path
        ));
    }

    let mut disk_files = Vec::new();
    scan_directory_recursively(dir_path, &mut disk_files);

    let mut indexed_count = 0;
    let mut missing_files = Vec::new();
    let mut stale_files = Vec::new();

    for file_path in &disk_files {
        let q = format!(
            "MATCH (f:File {{id: '{}'}}) RETURN f.last_modified",
            crate::models::escape_cypher_string(file_path)
        );
        match conn.query(&q) {
            Ok(mut res) => {
                if let Some(row) = res.by_ref().next() {
                    indexed_count += 1;
                    if let lbug::Value::Int64(last_mod) = row[0] {
                        if let Ok(meta) = std::fs::metadata(file_path) {
                            if let Ok(modified) = meta.modified() {
                                if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH)
                                {
                                    if duration.as_secs() as i64 > last_mod {
                                        stale_files.push(file_path.clone());
                                    }
                                }
                            }
                        }
                    }
                } else {
                    missing_files.push(file_path.clone());
                }
            }
            Err(_) => missing_files.push(file_path.clone()),
        }
    }

    let mut output = format!("## Coverage Report for `{}`\n", req.directory_path);
    output.push_str(&format!(
        "- **Total Source Files on Disk**: {}\n",
        disk_files.len()
    ));
    output.push_str(&format!("- **Indexed**: {indexed_count}\n"));
    output.push_str(&format!("- **Missing**: {}\n", missing_files.len()));
    output.push_str(&format!(
        "- **Stale (Needs Re-index)**: {}\n\n",
        stale_files.len()
    ));

    if !missing_files.is_empty() {
        output.push_str("### Sample Missing Files\n");
        for f in missing_files.iter().take(10) {
            output.push_str(&format!("- `{f}`\n"));
        }
        if missing_files.len() > 10 {
            output.push_str(&format!("- ... and {} more\n", missing_files.len() - 10));
        }
        output.push('\n');
    }

    if !stale_files.is_empty() {
        output.push_str("### Sample Stale Files\n");
        for f in stale_files.iter().take(10) {
            output.push_str(&format!("- `{f}`\n"));
        }
        if stale_files.len() > 10 {
            output.push_str(&format!("- ... and {} more\n", stale_files.len() - 10));
        }
    }

    if missing_files.is_empty() && stale_files.is_empty() {
        output.push_str("✅ **All files are fully indexed and up-to-date!**\n");
    } else {
        output.push_str("\n> **Action Required**: You have missing or stale files!\n");
        output.push_str("> 1. **Quick/One-off queries:** It may be faster to just use traditional tools (like `grep` or `read_file`) on these un-indexed files instead of paying the indexing cost.\n");
        output.push_str("> 2. **Structural graph queries:** If you need to trace callers or find subclasses, use the `parse_project_file` tool on these specific missing files. This forces `icnow` to bypass external LSIF indexers and use its internal 100% accurate Tree-sitter parser to add them to the graph!");
    }

    Ok(output)
}
