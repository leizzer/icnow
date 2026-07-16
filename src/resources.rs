use rmcp::{
    Service,
    model::{
        ClientNotification, ClientRequest, ErrorCode, ErrorData, ListResourcesResult,
        ReadResourceResult, Resource, ResourceContents, ResourceTemplate, ServerInfo, ServerResult,
    },
    service::{NotificationContext, RequestContext, RoleServer},
};

use crate::tools::GraphService;
use serde_json::json;

pub struct ResourceHandler {
    inner: GraphService,
}

impl ResourceHandler {
    pub fn new(inner: GraphService) -> Self {
        Self { inner }
    }
}

impl Service<RoleServer> for ResourceHandler {
    async fn handle_request(
        &self,
        request: ClientRequest,
        context: RequestContext<RoleServer>,
    ) -> Result<ServerResult, ErrorData> {
        match request {
            ClientRequest::ListResourcesRequest(_req) => {
                let db_path = self.inner.resolve_db_path_and_watch(None, None, None);
                let result = tokio::task::spawn_blocking(
                    move || -> Result<Vec<serde_json::Value>, String> {
                        let db_res = crate::database::get_or_init_db(&db_path);
                        let conn_res = match &db_res {
                            Ok(db) => lbug::Connection::new(db.as_ref()).map_err(|e| e.to_string()),
                            Err(e) => Err(e.clone()),
                        };
                        let conn = conn_res
                            .map_err(|e| format!("Failed to open DB: {e}"))?;

                        let project_root = std::path::Path::new(&db_path)
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();

                        let mut items = Vec::new();

                        let file_query = "MATCH (f:File) RETURN f.id ORDER BY f.id LIMIT 1000";
                        if let Ok(mut res) = conn.query(file_query) {
                            for row in res.by_ref() {
                                if let lbug::Value::String(id) = &row[0] {
                                    let relative_id = if id.starts_with(&project_root) {
                                        id.strip_prefix(&project_root)
                                            .unwrap_or(id)
                                            .trim_start_matches('/')
                                            .to_string()
                                    } else {
                                        id.clone()
                                    };
                                    items.push(json!({
                                        "uri": format!("node://{}", relative_id),
                                        "name": relative_id,
                                        "mimeType": "text/markdown",
                                        "description": "Source file"
                                    }));
                                }
                            }
                        }

                        let mem_query =
                            "MATCH (m:Memory) RETURN m.id, m.name ORDER BY m.id LIMIT 1000";
                        if let Ok(mut res) = conn.query(mem_query) {
                            for row in res.by_ref() {
                                if let (lbug::Value::String(id), lbug::Value::String(name)) =
                                    (&row[0], &row[1])
                                {
                                    items.push(json!({
                                        "uri": format!("memory://{}", id),
                                        "name": format!("Memory: {}", name),
                                        "mimeType": "text/markdown",
                                        "description": "Project Memory"
                                    }));
                                }
                            }
                        }

                        Ok(items)
                    },
                )
                .await
                .map_err(|e| ErrorData {
                    code: ErrorCode(-32000),
                    message: format!("Task error: {e}").into(),
                    data: None,
                })?;

                match result {
                    Ok(files) => {
                        let resources = files
                            .into_iter()
                            .filter_map(|val| serde_json::from_value::<Resource>(val).ok())
                            .collect();
                        Ok(ServerResult::ListResourcesResult(ListResourcesResult {
                            resources,
                            meta: None,
                            next_cursor: None,
                        }))
                    }
                    Err(e) => Err(ErrorData {
                        code: ErrorCode(-32000),
                        message: e.into(),
                        data: None,
                    }),
                }
            }
            ClientRequest::ListResourceTemplatesRequest(_req) => {
                let node_template: ResourceTemplate = serde_json::from_value(json!({
                    "uriTemplate": "node://{file_path}",
                    "name": "Project File",
                    "description": "Read a file from the project graph",
                    "mimeType": "text/markdown"
                }))
                .unwrap();

                let memory_template: ResourceTemplate = serde_json::from_value(json!({
                    "uriTemplate": "memory://{memory_id}",
                    "name": "Project Memory",
                    "description": "Read a saved project memory or architectural decision",
                    "mimeType": "text/markdown"
                }))
                .unwrap();

                let result = serde_json::from_value(json!({
                    "resourceTemplates": [node_template, memory_template]
                }))
                .unwrap();

                Ok(ServerResult::ListResourceTemplatesResult(result))
            }
            ClientRequest::ListPromptsRequest(_) => crate::prompts::handle_list_prompts(),
            ClientRequest::GetPromptRequest(req) => crate::prompts::handle_get_prompt(&req),
            ClientRequest::ReadResourceRequest(req) => {
                let uri = req.params.uri.clone();
                let as_json = uri.ends_with(".json");
                let db_path = self.inner.resolve_db_path_and_watch(None, None, None);
                let uri_clone = uri.clone();

                let result = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, String> {
                    let uri = uri_clone;
                    if uri.starts_with("memory://") {
                        let id = uri.trim_start_matches("memory://").to_string();
                        let db_res = crate::database::get_or_init_db(&db_path);
                        let conn_res = match &db_res {
                            Ok(db) => lbug::Connection::new(db.as_ref()).map_err(|e| e.to_string()),
                            Err(e) => Err(e.clone()),
                        };
                        let conn = conn_res
                            .map_err(|e| format!("Failed to open DB: {e}"))?;

                        let escaped_id = id.replace("'", "''");
                        let query = format!("MATCH (m:Memory {{id: '{escaped_id}'}}) RETURN m.name, m.description, m.keywords");
                        let mut res = conn.query(&query).map_err(|e| e.to_string())?;

                        if let Some(row) = res.next() {
                            let name = match &row[0] { lbug::Value::String(s) => s.clone(), _ => id.clone() };
                            let description = match &row[1] { lbug::Value::String(s) => s.clone(), _ => "".to_string() };
                            let keywords = match &row[2] { lbug::Value::String(s) => s.clone(), _ => "".to_string() };

                            Ok(json!({
                                "id": id,
                                "kind": "Memory",
                                "name": name,
                                "signature": format!("Keywords: {}", keywords),
                                "docstring": description,
                                "source_code": ""
                            }))
                        } else {
                            Err(format!("Memory {id} not found"))
                        }
                    } else if uri.starts_with("node://") {
                        let project_root = std::path::Path::new(&db_path)
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();

                        let raw_id = urlencoding::decode(uri.trim_start_matches("node://")).unwrap_or(std::borrow::Cow::Borrowed("")).to_string();

                        // Convert relative path back to absolute path using project_root
                        let id = if std::path::Path::new(&raw_id).is_absolute() {
                            raw_id
                        } else {
                            std::path::Path::new(&project_root).join(&raw_id).to_string_lossy().to_string()
                        };

                        let db_res = crate::database::get_or_init_db(&db_path);
                        let conn_res = match &db_res {
                            Ok(db) => lbug::Connection::new(db.as_ref()).map_err(|e| e.to_string()),
                            Err(e) => Err(e.clone()),
                        };
                        let conn = conn_res
                            .map_err(|e| format!("Failed to open DB: {e}"))?;

                        let escaped_id = id.replace("'", "''");
                        let query_label = format!("MATCH (n {{id: '{escaped_id}'}}) RETURN label(n)");
                        let mut res_label = conn.query(&query_label).map_err(|e| e.to_string())?;
                        let label = if let Some(row) = res_label.next() {
                            match &row[0] {
                                lbug::Value::String(s) => s.clone(),
                                _ => return Err(format!("Node {id} has invalid label")),
                            }
                        } else {
                            return Err(format!("Node {id} not found in graph"));
                        };

                        if label == "File" {
                            let source_code = std::fs::read_to_string(&id)
                                .unwrap_or_else(|_| format!("Could not read file from disk: {id}"));
                            Ok(json!({
                                "id": id,
                                "kind": "File",
                                "name": id,
                                "signature": "",
                                "docstring": "",
                                "source_code": source_code,
                            }))
                        } else {
                            let query = format!("MATCH (n:Symbol {{id: '{escaped_id}'}}) RETURN n.kind, n.name, n.signature, n.docstring, n.source_code");

                            let mut res = conn.query(&query).map_err(|e| e.to_string())?;

                            if let Some(row) = res.next() {
                                let kind = match &row[0] { lbug::Value::String(s) => s.clone(), _ => "Node".to_string() };
                                let name = match &row[1] { lbug::Value::String(s) => s.clone(), _ => id.clone() };
                                let signature = match &row[2] { lbug::Value::String(s) => s.clone(), _ => "".to_string() };
                                let docstring = match &row[3] { lbug::Value::String(s) => s.clone(), _ => "".to_string() };
                                let source_code = match &row[4] { lbug::Value::String(s) => s.clone(), _ => "".to_string() };

                                Ok(json!({
                                    "id": id,
                                    "kind": kind,
                                    "name": name,
                                    "signature": signature,
                                    "docstring": docstring,
                                    "source_code": source_code,
                                }))
                            } else {
                                Err(format!("Symbol {id} not found"))
                            }
                        }
                    } else {
                        Err("Invalid URI scheme".to_string())
                    }
                }).await.map_err(|e| ErrorData {
                    code: ErrorCode(-32000),
                    message: format!("Task error: {e}").into(),
                    data: None,
                })?;

                match result {
                    Ok(parsed) => {
                        let text = if as_json {
                            parsed.to_string()
                        } else {
                            let kind = parsed
                                .get("kind")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Node");
                            let name = parsed.get("name").and_then(|v| v.as_str()).unwrap_or("");
                            let docstring = parsed
                                .get("docstring")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let signature = parsed
                                .get("signature")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let source = parsed
                                .get("source_code")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");

                            let mut md = format!("# {kind} `{name}`\n\n");
                            if !signature.is_empty() {
                                md.push_str(&format!("**Signature**:\n```\n{signature}\n```\n\n"));
                            }
                            if !docstring.is_empty() {
                                md.push_str(&format!("**Documentation**:\n{docstring}\n\n"));
                            }
                            if !source.is_empty() {
                                md.push_str(&format!("**Source Code**:\n```\n{source}\n```\n"));
                            }
                            md
                        };

                        let mime_type = if as_json {
                            "application/json"
                        } else {
                            "text/markdown"
                        };

                        let resource_contents = json!({
                            "uri": uri,
                            "mimeType": mime_type,
                            "text": text
                        });

                        let contents = serde_json::from_value(resource_contents)
                            .unwrap_or_else(|_| ResourceContents::text(text, uri));

                        Ok(ServerResult::ReadResourceResult(ReadResourceResult::new(
                            vec![contents],
                        )))
                    }
                    Err(e) => Err(ErrorData {
                        code: ErrorCode(-32000),
                        message: e.into(),
                        data: None,
                    }),
                }
            }
            ClientRequest::InitializeRequest(req) => {
                let res = Service::handle_request(
                    &self.inner,
                    ClientRequest::InitializeRequest(req),
                    context,
                )
                .await;
                if let Ok(ServerResult::InitializeResult(mut info)) = res {
                    if let Ok(mut json_info) = serde_json::to_value(&info) {
                        if let Some(caps) = json_info.get_mut("capabilities") {
                            if let Some(caps_obj) = caps.as_object_mut() {
                                caps_obj.insert(
                                    "resources".to_string(),
                                    json!({
                                        "listChanged": false,
                                        "subscribe": false
                                    }),
                                );
                                caps_obj.insert(
                                    "prompts".to_string(),
                                    json!({
                                        "listChanged": false
                                    }),
                                );
                            }
                        }
                        if let Ok(new_info) = serde_json::from_value(json_info) {
                            info = new_info;
                        }
                    }
                    Ok(ServerResult::InitializeResult(info))
                } else {
                    res
                }
            }
            rest => Service::handle_request(&self.inner, rest, context).await,
        }
    }

    fn get_info(&self) -> ServerInfo {
        let mut info = Service::get_info(&self.inner);
        if let Ok(mut json_info) = serde_json::to_value(&info) {
            if let Some(caps) = json_info.get_mut("capabilities") {
                if let Some(caps_obj) = caps.as_object_mut() {
                    caps_obj.insert(
                        "resources".to_string(),
                        json!({
                            "listChanged": false,
                            "subscribe": false
                        }),
                    );
                    caps_obj.insert(
                        "prompts".to_string(),
                        json!({
                            "listChanged": false
                        }),
                    );
                }
            }
            if let Ok(new_info) = serde_json::from_value(json_info) {
                info = new_info;
            }
        }
        info
    }

    async fn handle_notification(
        &self,
        notification: ClientNotification,
        context: NotificationContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        Service::handle_notification(&self.inner, notification, context).await
    }
}
