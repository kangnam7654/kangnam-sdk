//! Filesystem catalog loader for prompt templates.
//!
//! Each `templates/<surface>/<id>.json` file deserializes to one
//! [`PromptTemplate`]. The on-disk layout is fixed:
//!
//! ```text
//! templates/
//! ├── image/
//! │   ├── <id>.json
//! │   └── …
//! └── video/
//!     ├── <id>.json
//!     └── …
//! ```

use std::fs;
use std::path::Path;

use kangnam_design_catalog_core as catalog;

use crate::model::{PromptTemplate, Surface};

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json parse error in {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("missing template `{0}` (file not found)")]
    Missing(String),
}

/// Internal error covering both io and json parse.
#[derive(Debug, thiserror::Error)]
enum ParseTemplateError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json parse error in {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: serde_json::Error,
    },
}

impl From<catalog::CatalogError> for LoadError {
    fn from(e: catalog::CatalogError) -> Self {
        match e {
            catalog::CatalogError::Io(e) => LoadError::Io(e),
            catalog::CatalogError::NotFound(id) => LoadError::Missing(id),
            catalog::CatalogError::Parse { id: _, source } => {
                if let Ok(inner) = source.downcast::<ParseTemplateError>() {
                    match *inner {
                        ParseTemplateError::Io(e) => LoadError::Io(e),
                        ParseTemplateError::Parse { path, source } => {
                            LoadError::Parse { path, source }
                        }
                    }
                } else {
                    LoadError::Io(std::io::Error::other("unknown parse error"))
                }
            }
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────

fn surface_dir(surface: Surface) -> &'static str {
    match surface {
        Surface::Image => "image",
        Surface::Video => "video",
    }
}

/// Filter: accept `.json` files (non-empty stem), skip everything else.
fn json_filter(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    let name = path.file_name()?.to_str()?;
    let stem = name.strip_suffix(".json")?;
    if stem.is_empty() {
        return None;
    }
    Some(stem.to_string())
}

fn parse_template(_id: &str, path: &Path) -> Result<PromptTemplate, ParseTemplateError> {
    let body = fs::read_to_string(path)?;
    serde_json::from_str(&body).map_err(|source| ParseTemplateError::Parse {
        path: path.display().to_string(),
        source,
    })
}

// ── public API ────────────────────────────────────────────────────────────

/// List the slugs of every `<id>.json` file under
/// `<root>/<surface>/`. Returns sorted ids.
pub fn list_template_ids(
    root: impl AsRef<Path>,
    surface: Surface,
) -> Result<Vec<String>, LoadError> {
    let dir = root.as_ref().join(surface_dir(surface));
    if !dir.exists() {
        return Ok(Vec::new());
    }
    catalog::list_ids(&dir, json_filter).map_err(LoadError::from)
}

/// Load every template under `<root>/<surface>/`. Returns templates sorted
/// by id. Errors short-circuit on the first malformed JSON file.
pub fn load_templates_from_dir(
    root: impl AsRef<Path>,
    surface: Surface,
) -> Result<Vec<PromptTemplate>, LoadError> {
    let dir = root.as_ref().join(surface_dir(surface));
    if !dir.exists() {
        return Ok(Vec::new());
    }
    catalog::load_dir(&dir, json_filter, parse_template).map_err(LoadError::from)
}

/// Load both surfaces in one call, concatenated and sorted by `(surface, id)`.
pub fn load_all_templates(root: impl AsRef<Path>) -> Result<Vec<PromptTemplate>, LoadError> {
    let mut out = load_templates_from_dir(&root, Surface::Image)?;
    out.extend(load_templates_from_dir(&root, Surface::Video)?);
    Ok(out)
}

/// Load one template by `(surface, id)`. Returns [`LoadError::Missing`] when
/// the JSON file isn't present.
pub fn load_template(
    root: impl AsRef<Path>,
    surface: Surface,
    id: &str,
) -> Result<PromptTemplate, LoadError> {
    let path = root
        .as_ref()
        .join(surface_dir(surface))
        .join(format!("{id}.json"));
    if !path.exists() {
        return Err(LoadError::Missing(id.to_string()));
    }
    parse_template(id, &path).map_err(|e| match e {
        ParseTemplateError::Io(e) => LoadError::Io(e),
        ParseTemplateError::Parse { path, source } => LoadError::Parse { path, source },
    })
}

// ── tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
        let image_ids = list_template_ids(&root, Surface::Image).unwrap();
        let video_ids = list_template_ids(&root, Surface::Video).unwrap();
        // Catalog ships 44 image + 50 video prompts (open-design 165f8f7).
        assert!(
            image_ids.len() >= 30,
            "expected at least 30 image templates, got {}",
            image_ids.len()
        );
        assert!(
            video_ids.len() >= 30,
            "expected at least 30 video templates, got {}",
            video_ids.len()
        );
    }

