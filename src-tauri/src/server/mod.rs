//! HTTP server boot — delegates to the `chat-server` crate.
//!
//! This module builds a [`chat_server::ServerContext`] from this app's
//! `AppState` and a [`chat_server::ServerConfig`] from env / build
//! constants, then calls [`chat_server::start`].

use std::sync::Arc;

use crate::state::AppState;

pub async fn start_server(
    state: Arc<AppState>,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ctx = Arc::new(chat_server::ServerContext {
        cli_manager: state.cli_manager.clone(),
        db: state.db.clone(),
        pending_permissions: state.pending_permissions.clone(),
        broadcast_tx: state.broadcast_tx.clone(),
        enhanced_broadcast_tx: state.enhanced_broadcast_tx.clone(),
    });
    let config = chat_server::ServerConfig {
        port,
        static_dir: std::env::var("KANGNAM_STATIC_DIR").ok(),
        server_name: "kangnam-client".to_string(),
        server_version: env!("CARGO_PKG_VERSION").to_string(),
        mcp_tool_namespace: "kangnam".to_string(),
    };
    chat_server::start(ctx, config).await
}
