//! JSON-RPC dispatch glue.
//!
//! The handlers and dispatcher live in the `chat-rpc` crate. This
//! module re-exports the JSON-RPC types under the original
//! `crate::rpc::types` path and provides [`dispatch`] — a thin
//! adapter that builds a [`chat_rpc::DispatchContext`] from this
//! app's `AppState` and forwards to `chat_rpc::dispatch`.

pub use chat_core::json_rpc as types;

use std::sync::Arc;

use crate::cli::broadcast_sink::BroadcastSink;
use crate::state::AppState;

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
