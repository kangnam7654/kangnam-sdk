//! Domain-agnostic tool execution runtime for an AI agent harness.
//!
//! `kangnam-harness-core` is intentionally I/O-free — it stores tool
//! *descriptors*, permission *rules*, and hook *definitions* but
//! doesn't run anything. This crate fills in the missing piece:
//!
//! - [`AgentTool`] — async trait every executable tool implements.
//! - [`ToolCtx`] — host-supplied execution context carrying session
//!   metadata + side-channel capability callbacks (filesystem, web,
//!   image generation, user-interaction bridge).
//! - [`ToolResult`] — three-way outcome including
//!   [`ToolResult::AwaitUser`] for tools that suspend the agent turn
//!   until the host posts a response through its interaction channel
//!   (forms, previews, selections, approvals, …).
//! - [`PermissionEvaluator`] / [`HookExecutor`] — per-tool gating
//!   surfaces that mirror Claude Code's settings.json semantics.
//!
//! This is the layer ADR-002 hinted at as a future extraction.
//! Phase 4 of the design family work activated it because the
//! design `ask` and `preview` tools need to suspend the agent turn
//! until the host posts back a response through an interaction bridge —
//! something `harness-core::Tool` doesn't model.
//!
//! # Designed for multiple consumers
//!
//! The runtime is *domain-agnostic*: applications provide their own
//! capability bundles (filesystem access, web search, image
//! generation, map lookup, accommodation provider, database access,
//! user interaction, …) and tool implementations on top of the
//! same trait surface.
//!
//! Current consumers:
//! - `design-tools` (Phase 4) — design family of 8 tools (ask,
//!   scaffold, skill, preview, tweaks, done, brand_asset_extract,
//!   gen_image).
//! - Travel Planner — accommodation / spot research / map / money
//!   advice tools (handoff in progress; capability-generic
//!   `ToolCtx<C>` lands in Phase 2 of the harness generalization).

pub mod hook;
pub mod permission;
pub mod tool;

pub use hook::{HookExecutor, HookOutcome};
pub use permission::{PermissionEvaluator, PermissionVerdict};
pub use tool::{
    AgentTool, AwaitKind, DefaultCapabilities, FsCallbacks, ImageCallbacks, InteractionBridge,
    ToolCtx, ToolError, ToolResult, WebCallbacks,
};

#[cfg(test)]
mod boundary_tests {
    #[test]
    fn runtime_does_not_depend_on_chat_crates() {
        let manifest = include_str!("../Cargo.toml");

        assert!(
            !manifest.contains("kangnam-chat"),
            "harness-runtime must stay host-agnostic; put session transport in a bridge or host crate"
        );
    }
}
