//! Typed [`PromptTemplate`] and the JSON schema it deserializes from.
//!
//! The JSON shape mirrors open-design's `prompt-templates/<surface>/*.json`
//! with original `previewImageUrl` / `previewVideoUrl` / `source` fields
//! preserved so attribution-required licenses (CC-BY-4.0, etc.) remain
//! intact at runtime.

use serde::{Deserialize, Serialize};

/// Image vs video — drives which preview field is meaningful.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Surface {
    Image,
    Video,
}

/// Original-source attribution. Required for CC-BY-4.0 prompts in the
/// vendored catalog; preserved verbatim from upstream JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceAttribution {
    /// Upstream repo slug (e.g. `YouMind-OpenLab/awesome-gpt-image-2`).
    pub repo: String,
    /// SPDX-style license id (e.g. `CC-BY-4.0`).
    pub license: String,
    /// Human-readable attribution name.
    pub author: String,
    /// Canonical URL for the prompt's source post / file.
    pub url: String,
}

/// One prompt template — image or video — keyed by `id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    /// Filename slug (matches the catalog file `templates/<surface>/<id>.json`).
    pub id: String,
    /// `image` | `video`.
    pub surface: Surface,
    /// Human-readable title.
    pub title: String,
    /// One-line description of what the prompt produces.
    #[serde(default)]
    pub summary: String,
    /// Free-form category (`Infographic`, `Profile Avatar`, `Social Media`, …).
    #[serde(default)]
    pub category: String,
    /// Free-form tags (`3d-render`, `cinematic`, …).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Target model (`gpt-image-2`, `seedance-2.0`, …).
    #[serde(default)]
    pub model: String,
    /// Aspect ratio hint (`1:1`, `16:9`, `9:16`, `4:5`).
    #[serde(default)]
    pub aspect: String,
    /// The actual prompt body — feed verbatim to the target model.
    pub prompt: String,
    /// Preview image URL (always present for image surface, often present
    /// for video surface as a thumbnail).
    #[serde(default, rename = "previewImageUrl")]
    pub preview_image_url: Option<String>,
    /// Preview video URL (video surface only).
    #[serde(default, rename = "previewVideoUrl")]
    pub preview_video_url: Option<String>,
    /// Required attribution for CC-BY-4.0 catalog entries.
    #[serde(default)]
    pub source: Option<SourceAttribution>,
    /// Catch-all for any unknown future fields.
    #[serde(flatten)]
    pub extras: serde_json::Map<String, serde_json::Value>,
}
