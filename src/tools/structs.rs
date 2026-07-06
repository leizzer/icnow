use crate::models::{Edge, Node};
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SaveNodeRequest {
    pub node: Node,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SaveEdgeRequest {
    pub edge: Edge,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ParseFileRequest {
    #[schemars(
        description = "The absolute or relative path to the Rust (.rs), Ruby (.rb), TypeScript (.ts), or TSX (.tsx) file to parse."
    )]
    pub file_path: String,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TraverseGraphRequest {
    #[schemars(
        description = "The globally unique string ID of the starting node (e.g. 'src/models.rs::Node')."
    )]
    pub node_id: String,
    #[schemars(description = "Maximum depth of recursive hops to traverse. Defaults to 2.")]
    pub max_depth: Option<u32>,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QueryGraphCypherRequest {
    #[schemars(
        description = "The Cypher query string to execute. Example: 'MATCH (c:Class)-[:DEFINES]->(m) RETURN c.id, m.id LIMIT 10'"
    )]
    pub query: String,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchSymbolsRequest {
    #[schemars(
        description = "The symbol name or pattern to search for (e.g., 'UserController', 'main', 'save_node')."
    )]
    pub query: String,
    #[schemars(description = "Optional limit on the number of results. Defaults to 50.")]
    pub limit: Option<u32>,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
    #[schemars(
        description = "Optional list of node labels to filter the results (e.g., ['Class', 'Method'])."
    )]
    pub kind_filter: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetDependenciesRequest {
    #[schemars(
        description = "The node ID or exact symbol name to trace dependencies for (e.g. 'src/main.rs::main' or just 'save_node')."
    )]
    pub node_id: String,
    #[schemars(
        description = "Direction to trace: 'incoming' (find callers of this node) or 'outgoing' (find what this node calls)."
    )]
    pub direction: String,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSymbolInfoRequest {
    #[schemars(
        description = "The node ID to retrieve 360-degree context for (e.g. 'src/main.rs::main')."
    )]
    pub node_id: String,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListIndexedFilesRequest {
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CoverageCheckRequest {
    #[schemars(
        description = "The absolute path to the directory to check coverage for (e.g. '/path/to/app/services')."
    )]
    pub directory_path: String,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetFileStructureRequest {
    #[schemars(
        description = "The absolute or relative path to the file to query (e.g. '/path/to/app/models/user.rb')."
    )]
    pub file_path: String,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GenerateInteractiveMapRequest {
    #[schemars(
        description = "The absolute path where the HTML file should be saved (e.g. '/path/to/project/architecture.html')."
    )]
    pub output_path: String,
    #[schemars(
        description = "Optional path prefix to filter the exported graph. Only nodes starting with this path (e.g. a specific directory or file) will be included."
    )]
    pub filter_path: Option<String>,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSymbolImplementationRequest {
    #[schemars(
        description = "The globally unique string ID of the node to retrieve source code for (e.g. 'src/models.rs::Node')."
    )]
    pub node_id: String,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TraceCallPathRequest {
    #[schemars(description = "The globally unique string ID of the starting node (caller).")]
    pub start_node_id: String,
    #[schemars(description = "The globally unique string ID of the target node (callee).")]
    pub end_node_id: String,
    #[schemars(
        description = "Maximum depth of recursive hops to traverse. Defaults to 5. Maximum is 10."
    )]
    pub max_depth: Option<u32>,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetGraphSchemaRequest {
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeepScanRequest {
    #[schemars(
        description = "Optional path to a pre-generated LSIF dump file. If omitted, icnow will attempt to auto-generate the LSIF dump based on project detection."
    )]
    pub lsif_path: Option<String>,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SaveMemoryRequest {
    #[schemars(
        description = "The globally unique string ID of the memory node. MUST start with the prefix 'memory::' (e.g. 'memory::user_auth')."
    )]
    pub id: String,
    #[schemars(
        description = "A concise, human-readable name for the concept or logic block (e.g. 'User Authentication Flow')."
    )]
    pub name: String,
    #[schemars(
        description = "A detailed description of the memory concept, detailing its architectural role, business rules, or key steps."
    )]
    pub description: String,
    #[schemars(
        description = "A list of relevant keywords to index this memory for search (e.g. ['login', 'jwt', 'session'])."
    )]
    pub keywords: Vec<String>,
    #[schemars(
        description = "A list of globally unique IDs of code elements (Files, Classes, Methods) or other memory nodes that this concept explains or relates to."
    )]
    pub links: Vec<String>,
    #[schemars(
        description = "Optional custom label type for the relationship edges created. Defaults to 'EXPLAINS' for code nodes and 'SUB_CONCEPT' for memory nodes."
    )]
    pub link_type: Option<String>,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetMemoryRequest {
    #[schemars(
        description = "The globally unique string ID of the memory node to retrieve (must start with 'memory::')."
    )]
    pub id: String,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchMemoriesRequest {
    #[schemars(
        description = "The search query to match against memory names, descriptions, and keywords using vector similarity."
    )]
    pub query: String,
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListMemoriesRequest {
    #[schemars(
        description = "Optional absolute path to the project root directory. If not specified, defaults to the server's current working directory."
    )]
    pub project_root: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetVersionRequest {}
