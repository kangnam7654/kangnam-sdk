use std::sync::Arc;
use tauri::State;

use crate::mcp::types::*;
use crate::state::AppState;

const VALID_TRANSPORT_TYPES: &[&str] = &["stdio", "http", "sse"];

fn validate_server_config(config: &ServerConfig) -> Result<(), String> {
    if !VALID_TRANSPORT_TYPES.contains(&config.transport_type.as_str()) {
        return Err(format!(
            "Invalid transport type '{}'. Must be one of: stdio, http, sse",
            config.transport_type
        ));
    }
    if config.transport_type == "stdio" {
        match config.command.as_deref() {
            None | Some("") => {
                return Err(
                    "Command is required for stdio transport and cannot be empty".to_string(),
                );
            }
            _ => {}
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn mcp_list_servers(state: State<'_, Arc<AppState>>) -> Result<Vec<ServerStatus>, String> {
    state.mcp.server_status().await
}

#[tauri::command]
pub async fn mcp_add_server(
    config: ServerConfig,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    validate_server_config(&config)?;
    state
        .mcp
        .request("mcp:add-server", serde_json::to_value(&config).map_err(|_| "Failed to serialize server config".to_string())?)
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn mcp_reconnect_server(
    name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    state
        .mcp
        .request("mcp:reconnect", serde_json::json!({ "name": name }))
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn mcp_update_server(
    old_name: String,
    config: ServerConfig,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    validate_server_config(&config)?;
    state
        .mcp
        .request(
            "mcp:update-server",
            serde_json::json!({ "oldName": old_name, "config": config }),
        )
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn mcp_remove_server(name: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state
        .mcp
        .request("mcp:remove-server", serde_json::json!({ "name": name }))
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn mcp_list_tools(state: State<'_, Arc<AppState>>) -> Result<Vec<AggregatedTool>, String> {
    state.mcp.list_tools().await
}

#[tauri::command]
pub async fn mcp_server_status(state: State<'_, Arc<AppState>>) -> Result<Vec<ServerStatus>, String> {
    state.mcp.server_status().await
}

#[tauri::command]
pub async fn mcp_get_config(
    name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let configs = state
        .mcp
        .request("mcp:server-configs", serde_json::json!({}))
        .await?;
    let configs: Vec<ServerConfig> =
        serde_json::from_value(configs).map_err(|_| "Invalid server configuration data".to_string())?;
    configs
        .into_iter()
        .find(|c| c.name == name)
        .ok_or(format!("Server '{name}' not found"))
        .and_then(|c| serde_json::to_value(c).map_err(|_| "Failed to serialize server config".to_string()))
}

#[tauri::command]
pub async fn mcp_ai_assist(
    _prompt: String,
    _provider: String,
    _model: Option<String>,
    _state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    // Phase 6: AI-assisted MCP configuration
    Err("AI assist not yet implemented".to_string())
}
