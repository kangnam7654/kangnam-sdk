//! JSON-RPC dispatch glue.
//!
//! Delegates to the `chat-rpc` crate, building a
//! [`chat_rpc::DispatchContext`] from this app's `AppState`. The
//! event sink is `chat_server::broadcast::BroadcastSink`, which fans
//! agent events out as JSON-RPC notifications on the broadcast
//! channels owned by `AppState`.
//!
//! Currently used only by Tauri-internal tests / fallback paths;
//! the running WebSocket server has its own dispatch loop in
//! `chat-server`.

use std::sync::Arc;

use chat_server::broadcast::BroadcastSink;

use crate::state::AppState;

#[allow(dead_code)]
pub async fn dispatch(
    request: chat_core::json_rpc::JsonRpcRequest,
    state: &AppState,
) -> chat_core::json_rpc::JsonRpcResponse {
    let make_sink = || -> Arc<dyn chat_agent::AgentEventSink> {
        Arc::new(BroadcastSink::new(
            state.broadcast_tx.clone(),
            Some(state.enhanced_broadcast_tx.clone()),
        ))
    };
    let ctx = chat_rpc::DispatchContext {
        cli_manager: &state.cli_manager,
        db: &state.db,
        pending_permissions: &state.pending_permissions,
        make_sink: &make_sink,
    };
    chat_rpc::dispatch(request, ctx).await
}
