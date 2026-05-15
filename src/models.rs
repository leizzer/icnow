use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Node {
    pub id: u32,
    pub label: String, // e.g., "User", "Product", "Order"
    pub kind: String, // e.g., "Model", "Controller", "View"
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Edge {
    pub id: u32,
    pub source: u32,
    pub target: u32,
    pub label: String,
    pub properties: HashMap<String, String>,
}
