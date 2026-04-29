//! CLI agent integration.
//!
//! All session management, parsing, and binary resolution live in
//! the `chat-agent` crate. This module re-exports the surface under
//! the original `crate::cli::*` paths so existing call sites
//! (`crate::cli::adapters::claude::ClaudeAdapter`, etc.) keep working.
//!
//! The transport bridge from `kangnam_chat::agent::AgentEventSink` to the
//! WebSocket broadcast channels lives in `kangnam_chat::server::broadcast`.

pub use kangnam_chat::agent::adapters;
