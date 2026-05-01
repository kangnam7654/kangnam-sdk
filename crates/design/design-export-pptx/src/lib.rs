//! design-export-pptx — write editable .pptx files from a neutral IR.
//!
//! See the crate README for an overview and the `write_deck` docs for
//! the entry point.

pub mod color;
pub mod color_convert;
pub mod deck;
pub mod element;
pub mod error;
pub mod geometry;
pub mod template;
pub mod writer;

pub use color::{Background, Color, Fill, GradientStop, Stroke};
pub use color_convert::parse_css_color;
pub use deck::{PptxDeck, PptxSlide};
pub use element::{
    ImageBox, ImageFit, ImageMime, OuterShadow, PptxElement, ShapeBox, ShapeKind, TextAlign,
    TextBox, TextStyle,
};
pub use error::{PptxWriteError, Result};
pub use geometry::Frame;
pub use template::{FontVariant, PptxTemplate, SlideRef};
pub use writer::{write_deck, write_deck_to_bytes};

#[cfg(feature = "slide-doc")]
pub mod from_slide_doc;

#[cfg(feature = "slide-doc")]
pub use from_slide_doc::{from_deck, from_slide_doc as slide_doc_to_pptx, FromSlideDocError};
