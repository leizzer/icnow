use anyhow::Result;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct LsifPosition {
    line: usize,
    character: usize,
}

#[derive(Debug, Deserialize)]
struct LsifLine {
    id: i64,
    #[serde(rename = "type")]
    type_field: String,
    label: String,
    // Vertex fields
    uri: Option<String>,
    start: Option<LsifPosition>,
    end: Option<LsifPosition>,
    identifier: Option<String>,
    scheme: Option<String>,
    kind: Option<String>,
    // Edge fields
    #[serde(rename = "outV")]
    out_v: Option<i64>,
    #[serde(rename = "inV")]
    in_v: Option<i64>,
    #[serde(rename = "inVs")]
    in_vs: Option<Vec<i64>>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
struct LsifRange {
    id: i64,
    start_line: usize,
    start_char: usize,
    end_line: usize,
    end_char: usize,
    document_id: Option<i64>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
struct LsifMoniker {
    identifier: String,
    scheme: String,
    kind: String,
}

pub fn auto_generate_lsif(project_root: &str) -> Result<String> {
    let root = Path::new(project_root);

    // 1. Detect Rust
    if root.join("Cargo.toml").exists() {
        tracing::info!("Detected Rust project. Running `rust-analyzer lsif`...");
        let output = std::process::Command::new("rust-analyzer")
            .arg("lsif")
            .arg(project_root)
            .current_dir(project_root)
            .output();

        let output = match output {
            Ok(o) => o,
            Err(_) => {
                return Err(anyhow::anyhow!(
                    "rust-analyzer is not installed or not in PATH. Please install it by running:\n\
                     rustup component add rust-analyzer"
                ));
            }
        };

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("rust-analyzer lsif failed: {err}"));
        }

        let temp_file_path = std::env::temp_dir().join("icnow_rust_lsif.lsif");
        std::fs::write(&temp_file_path, output.stdout)?;
        return Ok(temp_file_path.to_string_lossy().to_string());
    }

    // 2. Detect TypeScript/React
    if root.join("tsconfig.json").exists() {
        tracing::info!(
            "Detected TypeScript/React project. Running `npx -y @sourcegraph/lsif-tsc`..."
        );
        let output = std::process::Command::new("npx")
            .args(["-y", "@sourcegraph/lsif-tsc", "-p", "tsconfig.json"])
            .current_dir(project_root)
            .output();

        let output = match output {
            Ok(o) => o,
            Err(_) => {
                return Err(anyhow::anyhow!(
                    "npx/NodeJS is not installed or not in PATH. Please install NodeJS and npm."
                ));
            }
        };

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("npx @sourcegraph/lsif-tsc failed: {err}"));
        }

        // npx @sourcegraph/lsif-tsc generates `dump.lsif` in the project root
        let dump_path = root.join("dump.lsif");
        if dump_path.exists() {
            return Ok(dump_path.to_string_lossy().to_string());
        } else {
            return Err(anyhow::anyhow!(
                "npx @sourcegraph/lsif-tsc completed successfully but did not generate a dump.lsif file."
            ));
        }
    }

    // 3. Detect Ruby
    if root.join("Gemfile").exists() || root.join("Rakefile").exists() {
        tracing::info!("Detected Ruby project. Running `solargraph lsif`...");
        let output = std::process::Command::new("solargraph")
            .arg("lsif")
            .current_dir(project_root)
            .output();

        let output = match output {
            Ok(o) => o,
            Err(_) => {
                return Err(anyhow::anyhow!(
                    "solargraph is not installed or not in PATH. Please install it by running:\n\
                     gem install solargraph"
                ));
            }
        };

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("solargraph lsif failed: {err}"));
        }

        let dump_path = root.join("dump.lsif");
        if dump_path.exists() {
            return Ok(dump_path.to_string_lossy().to_string());
        } else {
            return Err(anyhow::anyhow!(
                "solargraph lsif completed successfully but did not generate a dump.lsif file."
            ));
        }
    }

    Err(anyhow::anyhow!(
        "No supported project files (Cargo.toml, tsconfig.json, Gemfile) detected in {project_root}.\n\
         Please generate the LSIF dump file manually and pass its path using `lsif_path`."
    ))
}

