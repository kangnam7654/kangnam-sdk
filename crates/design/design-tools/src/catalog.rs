//! Skill catalog adapter — used by `skill` and `scaffold` to look up
//! a `DesignSkill` body or its `assets/` directory at execute time.
//!
//! The trait is async even though the in-memory implementation
//! resolves synchronously — production hosts will likely back this
//! with the unified `prompts` SQLite table from Phase 0c.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use kangnam_design_skill::DesignSkill;

#[async_trait]
pub trait SkillCatalog: Send + Sync {
    /// Resolve a skill by id (typically the directory name on disk).
    /// Returns `None` if not found.
    async fn lookup(&self, id: &str) -> Option<Arc<DesignSkill>>;

    /// Resolve the absolute path of a skill's `assets/` directory.
    /// Returns `None` if the skill has no on-disk bundle (in-memory
    /// catalogs).
    async fn assets_dir(&self, id: &str) -> Option<PathBuf>;
}

/// Trivial in-memory catalog used by tests.
#[derive(Default)]
pub struct StaticSkillCatalog {
    skills: HashMap<String, Arc<DesignSkill>>,
    assets: HashMap<String, PathBuf>,
}

impl StaticSkillCatalog {
    pub fn new() -> Self { Self::default() }

    pub fn insert(&mut self, id: impl Into<String>, skill: DesignSkill) -> &mut Self {
        self.skills.insert(id.into(), Arc::new(skill));
        self
    }

    pub fn with_assets(&mut self, id: impl Into<String>, path: PathBuf) -> &mut Self {
        self.assets.insert(id.into(), path);
        self
    }
}

#[async_trait]
impl SkillCatalog for StaticSkillCatalog {
    async fn lookup(&self, id: &str) -> Option<Arc<DesignSkill>> {
        self.skills.get(id).cloned()
    }

    async fn assets_dir(&self, id: &str) -> Option<PathBuf> {
        self.assets.get(id).cloned()
    }
}
