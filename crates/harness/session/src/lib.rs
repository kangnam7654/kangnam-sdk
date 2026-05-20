//! Umbrella crate for the session module.
//!
//! Re-exports the four sibling crates under stable module names so
//! consumers depend on a single `kangnam-harness-session = { ... }`
//! line and pick
//! capabilities through feature flags.
//!
//! ```toml
//! # Full stack — Tauri/desktop apps, headless servers
//! kangnam-harness-session = { path = "...", features = ["server"] }
//!
//! # Just the agent + RPC, no HTTP server (e.g. embedded transports)
//! kangnam-harness-session = { path = "...", features = ["rpc"], default-features = false }
//!
//! # Just the agent, hosting your own RPC layer
//! kangnam-harness-session = { path = "...", features = ["agent"], default-features = false }
//!
//! # Just DB models + JSON-RPC types (CLI tools, tests, migrations)
//! kangnam-harness-session = { path = "...", default-features = false }
//! ```
//!
//! # Module layout
//! - [`core`] — domain types and persistence (always available)
//! - [`agent`] — CLI session manager (feature `agent`)
//! - [`rpc`] — JSON-RPC handlers (feature `rpc`, implies `agent`)
//! - [`server`] — Axum WebSocket + MCP server (feature `server`,
//!   implies `rpc`)
//!
//! # Boundary
//!
//! `kangnam-harness-session` owns the optional user-facing session
//! transport: message storage, session managers, JSON-RPC handlers,
//! WebSocket/MCP transport, and broadcast events. Generic tool,
//! skill, hook, and permission execution remain in the core harness
//! crates.

#![doc(html_no_source)]

/// Pure domain layer: conversations, messages, JSON-RPC types,
/// export formatters, schema migrations.
pub use kangnam_harness_session_core as core;

/// Persistent CLI agent session manager.
#[cfg(feature = "agent")]
pub use kangnam_harness_session_agent as agent;

/// JSON-RPC dispatcher and `cli.*` method handlers.
#[cfg(feature = "rpc")]
pub use kangnam_harness_session_rpc as rpc;

/// Axum WebSocket + MCP server.
#[cfg(feature = "server")]
pub use kangnam_harness_session_server as server;

#[cfg(test)]
mod boundary_tests {
    #[test]
    fn legacy_chat_crate_is_not_reintroduced() {
        let manifest = include_str!("../Cargo.toml");
        let workspace_manifest = include_str!("../../../../Cargo.toml");

        assert!(
            !manifest.contains("kangnam-chat"),
            "kangnam-harness-session must not depend on legacy kangnam-chat crates"
        );
        assert!(
            !workspace_manifest.contains("crates/chat"),
            "workspace must stay harness/router first; do not re-add crates/chat"
        );
    }
}
