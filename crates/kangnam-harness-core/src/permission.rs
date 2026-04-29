use serde::{Deserialize, Serialize};

use crate::Scope;

/// A permission rule controlling whether a tool call is allowed, requires
/// confirmation, or is blocked outright. Patterns are matched in priority
/// order; first match wins. Mirrors Claude Code's `permissions` block in
/// settings.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    pub id: String,
    /// Glob-like pattern matched against `tool_name` or `tool_name:argument`.
    /// Examples: `Bash`, `Bash(git status:*)`, `mcp__*`.
    pub pattern: String,
    pub action: PermissionAction,
    #[serde(default = "default_scope")]
    pub scope: Scope,
    #[serde(default)]
    pub sort_order: i64,
}

fn default_scope() -> Scope {
    Scope::User
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionAction {
    /// Run without prompting.
    Allow,
    /// Prompt the user before each invocation.
    Ask,
    /// Block the call.
    Deny,
}
