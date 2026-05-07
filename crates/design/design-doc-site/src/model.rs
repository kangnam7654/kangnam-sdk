//! Data-only types shared across the `design-doc-site` crate.
//!
//! Structs and enums here carry no business logic — only `derive` impls and
//! serde attributes. Logic that operates on these types lives in the module
//! that owns the relevant concern (e.g. `manifest_ingest` for conversion,
//! `site` for HTML rendering).

use serde::Deserialize;
use thiserror::Error;

// ──────────────────────────────────────────────────────────────────────────────
// Manifest ingest types
// ──────────────────────────────────────────────────────────────────────────────

/// Errors produced while converting a walker manifest into a [`SlideDoc`].
///
/// Kept as an enum (rather than `Infallible`) so future manifest versions
/// can surface validation failures without a breaking signature change.
///
/// [`SlideDoc`]: kangnam_design_doc_slide::slide::SlideDoc
#[derive(Debug, Error)]
pub enum ManifestError {
    /// Reserved for future manifest validation failures.
    #[error("invalid manifest: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub slide_id: String,
    #[serde(default)]
    pub width_px: Option<u32>,
    #[serde(default)]
    pub height_px: Option<u32>,
    #[serde(default)]
    pub background: Option<ManifestColor>,
    pub shapes: Vec<ManifestShape>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ManifestShape {
    Text(ManifestText),
    Rect(ManifestRect),
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestText {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub content: String,
    #[serde(default)]
    pub color: Option<ManifestColor>,
    #[serde(default = "default_font_size")]
    pub font_size_px: f32,
    #[serde(default = "default_font_weight")]
    pub font_weight: u32,
    #[serde(default)]
    pub align: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestRect {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    #[serde(default)]
    pub fill: Option<ManifestColor>,
    #[serde(default)]
    pub stroke_color: Option<ManifestColor>,
    #[serde(default)]
    pub stroke_width_px: Option<f32>,
    #[serde(default)]
    pub border_radius_px: f32,
}

/// RGBA color from the DOM walker manifest.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ManifestColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    #[serde(default = "default_alpha")]
    pub a: u8,
}

impl ManifestColor {
    pub(crate) fn to_css(&self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!(
                "rgba({},{},{},{:.3})",
                self.r,
                self.g,
                self.b,
                self.a as f32 / 255.0
            )
        }
    }
}

// ── serde defaults ────────────────────────────────────────────────────────────

pub(crate) fn default_alpha() -> u8 {
    255
}
pub(crate) fn default_font_size() -> f32 {
    24.0
}
pub(crate) fn default_font_weight() -> u32 {
    400
}
