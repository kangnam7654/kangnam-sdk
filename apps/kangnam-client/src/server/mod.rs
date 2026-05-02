//! HTTP server boot — delegates to the `chat-server` crate.
//!
//! This module builds a [`kangnam_chat::server::ServerContext`] from this app's
//! `AppState` and a [`kangnam_chat::server::ServerConfig`] from env / build
//! constants, then calls [`kangnam_chat::server::start`].

use std::sync::Arc;

use crate::state::AppState;

pub async fn start_server(
    state: Arc<AppState>,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ctx = Arc::new(kangnam_chat::server::ServerContext {
        cli_manager: state.cli_manager.clone(),
        db: state.db.clone(),
        pending_permissions: state.pending_permissions.clone(),
        // Phase 3 design-mode suspend/resume infra: question-form + preview
        // tool turn-suspend maps. Defaulted to None — Tauri desktop will
        // initialize these when frontend wires the design tool flow.
        pending_question_forms: None,
        pending_previews: None,
        broadcast_tx: state.broadcast_tx.clone(),
        enhanced_broadcast_tx: state.enhanced_broadcast_tx.clone(),
        // Tauri desktop is single-user / local-only — no WS upgrade auth or
        // per-turn billing guard. Hosts that need either pass `Some(...)`.
        auth_hook: None,
        message_guard: None,
    });
    let config = kangnam_chat::server::ServerConfig {
        port,
        static_dir: std::env::var("KANGNAM_STATIC_DIR").ok(),
        server_name: "kangnam-client".to_string(),
        server_version: env!("CARGO_PKG_VERSION").to_string(),
        mcp_tool_namespace: "kangnam".to_string(),
    };
    kangnam_chat::server::start(ctx, config).await
}