#[derive(Default)]
struct LsifContext {
    documents: HashMap<i64, String>,
    ranges: HashMap<i64, LsifRange>,
    monikers: HashMap<i64, LsifMoniker>,
    next_map: HashMap<i64, i64>,
    moniker_edge_map: HashMap<i64, i64>,
    def_edge_map: HashMap<i64, i64>,
    ref_edge_map: HashMap<i64, i64>,
    item_edge_map: HashMap<i64, Vec<i64>>,
    contains_edge_map: HashMap<i64, Vec<i64>>,
}

impl LsifContext {
    fn parse_lines<R: BufRead>(&mut self, reader: R) {
        for (line_num, line_res) in reader.lines().enumerate() {
            let line_str = match line_res {
                Ok(l) => l,
                Err(e) => {
                    tracing::warn!("Failed to read LSIF line {}: {}", line_num + 1, e);
                    continue;
                }
            };

            let line: LsifLine = match serde_json::from_str(&line_str) {
                Ok(val) => val,
                Err(_) => continue, // Ignore unrecognized lines
            };
            self.process_line(line);
        }
    }

    fn process_line(&mut self, line: LsifLine) {
        if line.type_field == "vertex" {
            match line.label.as_str() {
                "document" => {
                    if let Some(uri) = line.uri {
                        self.documents.insert(line.id, uri);
                    }
                }
                "range" => {
                    if let (Some(start), Some(end)) = (line.start, line.end) {
                        self.ranges.insert(
                            line.id,
                            LsifRange {
                                id: line.id,
                                start_line: start.line,
                                start_char: start.character,
                                end_line: end.line,
                                end_char: end.character,
                                document_id: None,
                            },
                        );
                    }
                }
                "moniker" => {
                    if let Some(ident) = line.identifier {
                        self.monikers.insert(
                            line.id,
                            LsifMoniker {
                                identifier: ident,
                                scheme: line.scheme.unwrap_or_default(),
                                kind: line.kind.unwrap_or_default(),
                            },
                        );
                    }
                }
                _ => {}
            }
        } else if line.type_field == "edge" {
            match line.label.as_str() {
                "contains" => {
                    if let (Some(out_v), Some(in_vs)) = (line.out_v, line.in_vs) {
                        self.contains_edge_map.insert(out_v, in_vs.clone());
                        for r_id in in_vs {
                            if let Some(r) = self.ranges.get_mut(&r_id) {
                                r.document_id = Some(out_v);
                            }
                        }
                    }
                }
                "next" => {
                    if let (Some(out_v), Some(in_v)) = (line.out_v, line.in_v) {
                        self.next_map.insert(out_v, in_v);
                    }
                }
                "moniker" => {
                    if let (Some(out_v), Some(in_v)) = (line.out_v, line.in_v) {
                        self.moniker_edge_map.insert(out_v, in_v);
                    }
                }
                "textDocument/definition" => {
                    if let (Some(out_v), Some(in_v)) = (line.out_v, line.in_v) {
                        self.def_edge_map.insert(out_v, in_v);
                    }
                }
                "textDocument/references" => {
                    if let (Some(out_v), Some(in_v)) = (line.out_v, line.in_v) {
                        self.ref_edge_map.insert(out_v, in_v);
                    }
                }
                "item" => {
                    if let Some(out_v) = line.out_v {
                        let targets = if let Some(in_vs) = line.in_vs {
                            in_vs
                        } else if let Some(in_v) = line.in_v {
                            vec![in_v]
                        } else {
                            Vec::new()
                        };
                        self.item_edge_map
                            .entry(out_v)
                            .or_default()
                            .extend(targets);
                    }
                }
                _ => {}
            }
        }
    }

    fn resolve_moniker(&self, start_id: i64) -> Option<LsifMoniker> {
        let mut curr = start_id;
        let mut visited = HashSet::new();
        while visited.insert(curr) {
            if let Some(&mon_id) = self.moniker_edge_map.get(&curr) {
                if let Some(m) = self.monikers.get(&mon_id) {
                    return Some(m.clone());
                }
            }
            if let Some(&next_id) = self.next_map.get(&curr) {
                curr = next_id;
            } else {
                break;
            }
        }
        None
    }

