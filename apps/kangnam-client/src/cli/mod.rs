//! CLI agent integration.
//!
//! All session management, parsing, and binary resolution live in
//! the `session-agent` crate. This module re-exports the surface under
//! the original `crate::cli::*` paths so existing call sites
//! (`crate::cli::adapters::claude::ClaudeAdapter`, etc.) keep working.
//!
//! The transport bridge from `kangnam_harness_session::agent::AgentEventSink` to the
//! WebSocket broadcast channels lives in `kangnam_harness_session::server::broadcast`.

pub use kangnam_harness_session::agent::adapters;
