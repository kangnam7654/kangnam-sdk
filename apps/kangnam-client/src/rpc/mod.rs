//! JSON-RPC dispatch glue.
//!
//! Delegates to the `session-rpc` crate, building a
//! [`kangnam_harness_session::rpc::DispatchContext`] from this app's `AppState`. The
//! event sink is `kangnam_harness_session::server::broadcast::BroadcastSink`, which fans
//! agent events out as JSON-RPC notifications on the broadcast
//! channels owned by `AppState`.
//!
//! Currently used only by Tauri-internal tests / fallback paths;
//! the running WebSocket server has its own dispatch loop in
//! `session-server`.

use std::sync::Arc;

use kangnam_harness_session::server::broadcast::BroadcastSink;

use crate::state::AppState;

#[allow(dead_code)]
pub async fn dispatch(
    request: kangnam_harness_session::core::json_rpc::JsonRpcRequest,
    state: &AppState,
) -> kangnam_harness_session::core::json_rpc::JsonRpcResponse {
    let make_sink = || -> Arc<dyn kangnam_harness_session::agent::AgentEventSink> {
        Arc::new(BroadcastSink::new(
            state.broadcast_tx.clone(),
            Some(state.enhanced_broadcast_tx.clone()),
        ))
    };
    let ctx = kangnam_harness_session::rpc::DispatchContext {
        cli_manager: &state.cli_manager,
        db: &state.db,
        pending_permissions: &state.pending_permissions,
        // Phase 3 design-mode suspend/resume infra not wired on Tauri yet —
        // question-form / preview tools land via WS server context only.
        pending_question_forms: None,
        pending_previews: None,
        make_sink: &make_sink,
        // Tauri desktop is single-user / local-only — no per-user auth or
        // per-turn billing guard. Hosts that need either pass `Some(...)`.
        user_id: None,
        guard: None,
    };
    kangnam_harness_session::rpc::dispatch(request, ctx).await
}
