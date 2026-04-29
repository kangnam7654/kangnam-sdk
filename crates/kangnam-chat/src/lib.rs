//! Umbrella crate for the chat module.
//!
//! Re-exports the four sibling crates under stable module names so
//! consumers depend on a single `chat = { ... }` line and pick
//! capabilities through feature flags.
//!
//! ```toml
//! # Full stack — Tauri/desktop apps, headless servers
//! chat = { path = "...", features = ["server"] }
//!
//! # Just the agent + RPC, no HTTP server (e.g. embedded transports)
//! chat = { path = "...", features = ["rpc"], default-features = false }
//!
//! # Just the agent, hosting your own RPC layer
//! chat = { path = "...", features = ["agent"], default-features = false }
//!
//! # Just DB models + JSON-RPC types (CLI tools, tests, migrations)
//! chat = { path = "...", default-features = false }
//! ```
//!
//! # Module layout
//! - [`core`] — domain types and persistence (always available)
//! - [`agent`] — CLI session manager (feature `agent`)
//! - [`rpc`] — JSON-RPC handlers (feature `rpc`, implies `agent`)
//! - [`server`] — Axum WebSocket + MCP server (feature `server`,
//!   implies `rpc`)

#![doc(html_no_source)]

/// Pure domain layer: conversations, messages, JSON-RPC types,
/// export formatters, schema migrations.
pub use kangnam_chat_core as core;

/// Persistent CLI agent session manager.
#[cfg(feature = "agent")]
pub use kangnam_chat_agent as agent;

/// JSON-RPC dispatcher and `cli.*` method handlers.
#[cfg(feature = "rpc")]
pub use kangnam_chat_rpc as rpc;

/// Axum WebSocket + MCP server.
#[cfg(feature = "server")]
pub use kangnam_chat_server as server;
