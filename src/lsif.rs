use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use serde::Deserialize;
use anyhow::Result;
use graphqlite::Graph;

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
            return Err(anyhow::anyhow!("rust-analyzer lsif failed: {}", err));
        }
        
        let temp_file_path = std::env::temp_dir().join("icnow_rust_lsif.lsif");
        std::fs::write(&temp_file_path, output.stdout)?;
        return Ok(temp_file_path.to_string_lossy().to_string());
    }
    
    // 2. Detect TypeScript/React
    if root.join("tsconfig.json").exists() {
        tracing::info!("Detected TypeScript/React project. Running `npx -y @sourcegraph/lsif-tsc`...");
        let output = std::process::Command::new("npx")
            .args(&["-y", "@sourcegraph/lsif-tsc", "-p", "tsconfig.json"])
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
            return Err(anyhow::anyhow!("npx @sourcegraph/lsif-tsc failed: {}", err));
        }
        
        // npx @sourcegraph/lsif-tsc generates `dump.lsif` in the project root
        let dump_path = root.join("dump.lsif");
        if dump_path.exists() {
            return Ok(dump_path.to_string_lossy().to_string());
        } else {
            return Err(anyhow::anyhow!("npx @sourcegraph/lsif-tsc completed successfully but did not generate a dump.lsif file."));
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
            return Err(anyhow::anyhow!("solargraph lsif failed: {}", err));
        }
        
        let dump_path = root.join("dump.lsif");
        if dump_path.exists() {
            return Ok(dump_path.to_string_lossy().to_string());
        } else {
            return Err(anyhow::anyhow!("solargraph lsif completed successfully but did not generate a dump.lsif file."));
        }
    }
    
    Err(anyhow::anyhow!(
        "No supported project files (Cargo.toml, tsconfig.json, Gemfile) detected in {}.\n\
         Please generate the LSIF dump file manually and pass its path using `lsif_path`.",
        project_root
    ))
}

