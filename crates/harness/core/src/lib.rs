//! Core SDK types for the kangnam harness.
//!
//! This crate intentionally stays domain-agnostic. It contains the shared
//! metadata models the harness manages and the executable tool contract
//! (`AgentTool`, `ToolCtx`, `ToolResult`, capability callback traits) used by
//! host applications. Domain policy, storage schemas, app workflows, and
//! provider-specific LLM loops live in sibling or consuming crates.

pub mod agent;
pub mod hook;
pub mod permission;
pub mod scope;
pub mod skill;
pub mod tool;

pub use agent::{Agent, ModelSelector};
pub use hook::{Hook, HookEvent, HookMatcher};
pub use permission::{Permission, PermissionAction};
pub use scope::Scope;
pub use skill::{Skill, SkillReference, SkillTrigger};
pub use tool::{
    AgentTool, AwaitKind, BuiltinTool, DefaultCapabilities, FsCallbacks, ImageCallbacks,
    InteractionBridge, Tool, ToolCtx, ToolError, ToolResult, ToolSource, WebCallbacks,
};

use thiserror::Error;

/// Errors raised by harness operations. Storage and runtime crates re-export
/// this and add their own variants where needed.
#[derive(Debug, Error)]
pub enum HarnessError {
    #[error("resource not found: {kind} id={id}")]
    NotFound { kind: &'static str, id: String },

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, HarnessError>;
