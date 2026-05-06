//! Catalog of ready-made image / video generation prompt templates.
//!
//! Each template is a JSON record at `templates/<surface>/<id>.json` with
//! a stable schema (id, surface, title, summary, category, tags, model,
//! aspect, prompt, previewImageUrl, optional previewVideoUrl, source).
//! Adapted from open-design's `prompt-templates/` directory (Apache-2.0)
//! plus upstream curators (YouMind-OpenLab/awesome-gpt-image-2 — CC-BY-4.0,
//! HeyGen/hyperframes — Apache-2.0).
//!
//! ## Distinct from `kangnam-design-prompt`
//!
//! `kangnam-design-prompt` composes daemon-side **system prompts**
//! (DISCOVERY directives + identity charter + active skill body + …).
//! This crate is a **library of end-user prompts** for image / video
//! generation — pick one, fill in any placeholders, hand it to
//! gpt-image-2 / Sora / hyperframes / etc.
//!
//! ## Usage
//!
//! ```no_run
//! use kangnam_design_prompt_template::{load_templates_from_dir, Surface};
//!
//! let templates = load_templates_from_dir("templates/").unwrap();
//! let avatars: Vec<_> = templates
//!     .iter()
//!     .filter(|t| t.surface == Surface::Image && t.has_tag("anime"))
//!     .collect();
//! # let _ = avatars;
//! ```
//!
//! ## Vendored catalog (94 templates)
//!
//! - **Image (44):** profile-avatar (≈19), social-media-post (≈10),
//!   illustration, infographic, e-commerce, vr, game-screenshot, …
//! - **Video (50):** cinematic, hyperframes (≈11), seedance, retro,
//!   character animation, K-pop dance, dragon, …
//!
//! Filter by `surface`, `category`, `tag`, or `model` to build UX pickers.

pub mod loader;

pub use loader::{
    list_template_ids, load_templates_from_dir, load_templates_from_surface_dir, LoadError,
};

use serde::{Deserialize, Serialize};

/// Render surface — drives which downstream model family the template
/// targets. Forward-compatible (`#[non_exhaustive]`) — future variants
/// (e.g. `Audio`) won't break exhaustive matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Surface {
    Image,
    Video,
}

impl Surface {
    /// Filesystem subdirectory name under the templates root.
    pub const fn dir_name(&self) -> &'static str {
        match self {
            Surface::Image => "image",
            Surface::Video => "video",
        }
    }

    /// Parse from the directory name produced by [`Self::dir_name`].
    /// Returns `None` for unknown values (forward-compat).
    pub fn from_dir_name(name: &str) -> Option<Self> {
        match name {
            "image" => Some(Surface::Image),
            "video" => Some(Surface::Video),
            _ => None,
        }
    }
}

/// Provenance pointer for a template — the upstream repo, license, author,
/// and canonical URL. Preserved verbatim from the source JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSource {
    pub repo: String,
    pub license: String,
    pub author: String,
    pub url: String,
}

/// One image / video generation prompt template. Mirrors the open-design
/// JSON schema 1:1 — see the crate README for the upstream license and
/// curator credits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    /// Stable kebab-case id, equals the JSON file stem.
    pub id: String,
    /// Render surface this template targets.
    pub surface: Surface,
    /// Human-readable title, e.g. "Profile / Avatar - Anime Girl …".
    pub title: String,
    /// Multi-sentence prose explaining what the template does.
    pub summary: String,
    /// High-level grouping (e.g. "Profile / Avatar", "Illustration", "Cinematic").
    pub category: String,
    /// Free-form tags; case-sensitive.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Target model — e.g. `"gpt-image-2"`, `"sora-2"`, `"hyperframes-html"`.
    pub model: String,
    /// Aspect ratio — `"1:1"`, `"16:9"`, `"4:3"`, `"9:16"`, …
    pub aspect: String,
    /// The actual prompt body to feed the model. May be multi-paragraph;
    /// preserve newlines exactly when copying out.
    pub prompt: String,
    /// Optional thumbnail / preview image (URL).
    #[serde(default, rename = "previewImageUrl")]
    pub preview_image_url: Option<String>,
    /// Optional preview video (videos only — image templates use only the
    /// image preview).
    #[serde(default, rename = "previewVideoUrl")]
    pub preview_video_url: Option<String>,
    /// Upstream provenance.
    pub source: TemplateSource,
    /// Catch-all for any future fields the schema gains; preserved
    /// verbatim through round-trip.
    #[serde(flatten)]
    pub extras: serde_json::Map<String, serde_json::Value>,
}

impl PromptTemplate {
    /// True when `tag` (case-sensitive) appears in [`Self::tags`].
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// True when [`Self::category`] equals `category` (case-sensitive).
    pub fn in_category(&self, category: &str) -> bool {
        self.category == category
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_round_trip_via_dir_name() {
        for s in [Surface::Image, Surface::Video] {
            assert_eq!(Surface::from_dir_name(s.dir_name()), Some(s));
        }
        assert!(Surface::from_dir_name("audio").is_none());
    }

    #[test]
    fn surface_serde_lowercase() {
        let s: Surface = serde_json::from_str("\"image\"").unwrap();
        assert_eq!(s, Surface::Image);
        assert_eq!(serde_json::to_string(&Surface::Video).unwrap(), "\"video\"");
    }

    #[test]
    fn deserializes_full_template_json() {
        let j = r#"{
            "id": "x",
            "surface": "image",
            "title": "T",
            "summary": "s",
            "category": "Cat",
            "tags": ["a", "b"],
            "model": "gpt-image-2",
            "aspect": "1:1",
            "prompt": "do the thing",
            "previewImageUrl": "https://example.invalid/x.png",
            "source": {
                "repo": "r/o",
                "license": "MIT",
                "author": "a",
                "url": "https://example.invalid"
            }
        }"#;
        let t: PromptTemplate = serde_json::from_str(j).unwrap();
        assert_eq!(t.id, "x");
        assert_eq!(t.surface, Surface::Image);
        assert!(t.has_tag("a"));
        assert!(t.has_tag("b"));
        assert!(!t.has_tag("c"));
        assert!(t.in_category("Cat"));
        assert_eq!(t.preview_image_url.as_deref(), Some("https://example.invalid/x.png"));
        assert!(t.preview_video_url.is_none());
    }

    #[test]
    fn round_trip_preserves_unknown_fields() {
        let j = serde_json::json!({
            "id": "x",
            "surface": "image",
            "title": "T",
            "summary": "s",
            "category": "Cat",
            "tags": [],
            "model": "m",
            "aspect": "1:1",
            "prompt": "p",
            "source": {
                "repo": "r",
                "license": "MIT",
                "author": "a",
                "url": "u"
            },
            "future_field": [1, 2, 3]
        });
        let t: PromptTemplate = serde_json::from_value(j).unwrap();
        let back = serde_json::to_value(&t).unwrap();
        assert_eq!(
            back.get("future_field").unwrap(),
            &serde_json::json!([1, 2, 3])
        );
    }

    #[test]
    fn missing_optional_preview_urls_ok() {
        let j = r#"{
            "id": "x", "surface": "video", "title": "T", "summary": "s",
            "category": "C", "tags": [], "model": "m", "aspect": "16:9",
            "prompt": "p",
            "source": {"repo": "r", "license": "L", "author": "a", "url": "u"}
        }"#;
        let t: PromptTemplate = serde_json::from_str(j).unwrap();
        assert_eq!(t.surface, Surface::Video);
        assert!(t.preview_image_url.is_none());
        assert!(t.preview_video_url.is_none());
    }
}
