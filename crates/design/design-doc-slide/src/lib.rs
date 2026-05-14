//! Slide / Deck document model — PPTX-shaped IR for the kangnam-sdk design family.
//!
//! Pure data + serialization: no I/O, no rendering, no database. The HTML
//! render path, DOM-manifest ingest, and zone override injection live in
//! the sister `design-doc-site` crate, which re-uses the types defined here.

pub mod deck;
pub mod slide;

pub use deck::Deck;
pub use slide::{
    Background, CANVAS_HEIGHT, CANVAS_WIDTH, Fill, Frame, ImageFit, ShapeKind, SlideDoc,
    SlideElement, Stroke, TextAlign, TextStyle,
};
