//! Concrete JSON-RPC method handlers.
//!
//! Each handler returns `RpcResult` (a `serde_json::Value` payload or a
//! [`JsonRpcError`]). The dispatcher in [`crate::dispatch`] wraps the
//! returned payload into a full `JsonRpcResponse`.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use kangnam_harness_session_core::conversations;
use kangnam_harness_session_core::json_rpc::JsonRpcError;
use serde::Deserialize;

use crate::DispatchContext;

type RpcResult = Result<serde_json::Value, JsonRpcError>;

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub async fn list_providers() -> RpcResult {
    let providers = kangnam_harness_session_agent::registry::CliRegistry::known_providers();
    serde_json::to_value(providers).map_err(|e| JsonRpcError::internal(&e.to_string()))
}

pub async fn check_installed(
    params: Option<serde_json::Value>,
    ctx: &DispatchContext<'_>,
) -> RpcResult {
    let p: ProviderParam = parse_params(params)?;
    let manager = ctx.cli_manager.lock().await;
    let status = manager
        .check_installed(&p.provider)
        .await
        .map_err(|e| JsonRpcError::internal(&e))?;
    serde_json::to_value(status).map_err(|e| JsonRpcError::internal(&e.to_string()))
}

pub async fn list_models(params: Option<serde_json::Value>) -> RpcResult {
    let p: ProviderParam = parse_params(params)?;
    let router_provider = match p.provider.as_str() {
        "codex_cli" | "codex" => "codex_local",
        "claude" => "claude_local",
        other => other,
    };
    let models = kangnam_router::list_models(router_provider, "")
        .await
        .map_err(|e| JsonRpcError::internal(&e.to_string()))?;
    serde_json::to_value(models).map_err(|e| JsonRpcError::internal(&e.to_string()))
}

pub async fn install(params: Option<serde_json::Value>, ctx: &DispatchContext<'_>) -> RpcResult {
    let p: ProviderParam = parse_params(params)?;
    let manager = ctx.cli_manager.lock().await;
    manager
        .install_cli(&p.provider)
        .await
        .map_err(|e| JsonRpcError::internal(&e))?;
    Ok(serde_json::Value::Null)
}

pub async fn start_session(
    params: Option<serde_json::Value>,
    ctx: &DispatchContext<'_>,
) -> RpcResult {
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

    // NOTE: conversation DB row is created lazily on first message
    // (send_message), not here. Prevents empty "New Chat" entries on
    // every app launch.

    let sink = (ctx.make_sink)();
    let manager = ctx.cli_manager.lock().await;
    manager
        .start_session(&p.provider, &working_dir, &session_id, sink, p.model)
        .await
        .map_err(|e| JsonRpcError::internal(&e))?;

    Ok(serde_json::Value::String(session_id))
}

pub async fn send_message(
    params: Option<serde_json::Value>,
    ctx: &DispatchContext<'_>,
) -> RpcResult {
    let p: SendMessageParams = parse_params(params)?;

    // Pre-message guard. Runs BEFORE persistence and BEFORE the
    // manager so a rejected turn (insufficient funds, rate limit)
    // leaves no orphan user message in the DB and never bills the
    // upstream LLM. Skipped when no guard is configured — Tauri /
    // desktop hosts use this default.
    if let Some(guard) = ctx.guard {
        guard
            .check(ctx.user_id, &p.session_id, &p.message)
            .await
            .map_err(JsonRpcError::from)?;
    }

    // Lazy conversation creation + persist user message.
    {
        let db = ctx
            .db
            .lock()
            .map_err(|e| JsonRpcError::internal(&e.to_string()))?;
        let exists: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
                rusqlite::params![p.session_id],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if !exists {
            let now = now_ts();
            let _ = db.execute(
                "INSERT INTO conversations (id, title, cli_provider, created_at, updated_at) \
                 VALUES (?1, 'New Chat', 'claude', ?2, ?3)",
                rusqlite::params![p.session_id, now, now],
            );
        }
        let _ = conversations::add_message(
            &db,
            &p.session_id,
            "user",
            &p.message,
            None,
            None,
            None,
            None,
            None,
        );
        let _ = conversations::auto_title_if_needed(&db, &p.session_id, &p.message);
    }

    let sink = (ctx.make_sink)();
    let manager = ctx.cli_manager.lock().await;
    manager
        .send_message(&p.session_id, &p.message, sink)
        .await
        .map_err(|e| JsonRpcError::internal(&e))?;
    Ok(serde_json::Value::Null)
}

pub async fn permission_response(
    params: Option<serde_json::Value>,
    ctx: &DispatchContext<'_>,
) -> RpcResult {
    let p: PermissionResponseParams = parse_params(params)?;

    let tx = {
        let mut pending = ctx.pending_permissions.lock().await;
        pending.remove(&p.id)
    };

    match tx {
        Some(sender) => {
            let _ = sender.send(p.allowed);
            Ok(serde_json::Value::Null)
        }
        None => Err(JsonRpcError::internal(&format!(
            "No pending permission request with id: {}",
            p.id
        ))),
    }
}

