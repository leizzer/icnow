use super::escape_cypher_string;
use lbug::Connection;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
impl Node {
    pub fn save(&self, conn: &Connection) -> anyhow::Result<()> {
        let table_name = match self.label.as_str() {
            "File" => "File",
            "Memory" => "Memory",
            _ => "Symbol",
        };

        let mut query = format!(
            "MERGE (n:{} {{id: '{}'}})",
            table_name,
            escape_cypher_string(&self.id)
        );
        let mut sets = vec![];
        if table_name == "Symbol" {
            sets.push(format!("n.kind = '{}'", escape_cypher_string(&self.kind)));
        }
        let valid_keys: Vec<&str> = match table_name {
            "Symbol" => vec![
                "name",
                "signature",
                "docstring",
                "kind",
                "start_line",
                "end_line",
                "file",
                "line",
            ],
            "File" => vec!["name", "kind", "last_modified"],
            "Memory" => vec!["name", "description", "keywords", "embedding"],
            _ => vec![],
        };

        for (k, v) in &self.properties {
            if k != "id" && valid_keys.contains(&k.as_str()) {
                if k == "embedding" || k == "start_line" || k == "end_line" {
                    sets.push(format!("n.{k} = {v}"));
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

        conn.query(&query)
            .map_err(|e| anyhow::anyhow!("Failed to save node: {e}"))?;
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SafePropertyValue(pub String);
