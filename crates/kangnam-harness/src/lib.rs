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

#![doc(html_no_source)]

/// Core domain: tools, skills, hooks, agents, permissions, scope.
pub use kangnam_harness_core as core;

/// Persistence layer: `HarnessStore` trait + SQLite default impl.
#[cfg(feature = "store")]
pub use kangnam_harness_store as store;
