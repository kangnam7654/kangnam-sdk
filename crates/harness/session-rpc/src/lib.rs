//! JSON-RPC handlers and dispatcher for the session module.
//!
//! The handlers are transport-agnostic: they take a borrowed
//! [`DispatchContext`] holding references to the resources they need
//! (CLI manager, DB, pending-permission map, sink factory). The host
//! application — Tauri, plain server, test harness — assembles the
//! context from its own state and calls [`dispatch`].
//!
//! Methods handled:
//! - `cli.listProviders` — known providers from the `session-agent` registry
//! - `cli.checkInstalled` — `claude` / `codex` install probe
//! - `cli.listModels` — provider model list when the backend can discover it
//! - `cli.install` — invoke provider install command
//! - `cli.startSession` — spawn an agent CLI session
//! - `cli.sendMessage` — write a user message to a running session
//! - `cli.permissionResponse` — answer a pending permission request
//! - `cli.questionFormResponse` — answer a pending host interaction
//!   that expects a form-shaped payload
//! - `cli.previewResult` — answer a pending host interaction that
//!   expects a preview payload
//! - `cli.stopSession` — kill a running session

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use kangnam_harness_session_agent::{AgentEventSink, CliManager};
use kangnam_harness_session_core::json_rpc::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use rusqlite::Connection;
use tokio::sync::{Mutex as AsyncMutex, oneshot};

pub mod guard;
pub mod handlers;

pub use guard::{GuardError, MessageGuard};

/// Map of in-flight permission requests waiting for a user decision.
pub type PendingPermissions = HashMap<String, oneshot::Sender<bool>>;

/// Generic map of in-flight host interactions waiting for a payload.
///
/// `session-rpc` only owns the transport method that resolves the
/// interaction. The producer may be a harness `InteractionBridge`, a
/// hand-written host flow, or an in-memory test driver.
pub type PendingInteractionResponses = HashMap<String, oneshot::Sender<serde_json::Value>>;

/// Map of in-flight `<question-form>` posts waiting for the user's
/// answers payload. Resolved by `cli.questionFormResponse`.
pub type PendingQuestionForms = PendingInteractionResponses;

/// Map of in-flight `preview` requests awaiting a screenshot + console
/// payload from the host webview. Resolved by `cli.previewResult`.
pub type PendingPreviews = PendingInteractionResponses;

/// Borrowed bundle of resources needed to dispatch chat RPC methods.
///
/// Lives only for the duration of one [`dispatch`] call. The host owns
/// the underlying values (typically inside its own AppState struct) and
/// constructs a fresh context per request.
pub struct DispatchContext<'a> {
    pub cli_manager: &'a AsyncMutex<CliManager>,
    pub db: &'a StdMutex<Connection>,
    pub pending_permissions: &'a AsyncMutex<PendingPermissions>,
    /// Optional form-response registry. Hosts that do not expose
    /// interactive form tools can leave it as `None`.
    pub pending_question_forms: Option<&'a AsyncMutex<PendingQuestionForms>>,
    /// Optional preview-response registry.
    pub pending_previews: Option<&'a AsyncMutex<PendingPreviews>>,
    /// Factory for the per-call event sink. Called once per
    /// `cli.startSession` / `cli.sendMessage` to mint a fresh
    /// `Arc<dyn AgentEventSink>` for the spawned task.
    pub make_sink: &'a (dyn Fn() -> Arc<dyn AgentEventSink> + Send + Sync),
    /// Authenticated user id, captured at WS upgrade by an
    /// [`crate::guard::MessageGuard`]-aware host. `None` for hosts
    /// that don't auth (Tauri desktop, local-only tools).
    pub user_id: Option<&'a str>,
    /// Optional pre-message check. When `Some`, the dispatcher
    /// invokes it before forwarding `cli.sendMessage` to the
    /// manager. On `Err`, the call is rejected with a structured
    /// JSON-RPC error and never reaches the LLM provider. See
    /// [`crate::guard::MessageGuard`] for typical use cases
    /// (per-turn billing, rate limiting).
    pub guard: Option<&'a dyn MessageGuard>,
}

#[cfg(test)]
mod boundary_tests {
    #[test]
    fn rpc_does_not_depend_on_legacy_chat_crates() {
        let manifest = include_str!("../Cargo.toml");

        assert!(
            !manifest.contains("kangnam-chat"),
            "session-rpc should resolve host interactions without depending on legacy chat crates"
        );
    }
}

/// Route a `JsonRpcRequest` to the right handler and wrap the result
/// in a `JsonRpcResponse`.
pub async fn dispatch(request: JsonRpcRequest, ctx: DispatchContext<'_>) -> JsonRpcResponse {
    let result = match request.method.as_str() {
        "cli.listProviders" => handlers::list_providers().await,
        "cli.checkInstalled" => handlers::check_installed(request.params, &ctx).await,
        "cli.listModels" => handlers::list_models(request.params).await,
        "cli.install" => handlers::install(request.params, &ctx).await,
        "cli.startSession" => handlers::start_session(request.params, &ctx).await,
        "cli.sendMessage" => handlers::send_message(request.params, &ctx).await,
        "cli.permissionResponse" => handlers::permission_response(request.params, &ctx).await,
        "cli.questionFormResponse" => handlers::question_form_response(request.params, &ctx).await,
        "cli.previewResult" => handlers::preview_result(request.params, &ctx).await,
        "cli.stopSession" => handlers::stop_session(request.params, &ctx).await,
        _ => Err(JsonRpcError::method_not_found()),
    };

    match result {
        Ok(value) => JsonRpcResponse::success(request.id, value),
        Err(error) => JsonRpcResponse::error(request.id, error),
    }
}
