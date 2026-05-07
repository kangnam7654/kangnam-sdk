//! Shared catalog-loader primitives used by design-skill, design-system,
//! design-craft, and design-prompt-template.
//!
//! The two public functions cover all four call sites:
//!
//! - [`list_ids`] — cheap directory scan that returns only the ids (no parse).
//! - [`load_dir`] — full walk that parses each entry and returns sorted items.
//!
//! Both accept a `filter` closure `Fn(&Path) -> Option<String>` that decides
//! whether an entry participates and what its catalog id is:
//!
//! * Return `Some(id)` to include the entry.
//! * Return `None` to skip it.
//!
//! This one closure handles all structural variants:
//! - Subdirectory + sentinel file → check `path.is_dir() && path.join("X").exists()`
//! - Flat `.md` files (not README) → check `path.is_file()` + strip extension
//! - Flat `.json` files → same pattern with a different suffix

use std::error::Error;
use std::fs;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("parse error for `{id}`: {source}")]
    Parse {
        id: String,
        #[source]
        source: Box<dyn Error + Send + Sync>,
    },
}

/// List the ids of all entries under `root` that pass `filter`, sorted.
///
/// `filter(path) -> Option<id>` — return `Some(id)` to include, `None` to skip.
///
/// # Errors
/// Returns [`CatalogError::Io`] on any `read_dir` / entry error.
pub fn list_ids(
    root: impl AsRef<Path>,
    filter: impl Fn(&Path) -> Option<String>,
) -> Result<Vec<String>, CatalogError> {
    let mut ids = Vec::new();
    for entry in fs::read_dir(root.as_ref())? {
        let entry = entry?;
        let path = entry.path();
        if let Some(id) = filter(&path) {
            ids.push(id);
        }
    }
    ids.sort();
    Ok(ids)
}

/// Walk `root`, call `filter` on each entry, parse passing entries with
/// `parse`, and return results sorted by id.
///
/// `filter(path) -> Option<id>` — return `Some(id)` to include, `None` to skip.
/// `parse(id, path) -> Result<T, E>` — parse a single entry.
///
/// # Errors
/// - [`CatalogError::Io`] on directory / read errors.
/// - [`CatalogError::Parse`] when `parse` returns an error, wrapping it with
///   the entry id for context.
pub fn load_dir<T, F, E>(
    root: impl AsRef<Path>,
    filter: impl Fn(&Path) -> Option<String>,
    mut parse: F,
) -> Result<Vec<T>, CatalogError>
where
    F: FnMut(&str, &Path) -> Result<T, E>,
    E: Into<Box<dyn Error + Send + Sync>>,
{
    let mut items: Vec<(String, T)> = Vec::new();
    for entry in fs::read_dir(root.as_ref())? {
        let entry = entry?;
        let path = entry.path();
        let Some(id) = filter(&path) else {
            continue;
        };
        let item = parse(&id, &path).map_err(|e| CatalogError::Parse {
            id: id.clone(),
            source: e.into(),
        })?;
        items.push((id, item));
    }
    items.sort_by(|(a, _), (b, _)| a.cmp(b));
    Ok(items.into_iter().map(|(_, v)| v).collect())
}

// ── tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Helper: filter accepting only `.md` files whose stem is not "README".
    fn md_filter(path: &Path) -> Option<String> {
        if !path.is_file() {
            return None;
        }
        let name = path.file_name()?.to_str()?;
        if name.eq_ignore_ascii_case("README.md") {
            return None;
        }
        let stem = name.strip_suffix(".md")?;
        if stem.is_empty() {
            return None;
        }
        Some(stem.to_string())
    }

    #[test]
    fn list_ids_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let ids = list_ids(tmp.path(), md_filter).unwrap();
        assert!(ids.is_empty());
    }

    #[test]
    fn list_ids_sorted_skips_readme() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("beta.md"), "b").unwrap();
        fs::write(tmp.path().join("alpha.md"), "a").unwrap();
        fs::write(tmp.path().join("README.md"), "r").unwrap();
        fs::write(tmp.path().join("notes.txt"), "x").unwrap();
        let ids = list_ids(tmp.path(), md_filter).unwrap();
        assert_eq!(ids, vec!["alpha", "beta"]);
    }

    #[test]
    fn load_dir_sorted_and_parsed() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("zeta.md"), "zcontent").unwrap();
        fs::write(tmp.path().join("alpha.md"), "acontent").unwrap();
        let items = load_dir(
            tmp.path(),
            md_filter,
            |id: &str, path: &Path| -> Result<String, std::io::Error> {
                let body = fs::read_to_string(path)?;
                Ok(format!("{id}:{body}"))
            },
        )
        .unwrap();
        assert_eq!(items, vec!["alpha:acontent", "zeta:zcontent"]);
    }

    #[test]
    fn load_dir_parse_error_wraps_with_id() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("broken.md"), "bad").unwrap();
        let result = load_dir(
            tmp.path(),
            md_filter,
            |_id: &str, _path: &Path| -> Result<(), Box<dyn Error + Send + Sync>> {
                Err("simulated parse failure".into())
            },
        );
        match result.unwrap_err() {
            CatalogError::Parse { id, .. } => assert_eq!(id, "broken"),
            e => panic!("expected Parse, got {e:?}"),
        }
    }

    #[test]
    fn load_dir_subdir_sentinel_filter() {
        let tmp = tempfile::tempdir().unwrap();
        // Subdir with sentinel -> include
        fs::create_dir(tmp.path().join("good")).unwrap();
        fs::write(tmp.path().join("good/DESIGN.md"), "# G").unwrap();
        // Subdir without sentinel -> skip
        fs::create_dir(tmp.path().join("bare")).unwrap();
        // File at root -> skip
        fs::write(tmp.path().join("readme.md"), "x").unwrap();

        let dir_filter = |path: &Path| -> Option<String> {
            if path.is_dir() && path.join("DESIGN.md").exists() {
                path.file_name()?.to_str().map(|s| s.to_string())
            } else {
                None
            }
        };

        let ids = list_ids(tmp.path(), dir_filter).unwrap();
        assert_eq!(ids, vec!["good"]);
    }
}