pub async fn question_form_response(
    params: Option<serde_json::Value>,
    ctx: &DispatchContext<'_>,
) -> RpcResult {
    let p: AwaitResponseParams = parse_params(params)?;

    let map = match ctx.pending_question_forms {
        Some(m) => m,
        None => {
            return Err(JsonRpcError::internal(
                "host has no pending_question_forms map (interactive form responses not wired)",
            ));
        }
    };
    let tx = {
        let mut pending = map.lock().await;
        pending.remove(&p.id)
    };
    match tx {
        Some(sender) => {
            let _ = sender.send(p.payload);
            Ok(serde_json::Value::Null)
        }
        None => Err(JsonRpcError::internal(&format!(
            "no pending question form with id: {}",
            p.id
        ))),
    }
}

pub async fn preview_result(
    params: Option<serde_json::Value>,
    ctx: &DispatchContext<'_>,
) -> RpcResult {
    let p: AwaitResponseParams = parse_params(params)?;

    let map = match ctx.pending_previews {
        Some(m) => m,
        None => {
            return Err(JsonRpcError::internal(
                "host has no pending_previews map (preview responses not wired)",
            ));
        }
    };
    let tx = {
        let mut pending = map.lock().await;
        pending.remove(&p.id)
    };
    match tx {
        Some(sender) => {
            let _ = sender.send(p.payload);
            Ok(serde_json::Value::Null)
        }
        None => Err(JsonRpcError::internal(&format!(
            "no pending preview with id: {}",
            p.id
        ))),
    }
}

