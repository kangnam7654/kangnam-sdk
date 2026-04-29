use serde::{Deserialize, Serialize};

use crate::Scope;

/// A sub-agent definition — the harness can spawn this with its own context
/// window, system prompt, and tool allowlist. Mirrors Claude Code's agent
/// frontmatter model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// The agent's system prompt — instructions, persona, output format.
    pub instructions: String,
    /// Which tools the agent is allowed to call. `None` = inherit parent's set.
    #[serde(default)]
    pub allowed_tools: Option<Vec<String>>,
    /// Optional model override; falls back to the harness default if `None`.
    #[serde(default)]
    pub model: Option<ModelSelector>,
    #[serde(default = "default_max_turns")]
    pub max_turns: u32,
    #[serde(default = "default_scope")]
    pub scope: Scope,
    #[serde(default)]
    pub sort_order: i64,
}

fn default_max_turns() -> u32 {
    10
}

fn default_scope() -> Scope {
    Scope::User
}

/// Identifies which model an agent should run on. Strings are resolved by the
/// runtime crate against whatever provider router is in use (e.g. llm-router).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModelSelector {
    /// Use the harness's configured default model.
    Default,
    /// Use a specific model id, e.g. "claude-opus-4-7" or "gpt-5".
    Named { id: String },
    /// Pick a model by tier — runtime maps these to concrete model ids.
    Tier(ModelTier),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelTier {
    Haiku,
    Sonnet,
    Opus,
}
