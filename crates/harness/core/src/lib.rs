//! Core types for the kangnam-harness runtime.
//!
//! This crate is intentionally I/O-free: pure data definitions for the four
//! resource kinds the harness manages — tools, skills, hooks, agents — plus
//! cross-cutting concerns (permissions, scope). Storage and execution live in
//! sibling crates (`harness-store`, `harness-runtime`).

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
pub use tool::{BuiltinTool, Tool, ToolSource};

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
