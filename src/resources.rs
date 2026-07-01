use rmcp::{
    Service,
    model::{
        ClientNotification, ClientRequest, ErrorCode, ErrorData, ListResourcesResult, ReadResourceResult,
        ResourceContents, ServerInfo, ServerResult, ResourceTemplate, Resource,
    },
    service::{NotificationContext, RequestContext, RoleServer, DynService},
};

use crate::tools::GraphService;
use std::borrow::Cow;
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
                let result = tokio::task::spawn_blocking(move || -> Result<Vec<serde_json::Value>, String> {
                    let conn = crate::open_db_connection(&db_path)
                        .map_err(|e| format!("Failed to open DB: {e}"))?;
                    
                    let query = "MATCH (f:File) RETURN f.id ORDER BY f.id LIMIT 1000";
                    let mut res = conn.query(query).map_err(|e| e.to_string())?;
                    
                    let mut files = Vec::new();
                    while let Some(row) = res.next() {
                        if let lbug::Value::String(id) = &row[0] {
                            files.push(json!({
                                "uri": format!("icnow://node/{}", urlencoding::encode(id)),
                                "name": id.clone(),
                                "mimeType": "text/markdown",
                                "description": "Source file"
                            }));
                        }
                    }
                    Ok(files)
                }).await.map_err(|e| ErrorData {
                    code: ErrorCode(-32000),
                    message: format!("Task error: {}", e).into(),
                    data: None,
                })?;

                match result {
                    Ok(files) => {
                        let resources = files.into_iter().filter_map(|val| {
                            serde_json::from_value::<Resource>(val).ok()
                        }).collect();
                        Ok(ServerResult::ListResourcesResult(ListResourcesResult {
                            resources,
                            meta: None,
                            next_cursor: None,
                        }))
                    },
                    Err(e) => Err(ErrorData {
                        code: ErrorCode(-32000),
                        message: e.into(),
                        data: None,
                    }),
                }
            }
            ClientRequest::ListResourceTemplatesRequest(_req) => {
                // Return a template for the node so clients can expose it via the @ menu
                let template: ResourceTemplate = serde_json::from_value(json!({
                    "uriTemplate": "icnow://node/{node_id}",
                    "name": "Graph Node",
                    "description": "Read a specific symbol or file from the codebase graph",
                    "mimeType": "text/markdown"
                })).unwrap();

                let result = serde_json::from_value(json!({
                    "resourceTemplates": [template]
                })).unwrap();

                Ok(ServerResult::ListResourceTemplatesResult(result))
            }
            ClientRequest::ReadResourceRequest(req) => {
                let uri = req.params.uri.clone();
                let Some(node_id) = uri.strip_prefix("icnow://node/") else {
                    return Err(ErrorData {
                        code: ErrorCode(-32602),
                        message: "Invalid resource URI. Must start with icnow://node/".to_string().into(),
                        data: None,
                    });
                };
                
                let (id, as_json) = if let Some(stripped) = node_id.strip_suffix("/json") {
                    (stripped, true)
                } else {
                    (node_id, false)
                };
                
                let id = urlencoding::decode(id).unwrap_or(Cow::Borrowed(id)).to_string();

                let db_path = self.inner.resolve_db_path_and_watch(None, None, Some(&id));
                
                let result = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, String> {
                    let conn = crate::open_db_connection(&db_path)
                        .map_err(|e| format!("Failed to open DB: {e}"))?;
                    
                    let escaped_id = id.replace("'", "''");
                    let query_label = format!("MATCH (n {{id: '{}'}}) RETURN label(n)", escaped_id);
                    let mut res_label = conn.query(&query_label).map_err(|e| e.to_string())?;
                    let label = if let Some(row) = res_label.next() {
                        match &row[0] {
                            lbug::Value::String(s) => s.clone(),
                            _ => return Err(format!("Node {} has invalid label", id)),
                        }
                    } else {
                        return Err(format!("Node {} not found in graph", id));
                    };

                    if label == "File" {
                        let source_code = std::fs::read_to_string(&id)
                            .unwrap_or_else(|_| format!("Could not read file from disk: {}", id));
                        Ok(json!({
                            "id": id,
                            "kind": "File",
                            "name": id,
                            "signature": "",
                            "docstring": "",
                            "source_code": source_code,
                        }))
                    } else {
                        let query = format!("MATCH (n:Symbol {{id: '{}'}}) RETURN n.kind, n.name, n.signature, n.docstring, n.source_code", escaped_id);
                        
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
                            Err(format!("Symbol {} not found", id))
                        }
                    }
                }).await.map_err(|e| ErrorData {
                    code: ErrorCode(-32000),
                    message: format!("Task error: {}", e).into(),
                    data: None,
                })?;

                match result {
                    Ok(parsed) => {
                        let text = if as_json {
                            parsed.to_string()
                        } else {
                            let kind = parsed.get("kind").and_then(|v| v.as_str()).unwrap_or("Node");
                            let name = parsed.get("name").and_then(|v| v.as_str()).unwrap_or("");
                            let docstring = parsed.get("docstring").and_then(|v| v.as_str()).unwrap_or("");
                            let signature = parsed.get("signature").and_then(|v| v.as_str()).unwrap_or("");
                            let source = parsed.get("source_code").and_then(|v| v.as_str()).unwrap_or("");
                            
                            let mut md = format!("# {} `{}`\n\n", kind, name);
                            if !signature.is_empty() {
                                md.push_str(&format!("**Signature**:\n```\n{}\n```\n\n", signature));
                            }
                            if !docstring.is_empty() {
                                md.push_str(&format!("**Documentation**:\n{}\n\n", docstring));
                            }
                            if !source.is_empty() {
                                md.push_str(&format!("**Source Code**:\n```\n{}\n```\n", source));
                            }
                            md
                        };
                        
                        let mime_type = if as_json { "application/json" } else { "text/markdown" };
                        
                        let resource_contents = json!({
                            "uri": uri,
                            "mimeType": mime_type,
                            "text": text
                        });
                        
                        let contents = serde_json::from_value(resource_contents).unwrap_or_else(|_| {
                            ResourceContents::text(text, uri)
                        });

                        Ok(ServerResult::ReadResourceResult(ReadResourceResult::new(vec![contents])))
                    },
                    Err(e) => Err(ErrorData {
                        code: ErrorCode(-32000),
                        message: e.into(),
                        data: None,
                    }),
                }
            }
            ClientRequest::InitializeRequest(req) => {
                let res = Service::handle_request(&self.inner, ClientRequest::InitializeRequest(req), context).await;
                if let Ok(ServerResult::InitializeResult(mut info)) = res {
                    if let Ok(mut json_info) = serde_json::to_value(&info) {
                        if let Some(caps) = json_info.get_mut("capabilities") {
                            if let Some(caps_obj) = caps.as_object_mut() {
                                caps_obj.insert("resources".to_string(), json!({
                                    "listChanged": false,
                                    "subscribe": false
                                }));
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
                    caps_obj.insert("resources".to_string(), json!({
                        "listChanged": false,
                        "subscribe": false
                    }));
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
