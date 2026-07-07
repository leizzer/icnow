use super::escape_cypher_string;
use lbug::Connection;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
        description = "The relationship label, e.g., 'CONTAINS' (file contains element), 'DEFINES', 'CALLS' (method calls method), 'IMPORTS', 'REFERENCES', 'INHERITS', 'INSTANTIATES'"
    )]
    pub label: String,
    #[schemars(
        description = "Additional properties to store on the edge (e.g., call line number, import aliases)"
    )]
    pub properties: HashMap<String, String>,
}
fn infer_node_label(id: &str) -> String {
    if id.starts_with("memory::") {
        "Memory".to_string()
    } else if id.starts_with('/') && !id.contains("::") {
        "File".to_string()
    } else {
        "Symbol".to_string()
    }
}

impl Edge {
    pub fn save(&self, conn: &Connection) -> anyhow::Result<()> {
        let rel_table = match self.label.as_str() {
            "CONTAINS" => "CONTAINS",
            "DEFINES" => "DEFINES",
            "CALLS" => "CALLS",
            "INHERITS" => "INHERITS",
            "INSTANTIATES" => "INSTANTIATES",
            "IMPORTS" => "IMPORTS",
            "DEPENDS_ON" => "DEPENDS_ON",
            "REFERENCES" => "REFERENCES",
            _ => "CALLS",
        };

        let src_label = infer_node_label(&self.source);
        let tgt_label = infer_node_label(&self.target);

        let query = format!(
            "MATCH (s:{} {{id: '{}'}}), (t:{} {{id: '{}'}}) MERGE (s)-[:{}]->(t)",
            src_label,
            escape_cypher_string(&self.source),
            tgt_label,
            escape_cypher_string(&self.target),
            rel_table
        );

        conn.query(&query)
            .map_err(|e| anyhow::anyhow!("Failed to save edge: {e}"))?;
        Ok(())
    }
}
