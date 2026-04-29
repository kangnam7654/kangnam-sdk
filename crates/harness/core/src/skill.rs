use serde::{Deserialize, Serialize};

use crate::Scope;

/// A skill — a markdown package the harness loads on demand to give the
/// model task-specific knowledge. Mirrors Claude Code's skill model: a name,
/// a trigger (when to load), the skill body, and optional reference files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub trigger: SkillTrigger,
    pub content: String,
    #[serde(default)]
    pub references: Vec<SkillReference>,
    #[serde(default = "default_scope")]
    pub scope: Scope,
    #[serde(default)]
    pub sort_order: i64,
}

fn default_scope() -> Scope {
    Scope::User
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum SkillTrigger {
    /// Load when the user prompt matches one of the trigger keywords.
    Auto { keywords: Vec<String> },
    /// Only load when the user explicitly invokes the skill.
    Manual,
    /// Always loaded for every turn (use sparingly).
    Always,
}

/// A supplementary file or snippet attached to a skill (Claude Code's
/// "skill references" — additional context loaded alongside the skill body).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillReference {
    pub id: String,
    pub name: String,
    pub content: String,
    #[serde(default)]
    pub sort_order: i64,
}
