use serde::{Deserialize, Serialize};

use crate::Scope;

/// A shell command the harness fires at a specific lifecycle event.
/// Mirrors Claude Code's settings.json hooks model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hook {
    pub id: String,
    pub event: HookEvent,
    /// Optional matcher — for tool-related events this filters by tool name.
    #[serde(default)]
    pub matcher: Option<HookMatcher>,
    /// The shell command to execute.
    pub command: String,
    /// Per-hook timeout in seconds; harness uses a default if `None`.
    #[serde(default)]
    pub timeout_secs: Option<u32>,
    #[serde(default = "default_scope")]
    pub scope: Scope,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_scope() -> Scope {
    Scope::User
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HookEvent {
    /// Before a tool call is executed; can block the call.
    PreToolUse,
    /// After a tool call completes successfully.
    PostToolUse,
    /// User submits a prompt; can rewrite or block it.
    UserPromptSubmit,
    /// Model returns control to the user (turn ends).
    Stop,
    /// New session starts.
    SessionStart,
    /// Session ends.
    SessionEnd,
    /// Sub-agent stops.
    SubagentStop,
    /// Notification fired (e.g. permission prompt).
    Notification,
}

/// Constrains when the hook fires. For tool events, `tool` is a glob/exact
/// match against the tool name. For other events, matchers are ignored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookMatcher {
    #[serde(default)]
    pub tool: Option<String>,
}
