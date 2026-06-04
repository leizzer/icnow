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
        description = "The high-level category of the node, e.g., 'File', 'Class', 'Module', 'Struct', 'Interface', 'Function', 'Method', 'Import'"
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
    pub fn save(&self, graph: &graphqlite::Graph) -> anyhow::Result<()> {
        let mut props: HashMap<String, SafePropertyValue> = self
            .properties
            .iter()
            .map(|(k, v)| (k.clone(), SafePropertyValue(v.clone())))
            .collect();
        props.insert("kind".to_string(), SafePropertyValue(self.kind.clone()));

        graph
            .upsert_node(&self.id, props, &self.label)
            .map_err(|e| anyhow::anyhow!("Failed to save node: {e}"))
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
        description = "The relationship label, e.g., 'CONTAINS' (file contains element), 'HAS_METHOD', 'CALLS' (method calls method), 'IMPORTS'"
    )]
    pub label: String,
    #[schemars(
        description = "Additional properties to store on the edge (e.g., call line number, import aliases)"
    )]
    pub properties: HashMap<String, String>,
}

impl Edge {
    pub fn save(&self, graph: &graphqlite::Graph) -> anyhow::Result<()> {
        let safe_props: HashMap<String, SafePropertyValue> = self
            .properties
            .iter()
            .map(|(k, v)| (k.clone(), SafePropertyValue(v.clone())))
            .collect();
        graph
            .upsert_edge(&self.source, &self.target, safe_props, &self.label)
            .map_err(|e| anyhow::anyhow!("Failed to save edge: {e}"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SafePropertyValue(pub String);

impl From<SafePropertyValue> for graphqlite::PropertyValue {
    fn from(wrapper: SafePropertyValue) -> Self {
        let s = wrapper.0;
        let s_lower = s.to_lowercase();
        if s_lower == "nan"
            || s_lower == "infinity"
            || s_lower == "inf"
            || s_lower == "-infinity"
            || s_lower == "-inf"
        {
            return graphqlite::PropertyValue::Text(s);
        }
        graphqlite::PropertyValue::from(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphqlite::PropertyValue;

    #[test]
    fn test_safe_property_value() {
        // Special float string values should map to Text
        match PropertyValue::from(SafePropertyValue("NaN".to_string())) {
            PropertyValue::Text(val) => assert_eq!(val, "NaN"),
            other => panic!("Expected PropertyValue::Text for NaN, got {other:?}"),
        }
        match PropertyValue::from(SafePropertyValue("Infinity".to_string())) {
            PropertyValue::Text(val) => assert_eq!(val, "Infinity"),
            other => panic!("Expected PropertyValue::Text for Infinity, got {other:?}"),
        }
        match PropertyValue::from(SafePropertyValue("inf".to_string())) {
            PropertyValue::Text(val) => assert_eq!(val, "inf"),
            other => panic!("Expected PropertyValue::Text for inf, got {other:?}"),
        }
        match PropertyValue::from(SafePropertyValue("-infinity".to_string())) {
            PropertyValue::Text(val) => assert_eq!(val, "-infinity"),
            other => panic!("Expected PropertyValue::Text for -infinity, got {other:?}"),
        }

        // Normal numbers should map to Integer or Float
        match PropertyValue::from(SafePropertyValue("123".to_string())) {
            PropertyValue::Integer(val) => assert_eq!(val, 123),
            other => panic!("Expected PropertyValue::Integer for 123, got {other:?}"),
        }
        match PropertyValue::from(SafePropertyValue("123.45".to_string())) {
            PropertyValue::Float(val) => assert_eq!(val, 123.45),
            other => panic!("Expected PropertyValue::Float for 123.45, got {other:?}"),
        }

        // Arbitrary text should map to Text
        match PropertyValue::from(SafePropertyValue("hello".to_string())) {
            PropertyValue::Text(val) => assert_eq!(val, "hello"),
            other => panic!("Expected PropertyValue::Text for hello, got {other:?}"),
        }
    }
}