pub fn parse_and_import_lsif(lsif_path: &str, db_path: &str, project_root: Option<&str>) -> Result<(usize, usize)> {
    tracing::info!("Starting LSIF import from {} into {}", lsif_path, db_path);
    
    let file = File::open(lsif_path)?;
    let reader = BufReader::new(file);
    
    let mut documents = HashMap::new(); // id -> uri
    let mut ranges = HashMap::new(); // id -> LsifRange
    let mut monikers = HashMap::new(); // id -> LsifMoniker
    
    let mut next_map = HashMap::new(); // outV -> inV
    let mut moniker_edge_map = HashMap::new(); // outV -> moniker_id
    let mut def_edge_map = HashMap::new(); // outV -> definitionResult_id
    let mut ref_edge_map = HashMap::new(); // outV -> referenceResult_id
    let mut item_edge_map = HashMap::new(); // outV -> list of range/resultSet IDs
    let mut contains_edge_map = HashMap::new(); // doc_id -> list of range_ids

    // 1. First pass: parse all JSON lines
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
        
        if line.type_field == "vertex" {
            match line.label.as_str() {
                "document" => {
                    if let Some(uri) = line.uri {
                        documents.insert(line.id, uri);
                    }
                }
                "range" => {
                    if let (Some(start), Some(end)) = (line.start, line.end) {
                        ranges.insert(line.id, LsifRange {
                            id: line.id,
                            start_line: start.line,
                            start_char: start.character,
                            end_line: end.line,
                            end_char: end.character,
                            document_id: None,
                        });
                    }
                }
                "moniker" => {
                    if let Some(ident) = line.identifier {
                        monikers.insert(line.id, LsifMoniker {
                            identifier: ident,
                            scheme: line.scheme.unwrap_or_default(),
                            kind: line.kind.unwrap_or_default(),
                        });
                    }
                }
                _ => {}
            }
        } else if line.type_field == "edge" {
            match line.label.as_str() {
                "contains" => {
                    if let (Some(out_v), Some(in_vs)) = (line.out_v, line.in_vs) {
                        contains_edge_map.insert(out_v, in_vs.clone());
                        for r_id in in_vs {
                            if let Some(r) = ranges.get_mut(&r_id) {
                                r.document_id = Some(out_v);
                            }
                        }
                    }
                }
                "next" => {
                    if let (Some(out_v), Some(in_v)) = (line.out_v, line.in_v) {
                        next_map.insert(out_v, in_v);
                    }
                }
                "moniker" => {
                    if let (Some(out_v), Some(in_v)) = (line.out_v, line.in_v) {
                        moniker_edge_map.insert(out_v, in_v);
                    }
                }
                "textDocument/definition" => {
                    if let (Some(out_v), Some(in_v)) = (line.out_v, line.in_v) {
                        def_edge_map.insert(out_v, in_v);
                    }
                }
                "textDocument/references" => {
                    if let (Some(out_v), Some(in_v)) = (line.out_v, line.in_v) {
                        ref_edge_map.insert(out_v, in_v);
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
                        item_edge_map.entry(out_v).or_insert_with(Vec::new).extend(targets);
                    }
                }
                _ => {}
            }
        }
    }

    // Trace helper for monikers
    let resolve_moniker = |start_id: i64| -> Option<LsifMoniker> {
        let mut curr = start_id;
        let mut visited = HashSet::new();
        while visited.insert(curr) {
            if let Some(&mon_id) = moniker_edge_map.get(&curr) {
                if let Some(m) = monikers.get(&mon_id) {
                    return Some(m.clone());
                }
            }
            if let Some(&next_id) = next_map.get(&curr) {
                curr = next_id;
            } else {
                break;
            }
        }
        None
    };

    // Trace helper for definitions
    let resolve_definitions = |start_id: i64| -> Vec<i64> {
        let mut defs = Vec::new();
        let mut curr = start_id;
        let mut visited = HashSet::new();
        while visited.insert(curr) {
            if let Some(&def_res_id) = def_edge_map.get(&curr) {
                if let Some(items) = item_edge_map.get(&def_res_id) {
                    defs.extend(items.clone());
                }
            }
            if let Some(&next_id) = next_map.get(&curr) {
                curr = next_id;
            } else {
                break;
            }
        }
        defs
    };

    // Trace helper for references
    let resolve_references = |start_id: i64| -> Vec<i64> {
        let mut refs = Vec::new();
        let mut curr = start_id;
        let mut visited = HashSet::new();
        while visited.insert(curr) {
            if let Some(&ref_res_id) = ref_edge_map.get(&curr) {
                if let Some(items) = item_edge_map.get(&ref_res_id) {
                    refs.extend(items.clone());
                }
            }
            if let Some(&next_id) = next_map.get(&curr) {
                curr = next_id;
            } else {
                break;
            }
        }
        refs
    };

    // 2. Build symbols representation
    // Let's scan all keys in def_edge_map and ref_edge_map
    let mut symbol_sources = HashSet::new();
    for &k in def_edge_map.keys() {
        symbol_sources.insert(k);
    }
    for &k in ref_edge_map.keys() {
        symbol_sources.insert(k);
    }

    struct SymbolInfo {
        name: String,
        defs: Vec<i64>,
        refs: Vec<i64>,
    }

    let mut symbols = Vec::new();
    let mut range_to_symbol_idx = HashMap::new(); // range_id -> index in symbols vec

    for src in symbol_sources {
        let defs_list = resolve_definitions(src);
        if defs_list.is_empty() {
            continue;
        }
        
        let moniker_opt = resolve_moniker(src);
        let name = if let Some(m) = moniker_opt {
            m.identifier
        } else {
            // Fallback: name after the first definition position
            if let Some(first_def_range) = ranges.get(&defs_list[0]) {
                if let Some(doc_uri) = first_def_range.document_id.and_then(|d| documents.get(&d)) {
                    let file_name = Path::new(doc_uri).file_name().and_then(|f| f.to_str()).unwrap_or("file");
                    format!("{}::symbol_L{}_C{}", file_name, first_def_range.start_line + 1, first_def_range.start_char + 1)
                } else {
                    format!("symbol_L{}_C{}", first_def_range.start_line + 1, first_def_range.start_char + 1)
                }
            } else {
                "anonymous_symbol".to_string()
            }
        };

        let refs_list = resolve_references(src);
        
        let sym_idx = symbols.len();
        for &def_id in &defs_list {
            range_to_symbol_idx.insert(def_id, sym_idx);
        }
        // Save the references to map call edges
        for &ref_id in &refs_list {
            range_to_symbol_idx.insert(ref_id, sym_idx);
        }

        symbols.push(SymbolInfo {
            name,
            defs: defs_list,
            refs: refs_list,
        });
    }

    // Group definition ranges by document to allow fast lookup of enclosing definitions for references
    let mut doc_defs = HashMap::new(); // doc_id -> list of (def_range_id, symbol_index)
    for (idx, sym) in symbols.iter().enumerate() {
        for &def_id in &sym.defs {
            if let Some(r) = ranges.get(&def_id) {
                if let Some(doc_id) = r.document_id {
                    doc_defs.entry(doc_id).or_insert_with(Vec::new).push((def_id, idx));
                }
            }
        }
    }

    // Helper to find the enclosing definition range for a reference range
    let find_enclosing_def = |ref_range: &LsifRange, doc_id: i64, doc_defs: &HashMap<i64, Vec<(i64, usize)>>, ranges: &HashMap<i64, LsifRange>| -> Option<usize> {
        if let Some(defs_in_doc) = doc_defs.get(&doc_id) {
            let mut best_fit = None;
            let mut best_fit_len = usize::MAX;
            
            for &(def_id, sym_idx) in defs_in_doc {
                if let Some(def_range) = ranges.get(&def_id) {
                    // Check if ref_range is inside def_range
                    if def_range.start_line <= ref_range.start_line && def_range.end_line >= ref_range.end_line {
                        // Keep the smallest enclosing definition (for nested scopes)
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
    };

    // 3. Build Node and Edge maps to save to DB
    let mut bulk_nodes: Vec<(String, HashMap<String, String>, String)> = Vec::new();
    let mut bulk_edges: Vec<(String, String, HashMap<String, String>, String)> = Vec::new();
    let mut seen_nodes = HashSet::new();
    let mut seen_edges = HashSet::new();

    let root_path = project_root.map(PathBuf::from);

    // Normalize URIs to paths
    let clean_path = |uri: &str| -> String {
        let mut path_str = uri.trim_start_matches("file://").to_string();
        // Handle Windows URI slash formatting
        if path_str.starts_with('/') && std::path::MAIN_SEPARATOR == '\\' {
            path_str = path_str.trim_start_matches('/').replace('/', "\\");
        }
        
        // Make it absolute/relative to root if needed
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

    // Insert Document/File nodes
    for (&_doc_id, uri) in &documents {
        let file_path = clean_path(uri);
        let mut props = HashMap::new();
        props.insert("name".to_string(), file_path.clone());
        props.insert("kind".to_string(), "file".to_string());
        
        if seen_nodes.insert(file_path.clone()) {
            bulk_nodes.push((file_path, props, "File".to_string()));
        }
    }

    // Map each symbol to structural nodes (Class/Method/Function)
    let mut symbol_idx_to_node_id = HashMap::new();

    for (idx, sym) in symbols.iter().enumerate() {
        // Find defining file path
        let file_path = if let Some(first_def_id) = sym.defs.first() {
            if let Some(r) = ranges.get(first_def_id) {
                if let Some(doc_uri) = r.document_id.and_then(|d| documents.get(&d)) {
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

        // Moniker parsing logic
        // 1. Ruby Solargraph style Class#method
        if sym.name.contains('#') {
            let parts: Vec<&str> = sym.name.split('#').collect();
            if parts.len() == 2 {
                let class_name = parts[0];
                let method_name = parts[1];
                
                let class_id = format!("{}::{}", file_path, class_name);
                let method_id = format!("{}::{}::{}", file_path, class_name, method_name);
                
                // Add Class Node
                if seen_nodes.insert(class_id.clone()) {
                    let mut props = HashMap::new();
                    props.insert("name".to_string(), class_name.to_string());
                    props.insert("kind".to_string(), "class".to_string());
                    props.insert("file".to_string(), file_path.clone());
                    bulk_nodes.push((class_id.clone(), props, "Class".to_string()));
                }
                
                // Link File -> Class
                let file_class_edge = format!("{}::CONTAINS::{}", file_path, class_id);
                if seen_edges.insert(file_class_edge) {
                    bulk_edges.push((file_path.clone(), class_id.clone(), HashMap::new(), "CONTAINS".to_string()));
                }
                
                // Add Method Node
                let method_name_full = format!("{}::{}", class_name, method_name);
                if seen_nodes.insert(method_id.clone()) {
                    let mut props = HashMap::new();
                    props.insert("name".to_string(), method_name_full);
                    props.insert("kind".to_string(), "method".to_string());
                    props.insert("file".to_string(), file_path.clone());
                    bulk_nodes.push((method_id.clone(), props, "Method".to_string()));
                }
                
                // Link Class -> Method
                let class_method_edge = format!("{}::HAS_METHOD::{}", class_id, method_id);
                if seen_edges.insert(class_method_edge) {
                    bulk_edges.push((class_id, method_id.clone(), HashMap::new(), "HAS_METHOD".to_string()));
                }
                
                // Link File -> Method
                let file_method_edge = format!("{}::CONTAINS::{}", file_path, method_id);
                if seen_edges.insert(file_method_edge) {
                    bulk_edges.push((file_path, method_id.clone(), HashMap::new(), "CONTAINS".to_string()));
                }
                
                symbol_idx_to_node_id.insert(idx, method_id);
                continue;
            }
        }

        // 2. Namespace double colon split (Rust, C++, or modules)
        if sym.name.contains("::") {
            let parts: Vec<&str> = sym.name.split("::").collect();
            let last_part = parts.last().unwrap_or(&"");
            
            // Check if last part starts with lowercase (function/method)
            let is_fn = last_part.chars().next().map(|c| c.is_lowercase()).unwrap_or(true);
            let label = if is_fn { "Function".to_string() } else { "Struct".to_string() };
            
            let node_id = format!("{}::{}", file_path, sym.name);
            if seen_nodes.insert(node_id.clone()) {
                let mut props = HashMap::new();
                props.insert("name".to_string(), sym.name.clone());
                props.insert("kind".to_string(), label.to_lowercase());
                props.insert("file".to_string(), file_path.clone());
                bulk_nodes.push((node_id.clone(), props, label));
            }
            
            // Link File -> Symbol
            let file_edge = format!("{}::CONTAINS::{}", file_path, node_id);
            if seen_edges.insert(file_edge) {
                bulk_edges.push((file_path, node_id.clone(), HashMap::new(), "CONTAINS".to_string()));
            }
            
            symbol_idx_to_node_id.insert(idx, node_id);
            continue;
        }

        // 3. Fallback / Single names
        let is_class = sym.name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
        let label = if is_class { "Class".to_string() } else { "Function".to_string() };
        let node_id = format!("{}::{}", file_path, sym.name);
        
        if seen_nodes.insert(node_id.clone()) {
            let mut props = HashMap::new();
            props.insert("name".to_string(), sym.name.clone());
            props.insert("kind".to_string(), label.to_lowercase());
            props.insert("file".to_string(), file_path.clone());
            bulk_nodes.push((node_id.clone(), props, label));
        }
        
        let file_edge = format!("{}::CONTAINS::{}", file_path, node_id);
        if seen_edges.insert(file_edge) {
            bulk_edges.push((file_path, node_id.clone(), HashMap::new(), "CONTAINS".to_string()));
        }
        
        symbol_idx_to_node_id.insert(idx, node_id);
    }

    // Now resolve references to map CALLS edges
    for (idx, sym) in symbols.iter().enumerate() {
        let caller_node_id = match symbol_idx_to_node_id.get(&idx) {
            Some(id) => id,
            None => continue,
        };

        for &ref_id in &sym.refs {
            let ref_range = match ranges.get(&ref_id) {
                Some(r) => r,
                None => continue,
            };
            let doc_id = match ref_range.document_id {
                Some(d) => d,
                None => continue,
            };

            // Find which definition encloses this reference range
            if let Some(enclosing_sym_idx) = find_enclosing_def(ref_range, doc_id, &doc_defs, &ranges) {
                if let Some(target_node_id) = symbol_idx_to_node_id.get(&enclosing_sym_idx) {
                    // target_node_id is the method/function being called
                    // caller_node_id is the caller method/function
                    // Let's create an edge: target_node_id -> caller_node_id?
                    // Wait, CALLS goes from caller to target (i.e. target is the callee).
                    // So caller_node_id (enclosing range) CALLS caller_node_id? No:
                    // Let's trace it:
                    // - `enclosing_sym_idx` is the symbol index of the enclosing definition (i.e., the caller).
                    // - `idx` is the symbol index of the reference (i.e., the callee).
                    // So: `caller = target_node_id` (representing the enclosing definition).
                    // `callee = caller_node_id` (representing the symbol we resolved from the reference).
                    // Yes! Let's rename the variables to avoid confusion:
                    let caller = target_node_id;
                    let callee = caller_node_id;
                    
                    if caller != callee {
                        let edge_id = format!("{}::CALLS::{}", caller, callee);
                        if seen_edges.insert(edge_id) {
                            bulk_edges.push((caller.clone(), callee.clone(), HashMap::new(), "CALLS".to_string()));
                        }
                    }
                }
            }
        }
    }

    // 4. Save to SQLite
    let graph = crate::open_db_graph(db_path)?;
    let node_map = graph.insert_nodes_bulk(bulk_nodes).map_err(|e| anyhow::anyhow!("Bulk insert nodes failed: {}", e))?;
    graph.insert_edges_bulk(bulk_edges, &node_map).map_err(|e| anyhow::anyhow!("Bulk insert edges failed: {}", e))?;

    let nodes_count = seen_nodes.len();
    let edges_count = seen_edges.len();
    
    tracing::info!("LSIF Import completed. Nodes: {}, Edges: {}", nodes_count, edges_count);
    Ok((nodes_count, edges_count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsif_parsing() {
        let db_path = "test_lsif.db";
        let _ = std::fs::remove_file(db_path);
        
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
        
        let _ = std::fs::remove_file(db_path);
        let _ = std::fs::remove_file(lsif_file_path);
    }
}
