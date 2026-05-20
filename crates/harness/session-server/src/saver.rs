//! Background task: subscribes to the unified-event broadcast and
//! persists assistant text to the conversation DB on `TurnEnd`.

use std::sync::Arc;

use kangnam_harness_session_agent::types::UnifiedMessage;
use kangnam_harness_session_core::conversations;

use crate::ServerContext;

/// Spawn the message-saver task. Returns immediately; the task lives
/// for the duration of the broadcast channel.
///
/// Listens to `cli.stream` notifications, accumulates `TextDelta` text
/// into a buffer, and writes the buffer as a single assistant message
/// to the most recent conversation when `TurnEnd` arrives.
pub fn start_message_saver(ctx: Arc<ServerContext>) {
    let mut rx = ctx.broadcast_tx.subscribe();

    tokio::spawn(async move {
        let mut text_buffer = String::new();

        loop {
            match rx.recv().await {
                Ok(notification) => {
                    if notification.method != "cli.stream" {
                        continue;
                    }
                    let params = match notification.params {
                        Some(ref p) => p,
                        None => continue,
                    };
                    let msg: UnifiedMessage = match serde_json::from_value(params.clone()) {
                        Ok(m) => m,
                        Err(_) => continue,
                    };
                    match msg {
                        UnifiedMessage::TextDelta { ref text } => {
                            text_buffer.push_str(text);
                        }
                        UnifiedMessage::TurnEnd { .. } => {
                            if !text_buffer.trim().is_empty() {
                                if let Ok(db) = ctx.db.lock() {
                                    if let Ok(convs) = conversations::list_conversations(&db) {
                                        if let Some(conv) = convs.first() {
                                            let _ = conversations::add_message(
                                                &db,
                                                &conv.id,
                                                "assistant",
                                                &text_buffer,
                                                None,
                                                None,
                                                None,
                                                None,
                                                None,
                                            );
                                        }
                                    }
                                }
                            }
                            text_buffer.clear();
                        }
                        _ => {}
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    eprintln!("[session-server::saver] Dropped {} messages (lagged)", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    });
}
