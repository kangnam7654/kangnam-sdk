//! Filesystem catalog loader. Each subdirectory of a "systems root" is one
//! design system, identified by its directory name; the `DESIGN.md` inside
//! becomes the body. Skips the optional `README.md` at the root level.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use kangnam_design_catalog_core as catalog;

use crate::parser::{NineSections, ParseError, parse_design_md};
use crate::tokens::{ColorToken, extract_color_tokens};

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

/// Internal error type that can hold both io and parse errors for use with
/// `catalog::load_dir`'s `E: Into<Box<dyn Error + Send + Sync>>` bound.
#[derive(Debug, thiserror::Error)]
enum ParseSystemError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse: {0}")]
    Parse(#[from] ParseError),
}

impl From<catalog::CatalogError> for LoadError {
    fn from(e: catalog::CatalogError) -> Self {
        match e {
            catalog::CatalogError::Io(e) => LoadError::Io(e),
            catalog::CatalogError::NotFound(id) => LoadError::Missing(PathBuf::from(id)),
            catalog::CatalogError::Parse { id: _, source } => {
                // Downcast to ParseSystemError variants.
                if let Ok(inner) = source.downcast::<ParseSystemError>() {
                    match *inner {
                        ParseSystemError::Io(e) => LoadError::Io(e),
                        ParseSystemError::Parse(e) => LoadError::Parse(e),
                    }
                } else {
                    LoadError::Io(std::io::Error::other("unknown parse error"))
                }
            }
        }
    }
}

// ── filter ────────────────────────────────────────────────────────────────

/// Accepts subdirectories that contain a `DESIGN.md`.
fn system_filter(path: &Path) -> Option<String> {
    if !path.is_dir() {
        return None;
    }
    if !path.join("DESIGN.md").exists() {
        return None;
    }
    path.file_name()?.to_str().map(|s| s.to_string())
}

// ── parse ─────────────────────────────────────────────────────────────────

fn parse_system(id: &str, path: &Path) -> Result<DesignSystem, ParseSystemError> {
    let body = fs::read_to_string(path.join("DESIGN.md"))?;
    let sections = parse_design_md(&body)?;
    let color_tokens = sections
        .color
        .as_deref()
        .map(extract_color_tokens)
        .unwrap_or_default();
    Ok(DesignSystem {
        id: id.to_string(),
        body,
        sections,
        color_tokens,
    })
}

// ── public API ────────────────────────────────────────────────────────────

/// Load every system at `<root>/<id>/DESIGN.md`. Returns systems sorted by
/// id. Errors short-circuit (use [`load_systems_from_dir_lossy`] for a
/// best-effort variant when bundled vendored data is partially malformed).
pub fn load_systems_from_dir(root: impl AsRef<Path>) -> Result<Vec<DesignSystem>, LoadError> {
    catalog::load_dir(root.as_ref(), system_filter, parse_system).map_err(LoadError::from)
}

/// Cheap directory-scan alternative — returns just the ids.
pub fn list_system_ids(root: impl AsRef<Path>) -> Result<Vec<String>, LoadError> {
    catalog::list_ids(root.as_ref(), system_filter).map_err(LoadError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
        assert!(
            ids.len() >= 130,
            "expected at least 130 vendored systems, got {}",
            ids.len()
        );
        // A few canonical entries we know are vendored.
        for id in [
            "cursor",
            "linear-app",
            "stripe",
            "vercel",
            "agentic",
            "shadcn",
            "discord",
        ] {
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

    #[test]
    fn empty_directory_yields_empty_list() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load_systems_from_dir(tmp.path()).unwrap().is_empty());
        assert!(list_system_ids(tmp.path()).unwrap().is_empty());
    }

    #[test]
    fn skips_subdirs_without_design_md() {
        let tmp = tempfile::tempdir().unwrap();
        // A subdir with only a README — should be skipped, not error.
        fs::create_dir(tmp.path().join("not-a-system")).unwrap();
        fs::write(tmp.path().join("not-a-system/README.md"), "x").unwrap();
        assert!(load_systems_from_dir(tmp.path()).unwrap().is_empty());
        assert!(list_system_ids(tmp.path()).unwrap().is_empty());
    }

    #[test]
    fn loads_multiple_systems_sorted_with_tokens() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("zeta")).unwrap();
        fs::write(
            tmp.path().join("zeta/DESIGN.md"),
            "# Z\n## Color\n`#ffffff`\n",
        )
        .unwrap();
        fs::create_dir(tmp.path().join("alpha")).unwrap();
        fs::write(
            tmp.path().join("alpha/DESIGN.md"),
            "# A\n## Color\n`#000000`\n",
        )
        .unwrap();
        let systems = load_systems_from_dir(tmp.path()).unwrap();
        assert_eq!(systems.len(), 2);
        assert_eq!(systems[0].id, "alpha");
        assert_eq!(systems[1].id, "zeta");
        // Color tokens flow through from the parsed `color` section.
        assert!(systems[0].color_tokens.iter().any(|t| t.value == "#000000"));
        assert!(systems[1].color_tokens.iter().any(|t| t.value == "#ffffff"));
    }

    #[test]
    fn malformed_design_md_propagates_parse_error() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("blank")).unwrap();
        fs::write(tmp.path().join("blank/DESIGN.md"), "   \n  \n").unwrap();
        let err = load_systems_from_dir(tmp.path()).unwrap_err();
        assert!(matches!(err, LoadError::Parse(ParseError::Empty)));
    }
}
