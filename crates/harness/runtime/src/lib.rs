//! Tool execution runtime for the kangnam-sdk harness.
//!
//! `harness-core` is intentionally I/O-free — it stores tool *descriptors*,
//! permission *rules*, and hook *definitions*, but doesn't run anything.
//! This crate fills in the missing piece: an async trait every executable
//! tool implements (`DesignTool`), a host-supplied execution context with
//! fs / web / image-gen / preview side-channels (`ToolCtx`), a result
//! shape that supports turn suspension (`ToolResult::AwaitUser`), and
//! permission + hook evaluators.
//!
//! This is the layer ADR-002 hinted at as a future extraction. Phase 4 of
//! the design family work activates it because `ask` and `preview` need
//! to suspend the agent turn until the host posts back a response over
//! the chat-rpc channel — something `harness-core::Tool` doesn't model.

pub mod tool;
pub mod permission;
pub mod hook;

pub use tool::{DesignTool, ToolCtx, ToolError, ToolResult, AwaitKind};
pub use permission::{PermissionEvaluator, PermissionVerdict};
pub use hook::{HookExecutor, HookOutcome};
