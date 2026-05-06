//! Filesystem catalog loader. Each subdirectory of `skills/` is one skill,
//! identified by directory name; the `SKILL.md` inside is the canonical
//! frontmatter + body. `assets/` and `references/` side files are kept on
//! disk and pointed to via [`DesignSkill::root`].

use std::fs;
use std::path::{Path, PathBuf};

use crate::frontmatter::{parse_frontmatter, FrontmatterError};
use crate::model::DesignSkill;

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("frontmatter: {0}")]
    Frontmatter(#[from] FrontmatterError),
    #[error("missing SKILL.md in {0}")]
    Missing(PathBuf),
}

/// Load every skill at `<root>/<id>/SKILL.md`. Returns sorted by id.
pub fn load_skills_from_dir(root: impl AsRef<Path>) -> Result<Vec<DesignSkill>, LoadError> {
    let mut skills = Vec::new();
    for entry in fs::read_dir(root.as_ref())? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if !path.join("SKILL.md").exists() {
            continue;
        }
        skills.push(load_skill(&path)?);
    }
    skills.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(skills)
}

/// Load a single skill from a directory containing `SKILL.md`. The skill's
/// `id` is set from the directory name; `root` is the absolute path.
pub fn load_skill(dir: impl AsRef<Path>) -> Result<DesignSkill, LoadError> {
    let dir = dir.as_ref();
    let skill_md = dir.join("SKILL.md");
    if !skill_md.exists() {
        return Err(LoadError::Missing(skill_md));
    }
    let body = fs::read_to_string(&skill_md)?;
    let mut skill = parse_frontmatter(&body)?;
    skill.id = dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();
    skill.root = dir.to_path_buf();
    Ok(skill)
}

/// Cheap directory-scan alternative — returns just the ids (no parse).
pub fn list_skill_ids(root: impl AsRef<Path>) -> Result<Vec<String>, std::io::Error> {
    let mut ids = Vec::new();
    for entry in fs::read_dir(root.as_ref())? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && path.join("SKILL.md").exists() {
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                ids.push(name.to_string());
            }
        }
    }
    ids.sort();
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vendored_root() -> PathBuf {
        let mut p = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap();
        p.push("skills");
        p
    }

    #[test]
    fn loads_full_vendored_catalog() {
        let root = vendored_root();
        if !root.exists() {
            return;
        }
        let ids = list_skill_ids(&root).unwrap();
        assert!(
            ids.len() >= 60,
            "expected at least 60 vendored skills, got {}",
            ids.len()
        );
        for id in [
            "web-prototype",
            "dashboard",
            "guizang-ppt",
            "mobile-app",
            "html-ppt",
            "hatch-pet",
            "kami-deck",
        ] {
            assert!(
                ids.iter().any(|i| i == id),
                "expected `{id}` in vendored catalog"
            );
        }
    }

    #[test]
    fn loads_one_skill_with_od_metadata() {
        let mut root = vendored_root();
        root.push("web-prototype");
        if !root.exists() {
            return;
        }
        let skill = load_skill(&root).unwrap();
        assert_eq!(skill.id, "web-prototype");
        assert_eq!(skill.name, "web-prototype");
        assert!(!skill.triggers.is_empty());
        assert_eq!(skill.od.mode.as_deref(), Some("prototype"));
        assert_eq!(skill.od.platform.as_deref(), Some("desktop"));
    }

    #[test]
    fn harness_skill_round_trip_preserves_extras() {
        let mut root = vendored_root();
        root.push("web-prototype");
        if !root.exists() {
            return;
        }
        let ds = load_skill(&root).unwrap();
        let h = ds.to_harness_skill();
        assert_eq!(h.id, "web-prototype");
        // Lossy `triggers → Auto{keywords}`.
        match &h.trigger {
            kangnam_harness_core::SkillTrigger::Auto { keywords } => {
                assert!(!keywords.is_empty(), "expected keywords from triggers");
            }
            other => panic!("expected SkillTrigger::Auto, got {:?}", other),
        }
        // od: keys land in frontmatter_extras.
        let extras = h.frontmatter_extras.as_object().unwrap();
        assert!(extras.contains_key("mode") || extras.contains_key("platform"));
    }

    #[cfg(feature = "craft")]
    #[test]
    fn resolve_crafts_returns_vendored_refs_for_typed_requires() {
        // Build a skill in-memory with explicit od.craft.requires — no need
        // to find a vendored skill that already opts in.
        let yaml = r#"---
name: x
od:
  craft:
    requires: [typography, anti-ai-slop, totally-fictional]
---
body
"#;
        let skill = crate::frontmatter::parse_frontmatter(yaml).unwrap();
        let crafts = skill.resolve_crafts();
        // Two known slugs resolve, the fictional one drops.
        assert_eq!(crafts.len(), 2);
        assert_eq!(crafts[0].id, "typography");
        assert_eq!(crafts[1].id, "anti-ai-slop");
    }

    #[cfg(feature = "craft")]
    #[test]
    fn resolve_crafts_empty_when_no_requires() {
        let yaml = "---\nname: x\n---\nbody\n";
        let skill = crate::frontmatter::parse_frontmatter(yaml).unwrap();
        let crafts = skill.resolve_crafts();
        assert!(crafts.is_empty());
    }
}