    fn resolve_definitions(&self, start_id: i64) -> Vec<i64> {
        let mut defs = Vec::new();
        let mut curr = start_id;
        let mut visited = HashSet::new();
        while visited.insert(curr) {
            if let Some(&def_res_id) = self.def_edge_map.get(&curr) {
                if let Some(items) = self.item_edge_map.get(&def_res_id) {
                    defs.extend(items.clone());
                }
            }
            if let Some(&next_id) = self.next_map.get(&curr) {
                curr = next_id;
            } else {
                break;
            }
        }
        defs
    }

    fn resolve_references(&self, start_id: i64) -> Vec<i64> {
        let mut refs = Vec::new();
        let mut curr = start_id;
        let mut visited = HashSet::new();
        while visited.insert(curr) {
            if let Some(&ref_res_id) = self.ref_edge_map.get(&curr) {
                if let Some(items) = self.item_edge_map.get(&ref_res_id) {
                    refs.extend(items.clone());
                }
            }
            if let Some(&next_id) = self.next_map.get(&curr) {
                curr = next_id;
            } else {
                break;
            }
        }
        refs
    }

    fn find_enclosing_def(
        &self,
        ref_range: &LsifRange,
        doc_id: i64,
        doc_defs: &HashMap<i64, Vec<(i64, usize)>>,
    ) -> Option<usize> {
        if let Some(defs_in_doc) = doc_defs.get(&doc_id) {
            let mut best_fit = None;
            let mut best_fit_len = usize::MAX;
            for &(def_id, sym_idx) in defs_in_doc {
                if let Some(def_range) = self.ranges.get(&def_id) {
                    if def_range.start_line <= ref_range.start_line
                        && def_range.end_line >= ref_range.end_line
                    {
                        let len = def_range.end_line - def_range.start_line;
                        if len < best_fit_len {
                            best_fit_len = len;
                            best_fit = Some(sym_idx);
                        }
                    }
                }
            }
            return best_fit;
        }
        None
    }
}

struct SymbolInfo {
    name: String,
    defs: Vec<i64>,
    refs: Vec<i64>,
}

#[allow(clippy::type_complexity)]
fn build_symbols(
    ctx: &LsifContext,
) -> (
    Vec<SymbolInfo>,
    HashMap<i64, usize>,
    HashMap<i64, Vec<(i64, usize)>>,
) {
    let mut symbol_sources = HashSet::new();
    for &k in ctx.def_edge_map.keys() {
        symbol_sources.insert(k);
    }
    for &k in ctx.ref_edge_map.keys() {
        symbol_sources.insert(k);
    }

    let mut symbols = Vec::new();
    let mut range_to_symbol_idx = HashMap::new();

    for src in symbol_sources {
        let defs_list = ctx.resolve_definitions(src);
        if defs_list.is_empty() {
            continue;
        }

        let moniker_opt = ctx.resolve_moniker(src);
        let name = if let Some(m) = moniker_opt {
            m.identifier
        } else if let Some(first_def_range) = ctx.ranges.get(&defs_list[0]) {
            if let Some(doc_uri) = first_def_range
                .document_id
                .and_then(|d| ctx.documents.get(&d))
            {
                let file_name = Path::new(doc_uri)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("file");
                format!(
                    "{}::symbol_L{}_C{}",
                    file_name,
                    first_def_range.start_line + 1,
                    first_def_range.start_char + 1
                )
            } else {
                format!(
                    "symbol_L{}_C{}",
                    first_def_range.start_line + 1,
                    first_def_range.start_char + 1
                )
            }
        } else {
            "anonymous_symbol".to_string()
        };

        let refs_list = ctx.resolve_references(src);
        let sym_idx = symbols.len();
        for &def_id in &defs_list {
            range_to_symbol_idx.insert(def_id, sym_idx);
        }
        for &ref_id in &refs_list {
            range_to_symbol_idx.insert(ref_id, sym_idx);
        }

        symbols.push(SymbolInfo {
            name,
            defs: defs_list,
            refs: refs_list,
        });
    }

    let mut doc_defs = HashMap::new();
    for (idx, sym) in symbols.iter().enumerate() {
        for &def_id in &sym.defs {
            if let Some(r) = ctx.ranges.get(&def_id) {
                if let Some(doc_id) = r.document_id {
                    doc_defs
                        .entry(doc_id)
                        .or_insert_with(Vec::new)
                        .push((def_id, idx));
                }
            }
        }
    }

    (symbols, range_to_symbol_idx, doc_defs)
}

