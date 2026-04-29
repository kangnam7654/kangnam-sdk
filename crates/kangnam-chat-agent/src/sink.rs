//! Transport-agnostic event sink for agent CLI sessions.
//!
//! The agent manager streams [`UnifiedMessage`] (and optional
//! [`ClaudeEnhancedEvent`]) values out through an
//! [`AgentEventSink`]. Hosts implement this trait however they like —
//! WebSocket broadcast, channel forwarding, in-process callback,
//! anything — and pass an `Arc<dyn AgentEventSink>` into
//! [`crate::manager::CliManager`].
//!
//! Keeping this trait here (rather than coupling the manager to a
//! concrete `tokio::sync::broadcast::Sender<JsonRpcNotification>`)
//! is what lets the same agent code run in a Tauri app, a CLI, a
//! standalone server, or a test harness.

use crate::types::{ClaudeEnhancedEvent, UnifiedMessage};

/// Sink that receives streaming events from a running agent session.
///
/// All methods are infallible from the manager's perspective; the
/// implementation is responsible for any error handling (failed sends
/// to a closed channel, etc.).
pub trait AgentEventSink: Send + Sync + 'static {
    /// Receive a unified semantic event (text deltas, tool calls,
    /// permission requests, agent lifecycle, etc.).
    fn emit_message(&self, msg: UnifiedMessage);

    /// Receive a Claude-specific enhanced event (task progress, hooks,
    /// rate limits, etc.). Default is no-op for hosts that don't care.
    fn emit_enhanced(&self, event: ClaudeEnhancedEvent) {
        let _ = event;
    }
}
