//! Umbrella crate for the kangnam-harness agent runtime.
//!
//! Re-exports sibling crates under stable module names so consumers
//! depend on a single `kangnam-harness = { ... }` line and pick
//! capabilities through feature flags.
//!
//! ```toml
//! # Default — core types + SQLite store
//! kangnam-harness = { workspace = true }
//!
//! # Just the pure domain (no persistence)
//! kangnam-harness = { workspace = true, default-features = false }
//! ```
//!
//! # Boundary
//!
//! `kangnam-harness` owns agent execution primitives: tools, skills,
//! hooks, permissions, scopes, persistence, and runtime capability
//! traits. It does not own conversations, WebSockets, JSON-RPC method
//! names, session history, or UI transport. Hosts such as
//! `kangnam-harness-session` may adapt session events into harness
//! interaction responses, but the core harness crates stay
//! host-agnostic.

#![doc(html_no_source)]

/// Core domain: tools, skills, hooks, agents, permissions, scope.
pub use kangnam_harness_core as core;

/// Persistence layer: `HarnessStore` trait + SQLite default impl.
#[cfg(feature = "store")]
pub use kangnam_harness_store as store;

#[cfg(test)]
mod boundary_tests {
    #[test]
    fn umbrella_does_not_depend_on_chat_crates() {
        let manifest = include_str!("../Cargo.toml");

        assert!(
            !manifest.contains("kangnam-chat"),
            "kangnam-harness must not depend on legacy chat crates; use router/session bridges where needed"
        );
    }
}
