//! CLI agent integration.
//!
//! The agent session manager and parser modules now live in the
//! `chat-agent` crate (sibling repo `chat-module`). This module
//! re-exports the chat-agent surface under the original
//! `crate::cli::*` paths so existing call sites keep working, and
//! adds the [`broadcast_sink::BroadcastSink`] adapter that bridges
//! chat-agent's transport-agnostic `AgentEventSink` to this app's
//! `tokio::sync::broadcast` channels carrying `JsonRpcNotification`.

pub use chat_agent::{adapters, manager, registry, types};

pub mod broadcast_sink;
