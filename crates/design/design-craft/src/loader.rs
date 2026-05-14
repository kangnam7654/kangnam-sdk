//! Filesystem catalog loader for user-supplied craft references. The
//! built-in 7 crafts are constants ([`crate::BUILTIN_CRAFTS`]) — this
//! loader is for projects that vendor their own `crafts/<id>.md` files
//! (custom rules, locale-specific guidance, brand-private craft).
//!
//! Each `<id>.md` becomes one [`OwnedCraft`] with `id` set from the
//! filename (without `.md`). Subdirectories are ignored; files starting
//! with `_` (e.g. `_README.md`) are skipped to avoid the "is this a
//! craft or scaffolding?" ambiguity.
//!
//! Use [`crate::render_for_prompt`] to concatenate the result for
//! injection — `OwnedCraft` and `Craft` both implement [`crate::AsCraftRef`]
//! so the same call site handles either source.
//!
//! ```no_run
//! use kangnam_design_craft::{load_crafts_from_dir, render_for_prompt};
//!
//! let crafts = load_crafts_from_dir("crafts/").unwrap();
//! let block = render_for_prompt(&crafts);
//! # let _ = block;
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use kangnam_design_catalog_core as catalog;

use crate::{AsCraftRef, Craft};

/// Owned craft loaded from disk. Mirrors [`Craft`] but heap-allocated
/// (the `'static` zero-cost path is for built-ins; user-vendored files
/// are necessarily owned).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnedCraft {
    pub id: String,
    pub title: String,
    pub when_to_require: String,
    pub body: String,
}

impl AsCraftRef for OwnedCraft {
    fn as_craft_ref(&self) -> CraftRef<'_> {
        CraftRef {
            id: &self.id,
            title: &self.title,
            when_to_require: &self.when_to_require,
            body: &self.body,
        }
    }
}

/// Lifetime-bounded adapter so that the `render_for_prompt`-style API can
/// read fields from either `&'static Craft` or `&'a OwnedCraft` without
/// forcing callers to clone strings.
#[derive(Debug, Clone, Copy)]
pub struct CraftRef<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub when_to_require: &'a str,
    pub body: &'a str,
}

impl<'a> From<&'a Craft> for CraftRef<'a> {
    fn from(c: &'a Craft) -> Self {
        Self {
            id: c.id,
            title: c.title,
            when_to_require: c.when_to_require,
            body: c.body,
        }
    }
}

impl<'a> AsCraftRef for CraftRef<'a> {
    fn as_craft_ref(&self) -> CraftRef<'_> {
        *self
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a directory: {0}")]
    NotDir(PathBuf),
}

