use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use lbug::Connection;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Node {
    #[schemars(
        description = "A globally unique identifier. For structural elements, use the format 'path/to/file.ext::ClassName::method_name' or 'path/to/file.ext::function_name'. For files, use the file path itself."
    )]
    pub id: String,
    #[schemars(
        description = "The high-level category of the node, e.g., 'File', 'Class', 'Module', 'Struct', 'Interface', 'Function', 'Method', 'Import', 'Memory'"
    )]
    pub label: String,
    #[schemars(
        description = "The specific AST syntax kind or code element, e.g., 'function_declaration', 'class_declaration', 'method_definition'"
    )]
    pub kind: String,
    #[schemars(
        description = "A key-value map for arbitrary node metadata, such as 'name', 'file', 'source_code', or 'last_modified'"
    )]
    pub properties: HashMap<String, String>,
}

pub fn escape_cypher_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

impl Node {
    pub fn save(&self, conn: &Connection) -> anyhow::Result<()> {
        let table_name = match self.label.as_str() {
            "File" => "File",
            "Memory" => "Memory",
            _ => "Symbol",
        };
        
        let mut query = format!("MERGE (n:{} {{id: '{}'}})", table_name, escape_cypher_string(&self.id));
        let mut sets = vec![];
        if table_name == "Symbol" {
            sets.push(format!("n.kind = '{}'", escape_cypher_string(&self.kind)));
        }
        let valid_keys: Vec<&str> = match table_name {
            "Symbol" => vec!["name", "signature", "docstring", "kind", "start_line", "end_line", "file", "line"],
            "File" => vec!["name", "kind", "last_modified"],
            "Memory" => vec!["name", "description", "keywords", "embedding"],
            _ => vec![],
        };

        for (k, v) in &self.properties {
            if k != "id" && valid_keys.contains(&k.as_str()) {
                if k == "embedding" || k == "start_line" || k == "end_line" {
                    sets.push(format!("n.{} = {}", k, v));
                } else {
                    sets.push(format!("n.{} = '{}'", k, escape_cypher_string(v)));
                }
            }
        }
        
        if !sets.is_empty() {
            query.push_str(" ON MATCH SET ");
            query.push_str(&sets.join(", "));
            query.push_str(" ON CREATE SET ");
            query.push_str(&sets.join(", "));
        }
        
        conn.query(&query).map_err(|e| anyhow::anyhow!("Failed to save node: {e}"))?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Edge {
    #[schemars(
        description = "A globally unique identifier for this specific relationship edge, e.g., 'source_id::RELATION_NAME::target_id'"
    )]
    pub id: String,
    #[schemars(description = "The globally unique string ID of the source node")]
    pub source: String,
    #[schemars(description = "The globally unique string ID of the target node")]
    pub target: String,
    #[schemars(
        description = "The relationship label, e.g., 'CONTAINS' (file contains element), 'DEFINES', 'CALLS' (method calls method), 'IMPORTS', 'REFERENCES'"
    )]
    pub label: String,
    #[schemars(
        description = "Additional properties to store on the edge (e.g., call line number, import aliases)"
    )]
    pub properties: HashMap<String, String>,
}

fn get_node_label(conn: &Connection, id: &str) -> Option<String> {
    for label in &["File", "Symbol", "Memory"] {
        let q = format!("MATCH (n:{} {{id: '{}'}}) RETURN n.id LIMIT 1", label, escape_cypher_string(id));
        if let Ok(mut res) = conn.query(&q) {
            if res.by_ref().next().is_some() {
                return Some(label.to_string());
            }
        }
    }
    None
}

impl Edge {
    pub fn save(&self, conn: &Connection) -> anyhow::Result<()> {
        let rel_table = match self.label.as_str() {
            "CONTAINS" => "CONTAINS",
            "DEFINES" => "DEFINES",
            "CALLS" => "CALLS",
            "IMPORTS" => "IMPORTS",
            "REFERENCES" => "REFERENCES",
            _ => "CALLS",
        };

        let src_label = get_node_label(conn, &self.source).unwrap_or_else(|| {
            if self.source.starts_with("memory::") {
                "Memory".to_string()
            } else if self.source.starts_with('/') && !self.source.contains("::") {
                "File".to_string()
            } else {
                "Symbol".to_string()
            }
        });

        let tgt_label = get_node_label(conn, &self.target).unwrap_or_else(|| {
            if self.target.starts_with("memory::") {
                "Memory".to_string()
            } else if self.target.starts_with('/') && !self.target.contains("::") {
                "File".to_string()
            } else {
                "Symbol".to_string()
            }
        });

        let query = format!(
            "MATCH (s:{} {{id: '{}'}}), (t:{} {{id: '{}'}}) MERGE (s)-[:{}]->(t)",
            src_label,
            escape_cypher_string(&self.source),
            tgt_label,
            escape_cypher_string(&self.target),
            rel_table
        );

        conn.query(&query).map_err(|e| anyhow::anyhow!("Failed to save edge: {e}"))?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SafePropertyValue(pub String);
