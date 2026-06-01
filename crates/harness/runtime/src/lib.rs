#![deny(unsafe_code)]
#![deprecated(
    since = "0.3.1",
    note = "renamed to kangnam-harness-core; use kangnam_harness_core instead"
)]
//! Deprecated compatibility crate for `kangnam-harness-runtime`.
//!
//! The generic SDK contract (`AgentTool`, `ToolCtx`, `ToolResult`, capability
//! callback traits, and metadata types) now lives in `kangnam-harness-core`.
//! This crate remains as a short-term re-export layer for existing path
//! consumers.

pub mod hook;
pub mod permission;
pub mod tool;

pub use hook::{HookExecutor, HookOutcome};
pub use kangnam_harness_core::*;
pub use permission::{PermissionEvaluator, PermissionVerdict};
