//! Slide / Deck document model — PPTX-shaped IR for the kangnam-sdk design family.
//!
//! Pure data + serialization: no I/O, no rendering, no database. The HTML
//! render path, DOM-manifest ingest, and zone override injection live in
//! the sister `design-doc-site` crate, which re-uses the types defined here.

pub mod slide;
pub mod deck;

pub use slide::{
    Background, Fill, Frame, ImageFit, ShapeKind, SlideDoc, SlideElement, Stroke, TextAlign,
    TextStyle, CANVAS_HEIGHT, CANVAS_WIDTH,
};
pub use deck::Deck;
