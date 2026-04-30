//! JSON-RPC handlers and dispatcher for the chat module.
//!
//! The handlers are transport-agnostic: they take a borrowed
//! [`DispatchContext`] holding references to the resources they need
//! (CLI manager, DB, pending-permission map, sink factory). The host
//! application — Tauri, plain server, test harness — assembles the
//! context from its own state and calls [`dispatch`].
//!
//! Methods handled:
//! - `cli.listProviders` — known providers from the `chat-agent` registry
//! - `cli.checkInstalled` — `claude` / `codex` install probe
//! - `cli.install` — invoke provider install command
//! - `cli.startSession` — spawn an agent CLI session
//! - `cli.sendMessage` — write a user message to a running session
//! - `cli.permissionResponse` — answer a pending permission request
//! - `cli.questionFormResponse` — answer a pending `<question-form>`
//!   posted by the design `ask` tool (Phase 4c)
//! - `cli.previewResult` — deliver a screenshot + console output back
//!   to the design `preview` tool (Phase 4c)
//! - `cli.stopSession` — kill a running session

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use kangnam_chat_agent::{AgentEventSink, CliManager};
use kangnam_chat_core::json_rpc::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use rusqlite::Connection;
use tokio::sync::{oneshot, Mutex as AsyncMutex};

pub mod guard;
pub mod handlers;

pub use guard::{GuardError, MessageGuard};

/// Map of in-flight permission requests waiting for a user decision.
pub type PendingPermissions = HashMap<String, oneshot::Sender<bool>>;

/// Map of in-flight `<question-form>` posts waiting for the user's
/// answers payload. Keyed by the `await_id` minted by the
/// `kangnam_harness_runtime::InteractionBridge::register_question_form`
/// implementation; resolved by `cli.questionFormResponse`.
pub type PendingQuestionForms = HashMap<String, oneshot::Sender<serde_json::Value>>;

/// Map of in-flight `preview` requests awaiting a screenshot + console
/// payload from the host webview. Keyed by `await_id` minted by
/// `kangnam_harness_runtime::InteractionBridge::register_preview`;
/// resolved by `cli.previewResult`.
pub type PendingPreviews = HashMap<String, oneshot::Sender<serde_json::Value>>;

/// Borrowed bundle of resources needed to dispatch chat RPC methods.
///
/// Lives only for the duration of one [`dispatch`] call. The host owns
/// the underlying values (typically inside its own AppState struct) and
/// constructs a fresh context per request.
pub struct DispatchContext<'a> {
    pub cli_manager: &'a AsyncMutex<CliManager>,
    pub db: &'a StdMutex<Connection>,
    pub pending_permissions: &'a AsyncMutex<PendingPermissions>,
    /// Phase 4c — design `ask` tool resolves through this map.
    /// `None` for hosts that don't run design tools (server smoke tests).
    pub pending_question_forms: Option<&'a AsyncMutex<PendingQuestionForms>>,
    /// Phase 4c — design `preview` tool resolves through this map.
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

/// Route a `JsonRpcRequest` to the right handler and wrap the result
/// in a `JsonRpcResponse`.
pub async fn dispatch(request: JsonRpcRequest, ctx: DispatchContext<'_>) -> JsonRpcResponse {
    let result = match request.method.as_str() {
        "cli.listProviders" => handlers::list_providers().await,
        "cli.checkInstalled" => handlers::check_installed(request.params, &ctx).await,
        "cli.install" => handlers::install(request.params, &ctx).await,
        "cli.startSession" => handlers::start_session(request.params, &ctx).await,
        "cli.sendMessage" => handlers::send_message(request.params, &ctx).await,
        "cli.permissionResponse" => handlers::permission_response(request.params, &ctx).await,
        "cli.questionFormResponse" => {
            handlers::question_form_response(request.params, &ctx).await
        }
        "cli.previewResult" => handlers::preview_result(request.params, &ctx).await,
        "cli.stopSession" => handlers::stop_session(request.params, &ctx).await,
        _ => Err(JsonRpcError::method_not_found()),
    };

    match result {
        Ok(value) => JsonRpcResponse::success(request.id, value),
        Err(error) => JsonRpcResponse::error(request.id, error),
    }
}
