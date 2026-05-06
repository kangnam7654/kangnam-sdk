//! Filesystem catalog loader. Walks `<root>/<surface>/<id>.json` and
//! deserializes each file into a [`PromptTemplate`]. The root layout
//! mirrors open-design's `prompt-templates/` directory:
//!
//! ```text
//! templates/
//!   image/
//!     profile-avatar-anime-girl-to-cinematic-photo.json
//!     …
//!   video/
//!     hyperframes-app-showcase-three-phones.json
//!     …
//! ```
//!
//! [`load_templates_from_dir`] loads every surface in one call.
//! [`load_templates_from_surface_dir`] loads a single surface (faster when
//! you only need one).

use std::fs;
use std::path::{Path, PathBuf};

use crate::{PromptTemplate, Surface};

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json parse {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("not a directory: {0}")]
    NotDir(PathBuf),
}

/// Load every template under `<root>/<surface>/<id>.json` for every known
/// [`Surface`]. Returns templates sorted by `(surface, id)` so the order
/// is deterministic.
pub fn load_templates_from_dir(root: impl AsRef<Path>) -> Result<Vec<PromptTemplate>, LoadError> {
    let root = root.as_ref();
    if !root.is_dir() {
        return Err(LoadError::NotDir(root.to_path_buf()));
    }
    let mut all = Vec::new();
    for surface in [Surface::Image, Surface::Video] {
        let dir = root.join(surface.dir_name());
        if !dir.exists() {
            continue;
        }
        let mut chunk = load_templates_from_surface_dir(&dir)?;
        all.append(&mut chunk);
    }
    all.sort_by(|a, b| (a.surface, &a.id).cmp(&(b.surface, &b.id)));
    Ok(all)
}

/// Load every `<id>.json` from a single surface directory (e.g.
/// `templates/image/`). Returns templates sorted by id.
pub fn load_templates_from_surface_dir(
    dir: impl AsRef<Path>,
) -> Result<Vec<PromptTemplate>, LoadError> {
    let dir = dir.as_ref();
    if !dir.is_dir() {
        return Err(LoadError::NotDir(dir.to_path_buf()));
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let body = fs::read_to_string(&path)?;
        let t: PromptTemplate = serde_json::from_str(&body).map_err(|source| LoadError::Json {
            path: path.clone(),
            source,
        })?;
        out.push(t);
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// Cheap directory-scan alternative — returns just the ids per surface.
pub fn list_template_ids(root: impl AsRef<Path>) -> Result<Vec<(Surface, String)>, LoadError> {
    let root = root.as_ref();
    if !root.is_dir() {
        return Err(LoadError::NotDir(root.to_path_buf()));
    }
    let mut out = Vec::new();
    for surface in [Surface::Image, Surface::Video] {
        let dir = root.join(surface.dir_name());
        if !dir.exists() {
            continue;
        }
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    out.push((surface, stem.to_string()));
                }
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
        let mut p = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap();
        p.push("templates");
        p
    }

    #[test]
    fn loads_full_vendored_catalog() {
        let root = vendored_root();
        if !root.exists() {
            return;
        }
        let templates = load_templates_from_dir(&root).unwrap();
        // 44 image + 50 video = 94 (catalog floor; future drops may add more).
        assert!(
            templates.len() >= 90,
            "expected ≥90 vendored templates, got {}",
            templates.len()
        );
        let n_image = templates
            .iter()
            .filter(|t| t.surface == Surface::Image)
            .count();
        let n_video = templates
            .iter()
            .filter(|t| t.surface == Surface::Video)
            .count();
        assert!(n_image >= 40, "expected ≥40 image templates, got {n_image}");
        assert!(n_video >= 45, "expected ≥45 video templates, got {n_video}");
    }

    #[test]
    fn lists_ids_per_surface() {
        let root = vendored_root();
        if !root.exists() {
            return;
        }
        let ids = list_template_ids(&root).unwrap();
        assert!(!ids.is_empty());
        let (surface, _id) = &ids[0];
        // Surface ordering: Image < Video by enum decl order.
        assert!(matches!(surface, Surface::Image | Surface::Video));
    }

    #[test]
    fn loads_single_surface_dir() {
        let mut root = vendored_root();
        root.push("image");
        if !root.exists() {
            return;
        }
        let templates = load_templates_from_surface_dir(&root).unwrap();
        assert!(templates.iter().all(|t| t.surface == Surface::Image));
        assert!(!templates.is_empty());
        // All have a non-empty prompt.
        for t in &templates {
            assert!(!t.prompt.trim().is_empty(), "{} prompt is empty", t.id);
        }
    }

    #[test]
    fn errors_on_non_directory() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let err = load_templates_from_dir(tmp.path()).unwrap_err();
        assert!(matches!(err, LoadError::NotDir(_)));
    }

    #[test]
    fn empty_root_yields_empty_list() {
        let tmp = tempfile::tempdir().unwrap();
        let templates = load_templates_from_dir(tmp.path()).unwrap();
        assert!(templates.is_empty());
    }

    #[test]
    fn reports_json_path_on_parse_error() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("image");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("bad.json"), "{ not valid").unwrap();
        let err = load_templates_from_dir(tmp.path()).unwrap_err();
        match err {
            LoadError::Json { path, .. } => {
                assert!(path.ends_with("image/bad.json"));
            }
            other => panic!("expected Json error, got {other:?}"),
        }
    }

    #[test]
    fn ignores_non_json_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("image");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("README.md"), "# notes").unwrap();
        fs::write(dir.join("x.txt"), "x").unwrap();
        let templates = load_templates_from_dir(tmp.path()).unwrap();
        assert!(templates.is_empty());
    }
}