#[allow(clippy::type_complexity)]
fn map_symbols_to_graph(
    ctx: &LsifContext,
    symbols: &[SymbolInfo],
    project_root: Option<&str>,
    doc_defs: &HashMap<i64, Vec<(i64, usize)>>,
) -> (
    Vec<(String, HashMap<String, String>, String)>,
    Vec<(String, String, HashMap<String, String>, String)>,
) {
    let mut bulk_nodes = Vec::new();
    let mut bulk_edges = Vec::new();
    let mut seen_nodes = HashSet::new();
    let mut seen_edges = HashSet::new();

    let root_path = project_root.map(PathBuf::from);
    let clean_path = |uri: &str| -> String {
        let mut path_str = uri.trim_start_matches("file://").to_string();
        if path_str.starts_with('/') && std::path::MAIN_SEPARATOR == '\\' {
            path_str = path_str.trim_start_matches('/').replace('/', "\\");
        }
        if let Some(ref root) = root_path {
            let p = Path::new(&path_str);
            if p.is_relative() {
                root.join(p).to_string_lossy().to_string()
            } else {
                path_str
            }
        } else {
            path_str
        }
    };

    for uri in ctx.documents.values() {
        let file_path = clean_path(uri);
        let mut props = HashMap::new();
        props.insert("name".to_string(), file_path.clone());
        props.insert("kind".to_string(), "file".to_string());

        if seen_nodes.insert(file_path.clone()) {
            bulk_nodes.push((file_path, props, "File".to_string()));
        }
    }

    let mut symbol_idx_to_node_id = HashMap::new();

    for (idx, sym) in symbols.iter().enumerate() {
        let file_path = if let Some(first_def_id) = sym.defs.first() {
            if let Some(r) = ctx.ranges.get(first_def_id) {
                if let Some(doc_uri) = r.document_id.and_then(|d| ctx.documents.get(&d)) {
                    clean_path(doc_uri)
                } else {
                    continue;
                }
            } else {
                continue;
            }
        } else {
            continue;
        };

        if sym.name.contains('#') {
            let parts: Vec<&str> = sym.name.split('#').collect();
            if parts.len() == 2 {
                let class_name = parts[0];
                let method_name = parts[1];

                let class_id = format!("{file_path}::{class_name}");
                let method_id = format!("{file_path}::{class_name}::{method_name}");

                if seen_nodes.insert(class_id.clone()) {
                    let mut props = HashMap::new();
                    props.insert("name".to_string(), class_name.to_string());
                    props.insert("kind".to_string(), "class".to_string());
                    props.insert("file".to_string(), file_path.clone());
                    bulk_nodes.push((class_id.clone(), props, "Class".to_string()));
                }

                let file_class_edge = format!("{file_path}::CONTAINS::{class_id}");
                if seen_edges.insert(file_class_edge) {
                    bulk_edges.push((
                        file_path.clone(),
                        class_id.clone(),
                        HashMap::new(),
                        "CONTAINS".to_string(),
                    ));
                }

                let method_name_full = format!("{class_name}::{method_name}");
                if seen_nodes.insert(method_id.clone()) {
                    let mut props = HashMap::new();
                    props.insert("name".to_string(), method_name_full);
                    props.insert("kind".to_string(), "method".to_string());
                    props.insert("file".to_string(), file_path.clone());
                    bulk_nodes.push((method_id.clone(), props, "Method".to_string()));
                }

                let class_method_edge = format!("{class_id}::HAS_METHOD::{method_id}");
                if seen_edges.insert(class_method_edge) {
                    bulk_edges.push((
                        class_id,
                        method_id.clone(),
                        HashMap::new(),
                        "HAS_METHOD".to_string(),
                    ));
                }

                let file_method_edge = format!("{file_path}::CONTAINS::{method_id}");
                if seen_edges.insert(file_method_edge) {
                    bulk_edges.push((
                        file_path.clone(),
                        method_id.clone(),
                        HashMap::new(),
                        "CONTAINS".to_string(),
                    ));
                }

                symbol_idx_to_node_id.insert(idx, method_id);
                continue;
            }
        }

        if sym.name.contains("::") {
            let parts: Vec<&str> = sym.name.split("::").collect();
            let last_part = parts.last().unwrap_or(&"");

            let is_fn = last_part
                .chars()
                .next()
                .map(|c| c.is_lowercase())
                .unwrap_or(true);
            let label = if is_fn {
                "Function".to_string()
            } else {
                "Struct".to_string()
            };

            let node_id = format!("{file_path}::{}", sym.name);
            if seen_nodes.insert(node_id.clone()) {
                let mut props = HashMap::new();
                props.insert("name".to_string(), sym.name.clone());
                props.insert("kind".to_string(), label.to_lowercase());
                props.insert("file".to_string(), file_path.clone());
                bulk_nodes.push((node_id.clone(), props, label));
            }

            let file_edge = format!("{file_path}::CONTAINS::{node_id}");
            if seen_edges.insert(file_edge) {
                bulk_edges.push((
                    file_path.clone(),
                    node_id.clone(),
                    HashMap::new(),
                    "CONTAINS".to_string(),
                ));
            }

            symbol_idx_to_node_id.insert(idx, node_id);
            continue;
        }

        let is_class = sym
            .name
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false);
        let label = if is_class {
            "Class".to_string()
        } else {
            "Function".to_string()
        };
        let node_id = format!("{file_path}::{}", sym.name);

        if seen_nodes.insert(node_id.clone()) {
            let mut props = HashMap::new();
            props.insert("name".to_string(), sym.name.clone());
            props.insert("kind".to_string(), label.to_lowercase());
            props.insert("file".to_string(), file_path.clone());
            bulk_nodes.push((node_id.clone(), props, label));
        }

        let file_edge = format!("{file_path}::CONTAINS::{node_id}");
        if seen_edges.insert(file_edge) {
            bulk_edges.push((
                file_path.clone(),
                node_id.clone(),
                HashMap::new(),
                "CONTAINS".to_string(),
            ));
        }

        symbol_idx_to_node_id.insert(idx, node_id);
    }

    for (idx, sym) in symbols.iter().enumerate() {
        let caller_node_id = match symbol_idx_to_node_id.get(&idx) {
            Some(id) => id,
            None => continue,
        };

        for &ref_id in &sym.refs {
            let ref_range = match ctx.ranges.get(&ref_id) {
                Some(r) => r,
                None => continue,
            };
            let doc_id = match ref_range.document_id {
                Some(d) => d,
                None => continue,
            };

            if let Some(enclosing_sym_idx) = ctx.find_enclosing_def(ref_range, doc_id, doc_defs) {
                if let Some(target_node_id) = symbol_idx_to_node_id.get(&enclosing_sym_idx) {
                    let caller = target_node_id;
                    let callee = caller_node_id;

                    if caller != callee {
                        let edge_id = format!("{caller}::CALLS::{callee}");
                        if seen_edges.insert(edge_id) {
                            bulk_edges.push((
                                caller.clone(),
                                callee.clone(),
                                HashMap::new(),
                                "CALLS".to_string(),
                            ));
                        }
                    }
                }
            }
        }
    }

    (bulk_nodes, bulk_edges)
}

