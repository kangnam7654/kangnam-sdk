//! Filesystem catalog loader. Each subdirectory of a "systems root" is one
//! design system, identified by its directory name; the `DESIGN.md` inside
//! becomes the body. Skips the optional `README.md` at the root level.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::parser::{parse_design_md, NineSections, ParseError};
use crate::tokens::{extract_color_tokens, ColorToken};

/// One loaded design system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignSystem {
    /// Directory name — used as the catalog id (e.g. "cursor", "linear-app").
    pub id: String,
    /// Full DESIGN.md body.
    pub body: String,
    /// Parsed 9-section structure.
    pub sections: NineSections,
    /// Color tokens extracted from the `color` section (empty when no color
    /// section parsed).
    pub color_tokens: Vec<ColorToken>,
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse: {0}")]
    Parse(#[from] ParseError),
    #[error("missing DESIGN.md in {0}")]
    Missing(PathBuf),
}

/// Load every system at `<root>/<id>/DESIGN.md`. Returns systems sorted by
/// id. Errors short-circuit (use [`load_systems_from_dir_lossy`] for a
/// best-effort variant when bundled vendored data is partially malformed).
pub fn load_systems_from_dir(root: impl AsRef<Path>) -> Result<Vec<DesignSystem>, LoadError> {
    let mut systems = Vec::new();
    for entry in fs::read_dir(root.as_ref())? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let design_md = path.join("DESIGN.md");
        if !design_md.exists() {
            // Tolerate non-system dirs (e.g. starter "default" might exist).
            continue;
        }
        let body = fs::read_to_string(&design_md)?;
        let sections = parse_design_md(&body)?;
        let color_tokens = sections
            .color
            .as_deref()
            .map(extract_color_tokens)
            .unwrap_or_default();
        let id = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        systems.push(DesignSystem {
            id,
            body,
            sections,
            color_tokens,
        });
    }
    systems.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(systems)
}

/// Cheap directory-scan alternative — returns just the ids.
pub fn list_system_ids(root: impl AsRef<Path>) -> Result<Vec<String>, LoadError> {
    let mut out = Vec::new();
    for entry in fs::read_dir(root.as_ref())? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && path.join("DESIGN.md").exists() {
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                out.push(name.to_string());
            }
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vendored_root() -> PathBuf {
        // From this test we are at $WORKSPACE/crates/design/design-system/.
        let mut p = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap();
        p.push("systems");
        p
    }

    #[test]
    fn loads_full_vendored_catalog() {
        let root = vendored_root();
        if !root.exists() {
            // Some downstream consumers vendor a subset; tolerate that.
            return;
        }
        let ids = list_system_ids(&root).unwrap();
        assert!(ids.len() >= 60, "expected at least 60 vendored systems, got {}", ids.len());
        // A few canonical entries we know are vendored.
        for id in ["cursor", "linear-app", "stripe", "vercel"] {
            assert!(
                ids.iter().any(|i| i == id),
                "expected `{id}` in vendored catalog"
            );
        }
    }

    #[test]
    fn loads_one_system_with_tokens() {
        let mut root = vendored_root();
        if !root.join("cursor").exists() {
            return;
        }
        // Load just `cursor` for speed.
        root.push("cursor");
        let body = fs::read_to_string(root.join("DESIGN.md")).unwrap();
        let sections = parse_design_md(&body).unwrap();
        let tokens = sections
            .color
            .as_deref()
            .map(extract_color_tokens)
            .unwrap_or_default();
        assert!(
            tokens.iter().any(|t| t.value.starts_with('#')),
            "expected hex tokens in cursor color section"
        );
    }
}
