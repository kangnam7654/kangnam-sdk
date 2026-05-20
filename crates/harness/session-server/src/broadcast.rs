//! Broadcast channels and the [`BroadcastSink`] that bridges
//! `kangnam_harness_session_agent::AgentEventSink` to JSON-RPC notifications fanned out
//! over `tokio::sync::broadcast`.

use kangnam_harness_session_agent::sink::AgentEventSink;
use kangnam_harness_session_agent::types::{ClaudeEnhancedEvent, UnifiedMessage};
use kangnam_harness_session_core::json_rpc::JsonRpcNotification;
use tokio::sync::broadcast;

/// Sender for `cli.stream` notifications (unified semantic events).
pub type BroadcastTx = broadcast::Sender<JsonRpcNotification>;
/// Receiver for `cli.stream` notifications.
pub type BroadcastRx = broadcast::Receiver<JsonRpcNotification>;

/// Sender for `cli.enhanced` notifications (Claude-specific events).
pub type EnhancedBroadcastTx = broadcast::Sender<JsonRpcNotification>;
/// Receiver for `cli.enhanced` notifications.
pub type EnhancedBroadcastRx = broadcast::Receiver<JsonRpcNotification>;

/// Default channel capacity for both broadcast channels.
pub const DEFAULT_CHANNEL_CAPACITY: usize = 256;

/// Convenience constructor for the unified-event channel.
pub fn create_channel() -> (BroadcastTx, BroadcastRx) {
    broadcast::channel(DEFAULT_CHANNEL_CAPACITY)
}

/// Convenience constructor for the Claude-enhanced-event channel.
pub fn create_enhanced_channel() -> (EnhancedBroadcastTx, EnhancedBroadcastRx) {
    broadcast::channel(DEFAULT_CHANNEL_CAPACITY)
}

/// `AgentEventSink` impl that wraps each event in a
/// `JsonRpcNotification` and publishes it on the broadcast channels.
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
        let n =
            JsonRpcNotification::new("cli.stream", serde_json::to_value(&msg).unwrap_or_default());
        let _ = self.stream.send(n);
    }

    fn emit_enhanced(&self, event: ClaudeEnhancedEvent) {
        if let Some(ref tx) = self.enhanced {
            let n = JsonRpcNotification::new(
                "cli.enhanced",
                serde_json::to_value(&event).unwrap_or_default(),
            );
            let _ = tx.send(n);
        }
    }
}