impl From<catalog::CatalogError> for LoadError {
    fn from(e: catalog::CatalogError) -> Self {
        match e {
            catalog::CatalogError::Io(e) => LoadError::Io(e),
            // NotFound and Parse cannot be produced by the craft parse closure
            // (which only returns io::Error), but we handle them defensively.
            catalog::CatalogError::NotFound(id) => {
                LoadError::Io(std::io::Error::other(format!("craft not found: {id}")))
            }
            catalog::CatalogError::Parse { id, source } => LoadError::Io(std::io::Error::other(
                format!("parse error for craft `{id}`: {source}"),
            )),
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────

/// Filter: accept `.md` files whose stem does not start with `_`.
fn md_craft_filter(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    let stem = path.file_stem()?.to_str()?;
    if stem.starts_with('_') {
        return None;
    }
    if path.extension()?.to_str() != Some("md") {
        return None;
    }
    Some(stem.to_string())
}

fn parse_craft(id: &str, path: &Path) -> Result<OwnedCraft, std::io::Error> {
    let body = fs::read_to_string(path)?;
    let title = first_h1(&body).unwrap_or_else(|| pretty_title(id));
    Ok(OwnedCraft {
        id: id.to_string(),
        title,
        when_to_require: String::new(),
        body,
    })
}

fn first_h1(body: &str) -> Option<String> {
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix("# ") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

fn pretty_title(stem: &str) -> String {
    // typography → Typography, anti-ai-slop → Anti-Ai-Slop (good enough as
    // a fallback; canonical entries already provide their own titles).
    stem.split('-')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

// ── public API ────────────────────────────────────────────────────────────

/// Load every `<id>.md` at `<root>/`. Returns crafts sorted by id.
/// Title defaults to the first H1 in the body (or the id, capitalized,
/// if no H1 is found). `when_to_require` is empty for user-supplied
/// crafts — the field exists for symmetry with the built-ins; consumers
/// who want it should encode their own convention (e.g. a `> Required for: …`
/// blockquote on the first line) and post-process.
pub fn load_crafts_from_dir(root: impl AsRef<Path>) -> Result<Vec<OwnedCraft>, LoadError> {
    let root = root.as_ref();
    if !root.is_dir() {
        return Err(LoadError::NotDir(root.to_path_buf()));
    }
    catalog::load_dir(root, md_craft_filter, parse_craft).map_err(LoadError::from)
}

/// Cheap directory-scan alternative — returns just the ids.
pub fn list_craft_ids(root: impl AsRef<Path>) -> Result<Vec<String>, LoadError> {
    let root = root.as_ref();
    if !root.is_dir() {
        return Err(LoadError::NotDir(root.to_path_buf()));
    }
    catalog::list_ids(root, md_craft_filter).map_err(LoadError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn loads_vendored_crafts_dir() {
        // Loading the bundled `crafts/` directory should round-trip every
        // built-in id (typography, color, …) plus the README which the
        // built-in API hides.
        let mut p = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap();
        p.push("crafts");
        if !p.exists() {
            return;
        }
        let crafts = load_crafts_from_dir(&p).unwrap();
        let ids: Vec<_> = crafts.iter().map(|c| c.id.as_str()).collect();
        for expected in [
            "typography",
            "color",
            "anti-ai-slop",
            "state-coverage",
            "animation-discipline",
            "accessibility-baseline",
            "rtl-and-bidi",
        ] {
            assert!(ids.contains(&expected), "missing {expected} in {ids:?}");
        }
        // README is filtered (capitalized — we filter underscored, but
        // README is special-cased: it should still load. In our convention
        // README is the only `_`-free non-craft markdown, so it leaks.
        // That's acceptable: built-in API has README as an explicit const,
        // and CraftCatalog ergonomics aren't hurt by a stray readme entry.
        // The test is here to document the behavior, not enforce filtering.
        assert!(ids.contains(&"README") || ids.contains(&"readme"));
    }

    #[test]
    fn ignores_subdirs_and_non_md() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("subdir")).unwrap();
        fs::write(tmp.path().join("subdir/typography.md"), "# nope").unwrap();
        fs::write(tmp.path().join("notes.txt"), "# also nope").unwrap();
        fs::write(tmp.path().join("real.md"), "# Real Title\n\nbody.\n").unwrap();
        let crafts = load_crafts_from_dir(tmp.path()).unwrap();
        assert_eq!(crafts.len(), 1);
        assert_eq!(crafts[0].id, "real");
        assert_eq!(crafts[0].title, "Real Title");
    }

    #[test]
    fn skips_underscore_prefixed_files() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("_README.md"), "# scaffold").unwrap();
        fs::write(tmp.path().join("real.md"), "# r").unwrap();
        let ids = list_craft_ids(tmp.path()).unwrap();
        assert_eq!(ids, vec!["real".to_string()]);
    }

    #[test]
    fn first_h1_picks_first_only() {
        assert_eq!(
            first_h1("preamble\n# First\n\n# Second\n"),
            Some("First".into())
        );
        assert_eq!(first_h1("no heading"), None);
        assert_eq!(first_h1(""), None);
    }

    #[test]
    fn pretty_title_capitalizes_segments() {
        assert_eq!(pretty_title("typography"), "Typography");
        assert_eq!(pretty_title("anti-ai-slop"), "Anti-Ai-Slop");
        assert_eq!(pretty_title(""), "");
    }

    #[test]
    fn falls_back_to_pretty_title_when_no_h1() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("foo-bar.md"), "no heading here").unwrap();
        let crafts = load_crafts_from_dir(tmp.path()).unwrap();
        assert_eq!(crafts[0].title, "Foo-Bar");
    }

    #[test]
    fn errors_on_non_directory() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let err = load_crafts_from_dir(tmp.path()).unwrap_err();
        assert!(matches!(err, LoadError::NotDir(_)));
    }

    #[test]
    fn owned_to_ref_round_trip() {
        let owned = OwnedCraft {
            id: "x".into(),
            title: "X".into(),
            when_to_require: "test".into(),
            body: "# X\n".into(),
        };
        let r = owned.as_craft_ref();
        assert_eq!(r.id, "x");
        assert_eq!(r.body, "# X\n");
    }

    #[test]
    fn render_mixes_builtin_and_owned() {
        // Mixed iterator — built-in const + owned-from-disk.
        let owned = OwnedCraft {
            id: "house-rule".into(),
            title: "House rule".into(),
            when_to_require: String::new(),
            body: "Don't use Tailwind indigo.".into(),
        };
        let crafts: Vec<CraftRef<'_>> =
            vec![crate::TYPOGRAPHY.as_craft_ref(), owned.as_craft_ref()];
        let out = crate::render_for_prompt(crafts);
        assert!(out.contains("## Typography craft rules"));
        assert!(out.contains("## House rule"));
    }
}