pub async fn stop_session(
    params: Option<serde_json::Value>,
    ctx: &DispatchContext<'_>,
) -> RpcResult {
    let p: SessionParam = parse_params(params)?;
    let manager = ctx.cli_manager.lock().await;
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
    working_dir: Option<String>,
    model: Option<String>,
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

/// Shared param shape for `cli.questionFormResponse` and `cli.previewResult`.
/// Both are resolve-by-id with an opaque payload.
#[derive(Deserialize)]
struct AwaitResponseParams {
    id: String,
    payload: serde_json::Value,
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

#[cfg(test)]
mod guard_dispatch_tests {
    //! End-to-end dispatcher tests that wire a [`MessageGuard`] into
    //! [`crate::dispatch`] and verify the rejected turn never reaches
    //! the manager. Uses a stub guard (no real billing) and a fresh
    //! in-memory rusqlite + empty `CliManager` so the success path
    //! can fall through to a clean session-not-found error from the
    //! manager — which proves the guard let the request through.

    use std::sync::{Arc, Mutex as StdMutex};

    use async_trait::async_trait;
    use kangnam_harness_session_agent::{AgentEventSink, CliManager};
    use rusqlite::Connection;
    use tokio::sync::Mutex as AsyncMutex;

    use crate::guard::{GuardError, MessageGuard};
    use crate::{DispatchContext, PendingPermissions, dispatch};
    use kangnam_harness_session_core::json_rpc::JsonRpcRequest;

    struct NullSink;
    impl AgentEventSink for NullSink {
        fn emit_message(&self, _msg: kangnam_harness_session_agent::UnifiedMessage) {}
    }

    struct DenyingGuard;
    #[async_trait]
    impl MessageGuard for DenyingGuard {
        async fn check(
            &self,
            _user_id: Option<&str>,
            _session_id: &str,
            _message: &str,
        ) -> Result<(), GuardError> {
            Err(GuardError::InsufficientFunds {
                required: 10,
                balance: 3,
                message: "10P required, you have 3P".into(),
            })
        }
    }

    struct AllowingGuard;
    #[async_trait]
    impl MessageGuard for AllowingGuard {
        async fn check(
            &self,
            _user_id: Option<&str>,
            _session_id: &str,
            _message: &str,
        ) -> Result<(), GuardError> {
            Ok(())
        }
    }

    fn fresh_db() -> Arc<StdMutex<Connection>> {
        let mut conn = Connection::open_in_memory().unwrap();
        kangnam_harness_session_core::migrations::run(&mut conn).unwrap();
        Arc::new(StdMutex::new(conn))
    }

    fn make_request(message: &str) -> JsonRpcRequest {
        serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "cli.sendMessage",
            "params": {
                "sessionId": "no-such-session",
                "message": message,
            }
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn guard_rejection_short_circuits_with_structured_error() {
        let cli_manager = AsyncMutex::new(CliManager::new());
        let db = fresh_db();
        let pending: AsyncMutex<PendingPermissions> = AsyncMutex::new(Default::default());
        let make_sink = || -> Arc<dyn AgentEventSink> { Arc::new(NullSink) };
        let guard = DenyingGuard;

        let ctx = DispatchContext {
            cli_manager: &cli_manager,
            db: &db,
            pending_permissions: &pending,
            pending_question_forms: None,
            pending_previews: None,
            make_sink: &make_sink,
            user_id: Some("u-1"),
            guard: Some(&guard),
        };

        let resp = dispatch(make_request("hi"), ctx).await;
        let err = resp.error.expect("guard should have rejected");
        assert_eq!(err.code, GuardError::CODE_INSUFFICIENT_FUNDS);
        let data = err.data.expect("structured detail");
        assert_eq!(data["required"], 10);
        assert_eq!(data["balance"], 3);

        // Critical: the rejected turn left no orphan user message in
        // the DB. (The handler persists the user row only AFTER the
        // guard passes — see handlers.rs.)
        let conn = db.lock().unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "guard rejection must not persist user turn");
    }

    #[tokio::test]
    async fn allowing_guard_lets_request_reach_manager() {
        let cli_manager = AsyncMutex::new(CliManager::new());
        let db = fresh_db();
        let pending: AsyncMutex<PendingPermissions> = AsyncMutex::new(Default::default());
        let make_sink = || -> Arc<dyn AgentEventSink> { Arc::new(NullSink) };
        let guard = AllowingGuard;

        let ctx = DispatchContext {
            cli_manager: &cli_manager,
            db: &db,
            pending_permissions: &pending,
            pending_question_forms: None,
            pending_previews: None,
            make_sink: &make_sink,
            user_id: Some("u-1"),
            guard: Some(&guard),
        };

        let resp = dispatch(make_request("hi"), ctx).await;
        // Guard allowed the call. Manager has no registered session,
        // so we expect SOME error — but NOT a guard error code. The
        // handler will have persisted the user turn before the
        // manager lookup failed.
        let err = resp.error.expect("manager should have errored");
        assert_ne!(err.code, GuardError::CODE_INSUFFICIENT_FUNDS);
        assert_ne!(err.code, GuardError::CODE_UNAUTHORIZED);
        assert_ne!(err.code, GuardError::CODE_RATE_LIMITED);

        // Persistence happened — verifies the guard ran BEFORE
        // persistence and ALLOWED the flow to continue.
        let conn = db.lock().unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn no_guard_skips_check_entirely() {
        let cli_manager = AsyncMutex::new(CliManager::new());
        let db = fresh_db();
        let pending: AsyncMutex<PendingPermissions> = AsyncMutex::new(Default::default());
        let make_sink = || -> Arc<dyn AgentEventSink> { Arc::new(NullSink) };

        let ctx = DispatchContext {
            cli_manager: &cli_manager,
            db: &db,
            pending_permissions: &pending,
            pending_question_forms: None,
            pending_previews: None,
            make_sink: &make_sink,
            user_id: None,
            guard: None,
        };

        // Same as the allowing-guard case — no rejection should
        // appear, persistence runs, manager lookup fails downstream.
        let resp = dispatch(make_request("hi"), ctx).await;
        let err = resp.error.expect("manager should have errored");
        assert_ne!(err.code, GuardError::CODE_INSUFFICIENT_FUNDS);

        let conn = db.lock().unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}

#[cfg(test)]
mod await_resolve_tests {
    //! Tests for `cli.questionFormResponse` and `cli.previewResult`
    //! resolve flows. These mirror `permission_response` semantics but
    //! carry an opaque JSON payload instead of a single `bool`.

    use std::sync::{Arc, Mutex as StdMutex};

    use kangnam_harness_session_agent::{AgentEventSink, CliManager};
    use kangnam_harness_session_core::json_rpc::JsonRpcRequest;
    use rusqlite::Connection;
    use tokio::sync::{Mutex as AsyncMutex, oneshot};

    use crate::{
        DispatchContext, PendingPermissions, PendingPreviews, PendingQuestionForms, dispatch,
    };

    struct NullSink;
    impl AgentEventSink for NullSink {
        fn emit_message(&self, _msg: kangnam_harness_session_agent::UnifiedMessage) {}
    }

    fn fresh_db() -> Arc<StdMutex<Connection>> {
        let mut conn = Connection::open_in_memory().unwrap();
        kangnam_harness_session_core::migrations::run(&mut conn).unwrap();
        Arc::new(StdMutex::new(conn))
    }

    #[tokio::test]
    async fn question_form_response_resolves_pending_sender() {
        let cli_manager = AsyncMutex::new(CliManager::new());
        let db = fresh_db();
        let pending_perms: AsyncMutex<PendingPermissions> = AsyncMutex::new(Default::default());
        let pending_qf: AsyncMutex<PendingQuestionForms> = AsyncMutex::new(Default::default());
        let pending_pv: AsyncMutex<PendingPreviews> = AsyncMutex::new(Default::default());
        let make_sink = || -> Arc<dyn AgentEventSink> { Arc::new(NullSink) };

        let (tx, rx) = oneshot::channel::<serde_json::Value>();
        pending_qf.lock().await.insert("await-q-1".into(), tx);

        let req: JsonRpcRequest = serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0", "id": 1,
            "method": "cli.questionFormResponse",
            "params": { "id": "await-q-1", "payload": { "answers": { "tone": "calm" } } }
        }))
        .unwrap();

        let ctx = DispatchContext {
            cli_manager: &cli_manager,
            db: &db,
            pending_permissions: &pending_perms,
            pending_question_forms: Some(&pending_qf),
            pending_previews: Some(&pending_pv),
            make_sink: &make_sink,
            user_id: None,
            guard: None,
        };
        let resp = dispatch(req, ctx).await;
        assert!(resp.error.is_none());

        let payload = rx.await.expect("sender should have fired");
        assert_eq!(payload["answers"]["tone"], "calm");
    }

    #[tokio::test]
    async fn preview_result_resolves_pending_sender() {
        let cli_manager = AsyncMutex::new(CliManager::new());
        let db = fresh_db();
        let pending_perms: AsyncMutex<PendingPermissions> = AsyncMutex::new(Default::default());
        let pending_qf: AsyncMutex<PendingQuestionForms> = AsyncMutex::new(Default::default());
        let pending_pv: AsyncMutex<PendingPreviews> = AsyncMutex::new(Default::default());
        let make_sink = || -> Arc<dyn AgentEventSink> { Arc::new(NullSink) };

        let (tx, rx) = oneshot::channel::<serde_json::Value>();
        pending_pv.lock().await.insert("await-p-1".into(), tx);

        let req: JsonRpcRequest = serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0", "id": 2,
            "method": "cli.previewResult",
            "params": { "id": "await-p-1", "payload": { "screenshot": "/tmp/x.png", "console": [] } }
        })).unwrap();

        let ctx = DispatchContext {
            cli_manager: &cli_manager,
            db: &db,
            pending_permissions: &pending_perms,
            pending_question_forms: Some(&pending_qf),
            pending_previews: Some(&pending_pv),
            make_sink: &make_sink,
            user_id: None,
            guard: None,
        };
        let resp = dispatch(req, ctx).await;
        assert!(resp.error.is_none());

        let payload = rx.await.expect("sender should have fired");
        assert_eq!(payload["screenshot"], "/tmp/x.png");
    }

    #[tokio::test]
    async fn missing_pending_map_yields_error() {
        let cli_manager = AsyncMutex::new(CliManager::new());
        let db = fresh_db();
        let pending_perms: AsyncMutex<PendingPermissions> = AsyncMutex::new(Default::default());
        let make_sink = || -> Arc<dyn AgentEventSink> { Arc::new(NullSink) };

        let req: JsonRpcRequest = serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0", "id": 3,
            "method": "cli.questionFormResponse",
            "params": { "id": "anything", "payload": {} }
        }))
        .unwrap();

        let ctx = DispatchContext {
            cli_manager: &cli_manager,
            db: &db,
            pending_permissions: &pending_perms,
            pending_question_forms: None,
            pending_previews: None,
            make_sink: &make_sink,
            user_id: None,
            guard: None,
        };
        let resp = dispatch(req, ctx).await;
        assert!(resp.error.is_some());
        let err = resp.error.unwrap();
        assert!(err.message.contains("interactive form responses not wired"));
    }

    #[tokio::test]
    async fn unknown_id_yields_error() {
        let cli_manager = AsyncMutex::new(CliManager::new());
        let db = fresh_db();
        let pending_perms: AsyncMutex<PendingPermissions> = AsyncMutex::new(Default::default());
        let pending_qf: AsyncMutex<PendingQuestionForms> = AsyncMutex::new(Default::default());
        let pending_pv: AsyncMutex<PendingPreviews> = AsyncMutex::new(Default::default());
        let make_sink = || -> Arc<dyn AgentEventSink> { Arc::new(NullSink) };

        let req: JsonRpcRequest = serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0", "id": 4,
            "method": "cli.previewResult",
            "params": { "id": "no-such-await", "payload": {} }
        }))
        .unwrap();

        let ctx = DispatchContext {
            cli_manager: &cli_manager,
            db: &db,
            pending_permissions: &pending_perms,
            pending_question_forms: Some(&pending_qf),
            pending_previews: Some(&pending_pv),
            make_sink: &make_sink,
            user_id: None,
            guard: None,
        };
        let resp = dispatch(req, ctx).await;
        assert!(resp.error.is_some());
        assert!(resp.error.unwrap().message.contains("no pending preview"));
    }
}
