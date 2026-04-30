use serde::{Deserialize, Serialize};

use crate::Scope;

/// A skill — a markdown package the harness loads on demand to give the
/// model task-specific knowledge. Mirrors Claude Code's skill model: a name,
/// a trigger (when to load), the skill body, and optional reference files.
///
/// `frontmatter_extras` carries any frontmatter keys that don't map to the
/// canonical fields above — most notably the `od:` namespace from
/// open-design's [SKILL.md][od] extension (`mode`, `platform`, `scenario`,
/// `preview`, `design_system`, ...) and Anthropic SKILL.md V1 fields not yet
/// promoted to columns (`allowed_tools`, `disable_model_invocation`,
/// `argument_hint`, `model`, `user_invocable`). Default `Null` keeps the
/// field non-breaking for existing JSON blobs and SQLite rows: rows that
/// pre-date this field deserialize to `Value::Null`.
///
/// [od]: https://github.com/nexu-io/open-design/blob/main/docs/skills-protocol.md
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
    /// Free-form frontmatter slot for fields not yet promoted to columns.
    /// See struct-level docs.
    #[serde(default)]
    pub frontmatter_extras: serde_json::Value,
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
