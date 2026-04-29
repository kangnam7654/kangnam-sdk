//! MCP (Model Context Protocol) endpoint exposing a single `approve`
//! tool that Claude Code calls before executing risky actions.
//!
//! The tool blocks (up to 5 minutes) on a `oneshot::Sender<bool>`
//! placed in [`ServerContext::pending_permissions`]; the host's UI
//! resolves the request through the `cli.permissionResponse` JSON-RPC
//! method (handled in [`kangnam_chat_rpc`]).

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use kangnam_chat_core::json_rpc::JsonRpcNotification;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ServerContext;

#[derive(Clone)]
pub(crate) struct McpState {
    pub ctx: Arc<ServerContext>,
    pub server_name: String,
    pub server_version: String,
    pub tool_namespace: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct McpRequest {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct McpResponse {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<serde_json::Value>,
}

impl McpResponse {
    fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Option<serde_json::Value>, code: i32, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(json!({ "code": code, "message": message })),
        }
    }
}

/// `GET /mcp` — SSE handshake for the MCP Streamable HTTP protocol.
pub(crate) async fn mcp_sse_handler() -> impl IntoResponse {
    use axum::response::sse::{Event, Sse};
    use futures::stream;
    use std::convert::Infallible;

    let init_event = Event::default().event("endpoint").data("/mcp");

    Sse::new(stream::once(async move {
        Ok::<_, Infallible>(init_event)
    }))
}

/// `POST /mcp` — JSON-RPC entrypoint for `initialize`, `tools/list`,
/// `tools/call`.
pub(crate) async fn mcp_handler(
    State(state): State<Arc<McpState>>,
    Json(req): Json<McpRequest>,
) -> impl IntoResponse {
    match req.method.as_str() {
        "initialize" => handle_initialize(&state, req.id).into_response(),
        "notifications/initialized" => (StatusCode::OK, "").into_response(),
        "tools/list" => handle_tools_list(req.id).into_response(),
        "tools/call" => handle_tools_call(state, req.id, req.params).await.into_response(),
        _ => Json(McpResponse::error(req.id, -32601, "Method not found")).into_response(),
    }
}

fn handle_initialize(state: &McpState, id: Option<serde_json::Value>) -> Json<McpResponse> {
    Json(McpResponse::success(
        id,
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": state.server_name,
                "version": state.server_version,
            }
        }),
    ))
}

fn handle_tools_list(id: Option<serde_json::Value>) -> Json<McpResponse> {
    Json(McpResponse::success(
        id,
        json!({
            "tools": [
                {
                    "name": "approve",
                    "description": "Request user approval for a tool execution",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "tool_name": {
                                "type": "string",
                                "description": "Name of the tool requesting approval"
                            },
                            "tool_input": {
                                "type": "object",
                                "description": "Input parameters of the tool requesting approval"
                            },
                            "description": {
                                "type": "string",
                                "description": "Human-readable description of what the tool will do"
                            }
                        },
                        "required": ["tool_name", "tool_input", "description"]
                    }
                }
            ]
        }),
    ))
}

async fn handle_tools_call(
    state: Arc<McpState>,
    id: Option<serde_json::Value>,
    params: Option<serde_json::Value>,
) -> Json<McpResponse> {
    let _ = &state.tool_namespace; // reserved for future multi-tool routing

    let params = match params {
        Some(p) => p,
        None => return Json(McpResponse::error(id, -32602, "Missing params")),
    };

    let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");

    if tool_name != "approve" {
        return Json(McpResponse::error(
            id,
            -32602,
            &format!("Unknown tool: {}", tool_name),
        ));
    }

    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    let permission_id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();

    {
        let mut pending = state.ctx.pending_permissions.lock().await;
        pending.insert(permission_id.clone(), tx);
    }

    let notification = JsonRpcNotification::new(
        "cli.permissionRequest",
        json!({
            "id": permission_id,
            "tool_name": arguments.get("tool_name").cloned().unwrap_or(json!(null)),
            "tool_input": arguments.get("tool_input").cloned().unwrap_or(json!(null)),
            "description": arguments.get("description").cloned().unwrap_or(json!(null)),
        }),
    );

    let _ = state.ctx.broadcast_tx.send(notification);

    let result = tokio::time::timeout(std::time::Duration::from_secs(300), rx).await;

    {
        let mut pending = state.ctx.pending_permissions.lock().await;
        pending.remove(&permission_id);
    }

    match result {
        Ok(Ok(approved)) => Json(McpResponse::success(
            id,
            json!({
                "content": [
                    { "type": "text", "text": json!({ "approved": approved }).to_string() }
                ]
            }),
        )),
        Ok(Err(_)) => Json(McpResponse::error(
            id,
            -32603,
            "Permission request was cancelled",
        )),
        Err(_) => Json(McpResponse::error(id, -32603, "Permission request timed out")),
    }
}