pub fn parse_and_import_lsif(
    lsif_path: &str,
    db_path: &str,
    project_root: Option<&str>,
) -> Result<(usize, usize)> {
    tracing::info!("Starting LSIF import from {} into {}", lsif_path, db_path);

    let file = File::open(lsif_path)?;
    let reader = BufReader::new(file);

    let mut ctx = LsifContext::default();
    ctx.parse_lines(reader);

    let (symbols, _range_map, doc_defs) = build_symbols(&ctx);
    let (all_nodes, all_edges) = map_symbols_to_graph(&ctx, &symbols, project_root, &doc_defs);

    let nodes_count = all_nodes.len();
    let edges_count = all_edges.len();

    let conn = crate::open_db_connection(db_path).map_err(|e| anyhow::anyhow!(e))?;

    if !all_nodes.is_empty() {
        tracing::info!(
            "Bulk inserting {} nodes and {} edges...",
            all_nodes.len(),
            all_edges.len()
        );

        let mut current_tx_nodes = 0;
        tracing::info!("Starting BEGIN TRANSACTION");
        let _ = conn.query("BEGIN TRANSACTION");
        tracing::info!("Finished BEGIN TRANSACTION");

        for (id, props, label) in all_nodes {
            let mut safe_props = HashMap::new();
            let mut kind = String::new();
            for (k, v) in props {
                if k == "kind" {
                    kind = v.clone();
                }
                safe_props.insert(k, v);
            }
            let node = crate::models::Node {
                id: id.clone(),
                label,
                kind,
                properties: safe_props,
            };
            let _ = node.save(&conn);
            
            current_tx_nodes += 1;
            if current_tx_nodes >= 50 {
                let _ = conn.query("COMMIT");
                current_tx_nodes = 0;
                let _ = conn.query("BEGIN TRANSACTION");
            }
        }
        
        for (source, target, props, label) in all_edges {
            let mut safe_props = HashMap::new();
            for (k, v) in props {
                safe_props.insert(k, v);
            }
            let edge = crate::models::Edge {
                id: format!("{}::{}::{}", source, label, target),
                source,
                target,
                label,
                properties: safe_props,
            };
            let _ = edge.save(&conn);
            
            current_tx_nodes += 1;
            if current_tx_nodes >= 50 {
                let _ = conn.query("COMMIT");
                current_tx_nodes = 0;
                let _ = conn.query("BEGIN TRANSACTION");
            }
        }
        
        let _ = conn.query("COMMIT");
    }

    tracing::info!(
        "LSIF Import completed. Nodes: {}, Edges: {}",
        nodes_count,
        edges_count
    );
    Ok((nodes_count, edges_count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsif_parsing() {
        let db_path = "test_lsif.db";
        let _ = std::fs::remove_file(db_path);
        let _ = std::fs::remove_file(format!("{}.wal", db_path));

        // Create mock LSIF JSON lines file content
        let mock_content = r#"{"id":1,"type":"vertex","label":"metaData","version":"0.5.0","projectRoot":"file:///project"}
{"id":2,"type":"vertex","label":"document","uri":"file:///project/main.rs"}
{"id":3,"type":"vertex","label":"range","start":{"line":4,"character":4},"end":{"line":6,"character":5}}
{"id":4,"type":"edge","label":"contains","outV":2,"inVs":[3]}
{"id":5,"type":"vertex","label":"moniker","identifier":"main_func","scheme":"rust-analyzer","kind":"export"}
{"id":6,"type":"edge","label":"moniker","outV":3,"inVs":[5]}
{"id":7,"type":"vertex","label":"resultSet"}
{"id":8,"type":"edge","label":"next","outV":3,"inV":7}
{"id":9,"type":"vertex","label":"definitionResult"}
{"id":10,"type":"edge","label":"textDocument/definition","outV":7,"inV":9}
{"id":11,"type":"edge","label":"item","outV":9,"inVs":[3]}
"#;

        let lsif_file_path = "test_mock.lsif";
        std::fs::write(lsif_file_path, mock_content).unwrap();

        let res = parse_and_import_lsif(lsif_file_path, db_path, None);
        assert!(res.is_ok(), "Failed to parse mock LSIF: {:?}", res.err());

        let (nodes, edges) = res.unwrap();
        assert_eq!(nodes, 2); // File, Function
        assert_eq!(edges, 1); // File -> Function CONTAINS

        let _ = std::fs::remove_file(lsif_file_path);
        let _ = std::fs::remove_file(db_path);
        let _ = std::fs::remove_file(format!("{}.wal", db_path));
    }
}
