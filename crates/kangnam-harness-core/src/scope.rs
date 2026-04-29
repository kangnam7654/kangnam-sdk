use serde::{Deserialize, Serialize};

/// Where a resource is defined. Mirrors Claude Code's settings.json merge order:
/// `User` (global) → `Project` (repo-shared) → `Local` (repo-local, gitignored).
/// Later scopes override earlier ones when the harness merges configurations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    User,
    Project,
    Local,
}

impl Scope {
    /// Sort order used when merging — lower number = lower priority (overridden first).
    pub fn priority(self) -> u8 {
        match self {
            Scope::User => 0,
            Scope::Project => 1,
            Scope::Local => 2,
        }
    }
}
