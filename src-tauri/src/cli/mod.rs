//! CLI agent integration.
//!
//! All session management, parsing, and binary resolution live in
//! the `chat-agent` crate. This module re-exports the surface under
//! the original `crate::cli::*` paths so existing call sites
//! (`crate::cli::adapters::claude::ClaudeAdapter`, etc.) keep working.
//!
//! The transport bridge from `chat_agent::AgentEventSink` to the
//! WebSocket broadcast channels lives in `chat_server::broadcast`.

pub use chat_agent::adapters;