    #[test]
    fn loads_one_image_template_with_attribution() {
        let root = vendored_root();
        let canonical = "3d-stone-staircase-evolution-infographic";
        if !root
            .join("image")
            .join(format!("{canonical}.json"))
            .exists()
        {
            return;
        }
        let t = load_template(&root, Surface::Image, canonical).unwrap();
        assert_eq!(t.id, canonical);
        assert_eq!(t.surface, Surface::Image);
        assert!(!t.prompt.is_empty());
        let src = t.source.expect("vendored image prompts carry attribution");
        assert!(!src.repo.is_empty());
        assert!(
            !src.license.is_empty(),
            "license required for CC-BY catalog"
        );
    }

    #[test]
    fn missing_template_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let err = load_template(tmp.path(), Surface::Image, "no-such-thing").unwrap_err();
        assert!(matches!(err, LoadError::Missing(_)));
    }

    #[test]
    fn empty_dir_yields_empty_list() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(
            load_templates_from_dir(tmp.path(), Surface::Image)
                .unwrap()
                .is_empty()
        );
        assert!(
            list_template_ids(tmp.path(), Surface::Image)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn loads_sorted_by_id() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("image");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("zeta.json"),
            r#"{"id":"zeta","surface":"image","title":"Z","prompt":"z"}"#,
        )
        .unwrap();
        fs::write(
            dir.join("alpha.json"),
            r#"{"id":"alpha","surface":"image","title":"A","prompt":"a"}"#,
        )
        .unwrap();
        let templates = load_templates_from_dir(tmp.path(), Surface::Image).unwrap();
        assert_eq!(templates.len(), 2);
        assert_eq!(templates[0].id, "alpha");
        assert_eq!(templates[1].id, "zeta");
    }

    #[test]
    fn malformed_json_propagates_parse_error() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("image");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("broken.json"), "{ not valid json").unwrap();
        let err = load_templates_from_dir(tmp.path(), Surface::Image).unwrap_err();
        assert!(matches!(err, LoadError::Parse { .. }));
    }

    #[test]
    fn load_all_templates_combines_surfaces() {
        let tmp = tempfile::tempdir().unwrap();
        let img = tmp.path().join("image");
        let vid = tmp.path().join("video");
        fs::create_dir_all(&img).unwrap();
        fs::create_dir_all(&vid).unwrap();
        fs::write(
            img.join("a.json"),
            r#"{"id":"a","surface":"image","title":"A","prompt":"a"}"#,
        )
        .unwrap();
        fs::write(
            vid.join("b.json"),
            r#"{"id":"b","surface":"video","title":"B","prompt":"b"}"#,
        )
        .unwrap();
        let all = load_all_templates(tmp.path()).unwrap();
        assert_eq!(all.len(), 2);
        // Image first (Surface::Image deserializes from "image"), then video.
        assert_eq!(all[0].surface, Surface::Image);
        assert_eq!(all[1].surface, Surface::Video);
    }
}
