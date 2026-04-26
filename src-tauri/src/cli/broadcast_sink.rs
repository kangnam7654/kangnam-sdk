//! Bridge between `chat_agent::AgentEventSink` and this app's
//! `JsonRpcNotification` broadcast channels.
//!
//! `chat-agent` does not know about JSON-RPC or WebSocket — it just
//! emits typed `UnifiedMessage` / `ClaudeEnhancedEvent` values to a
//! sink. This adapter wraps each event into a `JsonRpcNotification`
//! (`cli.stream` for unified, `cli.enhanced` for enhanced) and
//! publishes it on the corresponding `tokio::sync::broadcast::Sender`.

use chat_agent::sink::AgentEventSink;
use chat_agent::types::{ClaudeEnhancedEvent, UnifiedMessage};

use crate::rpc::types::JsonRpcNotification;
use crate::server::broadcast::{BroadcastTx, EnhancedBroadcastTx};

/// Sink that publishes agent events as JSON-RPC notifications on two
/// broadcast channels.
pub struct BroadcastSink {
    pub stream: BroadcastTx,
    pub enhanced: Option<EnhancedBroadcastTx>,
}

impl BroadcastSink {
    pub fn new(stream: BroadcastTx, enhanced: Option<EnhancedBroadcastTx>) -> Self {
        Self { stream, enhanced }
    }
}

impl AgentEventSink for BroadcastSink {
    fn emit_message(&self, msg: UnifiedMessage) {
        let notification = JsonRpcNotification::new(
            "cli.stream",
            serde_json::to_value(&msg).unwrap_or_default(),
        );
        let _ = self.stream.send(notification);
    }

    fn emit_enhanced(&self, event: ClaudeEnhancedEvent) {
        if let Some(ref tx) = self.enhanced {
            let notification = JsonRpcNotification::new(
                "cli.enhanced",
                serde_json::to_value(&event).unwrap_or_default(),
            );
            let _ = tx.send(notification);
        }
    }
}
