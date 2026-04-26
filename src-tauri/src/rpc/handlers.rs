use serde::Deserialize;
use std::path::PathBuf;

use crate::cli::registry::CliRegistry;
use crate::db::conversations;
use crate::rpc::types::JsonRpcError;
use crate::state::AppState;
use chrono;
use rusqlite;

type RpcResult = Result<serde_json::Value, JsonRpcError>;

pub async fn list_providers() -> RpcResult {
    let providers = CliRegistry::known_providers();
    Ok(serde_json::to_value(providers).map_err(|e| JsonRpcError::internal(&e.to_string()))?)
}

pub async fn check_installed(params: Option<serde_json::Value>, state: &AppState) -> RpcResult {
    let p: ProviderParam = parse_params(params)?;
    let manager = state.cli_manager.lock().await;
    let status = manager
        .check_installed(&p.provider)
        .await
        .map_err(|e| JsonRpcError::internal(&e))?;
    Ok(serde_json::to_value(status).map_err(|e| JsonRpcError::internal(&e.to_string()))?)
}

pub async fn install(params: Option<serde_json::Value>, state: &AppState) -> RpcResult {
    let p: ProviderParam = parse_params(params)?;
    let manager = state.cli_manager.lock().await;
    manager
        .install_cli(&p.provider)
        .await
        .map_err(|e| JsonRpcError::internal(&e))?;
    Ok(serde_json::Value::Null)
}

pub async fn start_session(params: Option<serde_json::Value>, state: &AppState) -> RpcResult {
    let p: StartSessionParams = parse_params(params)?;

    let working_dir = match p.working_dir {
        Some(ref dir) => {
            let path = PathBuf::from(dir);
            if !path.is_dir() {
                return Err(JsonRpcError::dir_not_found(dir));
            }
            path
        }
        None => dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp")),
    };

    let session_id = uuid::Uuid::new_v4().to_string();

    // NOTE: conversation DB row is created lazily on first message (send_message),
    // not here. This prevents empty "New Chat" entries on every app launch.

    let sink: std::sync::Arc<dyn chat_agent::AgentEventSink> = std::sync::Arc::new(
        crate::cli::broadcast_sink::BroadcastSink::new(
            state.broadcast_tx.clone(),
            Some(state.enhanced_broadcast_tx.clone()),
        ),
    );
    let manager = state.cli_manager.lock().await;
    manager
        .start_session(&p.provider, &working_dir, &session_id, sink, p.model)
        .await
        .map_err(|e| JsonRpcError::internal(&e))?;

    Ok(serde_json::Value::String(session_id))
}

pub async fn send_message(params: Option<serde_json::Value>, state: &AppState) -> RpcResult {
    let p: SendMessageParams = parse_params(params)?;

    // Create conversation in DB if it doesn't exist yet (lazy creation)
    // Then save user message
    {
        let db = state.db.lock().map_err(|e| JsonRpcError::internal(&e.to_string()))?;
        let exists: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
            rusqlite::params![p.session_id],
            |r| r.get(0),
        ).unwrap_or(false);
        if !exists {
            let now = chrono::Utc::now().timestamp();
            let _ = db.execute(
                "INSERT INTO conversations (id, title, cli_provider, created_at, updated_at) \
                 VALUES (?1, 'New Chat', 'claude', ?2, ?3)",
                rusqlite::params![p.session_id, now, now],
            );
        }
        let _ = conversations::add_message(
            &db, &p.session_id, "user", &p.message,
            None, None, None, None, None,
        );
        let _ = conversations::auto_title_if_needed(&db, &p.session_id, &p.message);
    }

    let sink: std::sync::Arc<dyn chat_agent::AgentEventSink> = std::sync::Arc::new(
        crate::cli::broadcast_sink::BroadcastSink::new(
            state.broadcast_tx.clone(),
            Some(state.enhanced_broadcast_tx.clone()),
        ),
    );
    let manager = state.cli_manager.lock().await;
    manager
        .send_message(&p.session_id, &p.message, sink)
        .await
        .map_err(|e| JsonRpcError::internal(&e))?;
    Ok(serde_json::Value::Null)
}

pub async fn permission_response(params: Option<serde_json::Value>, state: &AppState) -> RpcResult {
    let p: PermissionResponseParams = parse_params(params)?;

    let tx = {
        let mut pending = state.pending_permissions.lock().await;
        pending.remove(&p.id)
    };

    match tx {
        Some(sender) => {
            let _ = sender.send(p.allowed);
            Ok(serde_json::Value::Null)
        }
        None => Err(JsonRpcError::internal(&format!(
            "No pending permission request with id: {}", p.id
        ))),
    }
}

pub async fn stop_session(params: Option<serde_json::Value>, state: &AppState) -> RpcResult {
    let p: SessionParam = parse_params(params)?;
    let manager = state.cli_manager.lock().await;
    manager
        .stop_session(&p.session_id)
        .await
        .map_err(|e| JsonRpcError::internal(&e))?;
    Ok(serde_json::Value::Null)
}

// -- Param types --

#[derive(Deserialize)]
struct ProviderParam {
    provider: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartSessionParams {
    provider: String,
    working_dir: Option<String>, // Optional — None = chat mode (no directory)
    model: Option<String>,       // Optional — None = use CLI default model
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendMessageParams {
    session_id: String,
    message: String,
}

#[derive(Deserialize)]
struct PermissionResponseParams {
    id: String,
    allowed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionParam {
    session_id: String,
}

fn parse_params<T: serde::de::DeserializeOwned>(
    params: Option<serde_json::Value>,
) -> Result<T, JsonRpcError> {
    let value = params.unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
    serde_json::from_value(value).map_err(|e| JsonRpcError::invalid_params(&e.to_string()))
}
