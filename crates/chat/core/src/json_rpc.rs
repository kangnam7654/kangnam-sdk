#![allow(dead_code)]

//! JSON-RPC 2.0 message types used between the chat client and server.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Clone)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Clone)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Server → Client notification (no id, no response expected)
#[derive(Debug, Serialize, Clone)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<serde_json::Value>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

impl JsonRpcNotification {
    pub fn new(method: &str, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: Some(params),
        }
    }
}

impl JsonRpcError {
    pub fn method_not_found() -> Self {
        Self {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
        }
    }
    pub fn invalid_params(msg: &str) -> Self {
        Self {
            code: -32602,
            message: msg.to_string(),
            data: None,
        }
    }
    pub fn internal(msg: &str) -> Self {
        Self {
            code: -32603,
            message: msg.to_string(),
            data: None,
        }
    }
    pub fn provider_not_found(provider: &str) -> Self {
        Self {
            code: -32001,
            message: format!("Unknown provider: {}", provider),
            data: None,
        }
    }
    pub fn session_not_found(id: &str) -> Self {
        Self {
            code: -32002,
            message: format!("Session not found: {}", id),
            data: None,
        }
    }
    pub fn dir_not_found(path: &str) -> Self {
        Self {
            code: -32005,
            message: format!("Directory does not exist: {}", path),
            data: None,
        }
    }
}
