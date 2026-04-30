//! Slide/Deck/Site document model for the Canvas SDK.
//!
//! This crate is pure data + rendering: no I/O, no database, no HTTP.

pub mod slide;
pub mod deck;
pub mod site;
pub mod html_render;
pub mod manifest_ingest;
pub mod inject;

pub use slide::{
    Background, Fill, Frame, ImageFit, ShapeKind, SlideDoc, SlideElement, Stroke, TextAlign,
    TextStyle, CANVAS_HEIGHT, CANVAS_WIDTH,
};
pub use deck::Deck;
pub use site::SiteDoc;
