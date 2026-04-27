use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use crate::rpc;
use crate::rpc::types::JsonRpcRequest;
use crate::state::AppState;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Channel for sending messages back to the client (responses + notifications)
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<String>(64);

    // Task 1: outbound channel → WebSocket sender
    let send_task = tokio::spawn(async move {
        while let Some(text) = outbound_rx.recv().await {
            if ws_sender.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    // Task 2: broadcast notifications → outbound channel
    let mut broadcast_rx = state.broadcast_tx.subscribe();
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

    // Task 3: enhanced broadcast notifications → outbound channel
    let mut enhanced_rx = state.enhanced_broadcast_tx.subscribe();
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

    // Main loop: receive JSON-RPC requests → dispatch → send response via outbound channel
    while let Some(Ok(msg)) = ws_receiver.next().await {
        match msg {
            Message::Text(text) => {
                let text_str: &str = &text;
                if let Ok(request) = serde_json::from_str::<JsonRpcRequest>(text_str) {
                    let response = rpc::dispatch(request, &state).await;
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

    // Cleanup
    enhanced_task.abort();
    notify_task.abort();
    send_task.abort();
}
