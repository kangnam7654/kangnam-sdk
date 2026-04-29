//! WebSocket handler — multiplexes JSON-RPC requests/responses with
//! broadcast notifications on a single socket per client.
//!
//! Authentication, when configured, runs once at upgrade time. The
//! resulting [`crate::UserContext`] (specifically its `user_id`) is
//! threaded into every JSON-RPC dispatch so a [`kangnam_chat_rpc::MessageGuard`]
//! downstream can use it for billing, rate limits, etc.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket},
        Query, State, WebSocketUpgrade,
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use kangnam_chat_core::json_rpc::JsonRpcRequest;
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use crate::auth::{AuthParams, UserContext};
use crate::broadcast::BroadcastSink;
use crate::ServerContext;

pub(crate) async fn ws_handler(
    ws: WebSocketUpgrade,
    State(ctx): State<Arc<ServerContext>>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let user_ctx = match resolve_user(&ctx, &query, &headers).await {
        Ok(uc) => uc,
        Err(status) => return status.into_response(),
    };

    ws.on_upgrade(move |socket| handle_socket(socket, ctx, user_ctx))
}

/// Run the auth hook (if configured) and convert any error into a
/// 401 response. `Ok(None)` means no hook is set — the connection is
/// allowed through with no user identity.
async fn resolve_user(
    ctx: &ServerContext,
    query: &HashMap<String, String>,
    headers: &HeaderMap,
) -> Result<Option<UserContext>, StatusCode> {
    let Some(hook) = ctx.auth_hook.as_ref() else {
        return Ok(None);
    };

    let header_map: HashMap<String, String> = headers
        .iter()
        .filter_map(|(k, v)| Some((k.as_str().to_lowercase(), v.to_str().ok()?.to_string())))
        .collect();
    let params = AuthParams {
        query,
        headers: &header_map,
    };

    match hook.authenticate(params).await {
        Ok(uc) => Ok(Some(uc)),
        Err(_) => Err(StatusCode::UNAUTHORIZED),
    }
}

async fn handle_socket(
    socket: WebSocket,
    ctx: Arc<ServerContext>,
    user: Option<UserContext>,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Internal channel: handlers + broadcast forwarders → WS sender.
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<String>(64);

    let send_task = tokio::spawn(async move {
        while let Some(text) = outbound_rx.recv().await {
            if ws_sender.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    // Forward unified-event broadcast → outbound.
    let mut broadcast_rx = ctx.broadcast_tx.subscribe();
    let notify_tx = outbound_tx.clone();
    let notify_task = tokio::spawn(async move {
        while let Ok(notification) = broadcast_rx.recv().await {
            if let Ok(text) = serde_json::to_string(&notification) {
                if notify_tx.send(text).await.is_err() {
                    break;
                }
            }
        }
    });

    // Forward Claude-enhanced broadcast → outbound.
    let mut enhanced_rx = ctx.enhanced_broadcast_tx.subscribe();
    let enhanced_notify_tx = outbound_tx.clone();
    let enhanced_task = tokio::spawn(async move {
        while let Ok(notification) = enhanced_rx.recv().await {
            if let Ok(text) = serde_json::to_string(&notification) {
                if enhanced_notify_tx.send(text).await.is_err() {
                    break;
                }
            }
        }
    });

    // Captured once per socket; cheap `&str` slice on every dispatch.
    let user_id = user.as_ref().map(|u| u.user_id.clone());

    // Main loop: receive JSON-RPC → dispatch via chat-rpc → reply.
    while let Some(Ok(msg)) = ws_receiver.next().await {
        match msg {
            Message::Text(text) => {
                let text_str: &str = &text;
                if let Ok(request) = serde_json::from_str::<JsonRpcRequest>(text_str) {
                    let make_sink = || -> Arc<dyn kangnam_chat_agent::AgentEventSink> {
                        Arc::new(BroadcastSink::new(
                            ctx.broadcast_tx.clone(),
                            Some(ctx.enhanced_broadcast_tx.clone()),
                        ))
                    };
                    let dispatch_ctx = kangnam_chat_rpc::DispatchContext {
                        cli_manager: &ctx.cli_manager,
                        db: &ctx.db,
                        pending_permissions: &ctx.pending_permissions,
                        make_sink: &make_sink,
                        user_id: user_id.as_deref(),
                        guard: ctx.message_guard.as_deref(),
                    };
                    let response = kangnam_chat_rpc::dispatch(request, dispatch_ctx).await;
                    if let Ok(response_text) = serde_json::to_string(&response) {
                        if outbound_tx.send(response_text).await.is_err() {
                            break;
                        }
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    enhanced_task.abort();
    notify_task.abort();
    send_task.abort();
}
