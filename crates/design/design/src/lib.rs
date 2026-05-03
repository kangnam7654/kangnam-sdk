#![doc = include_str!("../README.md")]

//! kangnam-sdk design family umbrella.
//!
//! Re-exports the design family sister crates under stable module names
//! so consumers depend on a single `design = { ... }` line and pick
//! capabilities through feature flags. Replaces the legacy `canvas` umbrella.

#[cfg(feature = "slide")]
pub use kangnam_design_doc_slide as slide;

#[cfg(feature = "site")]
pub use kangnam_design_doc_site as site;

#[cfg(feature = "llm")]
pub use kangnam_design_llm as llm;

#[cfg(feature = "editor-html")]
pub use kangnam_design_editor_html as editor_html;

#[cfg(feature = "editor-slide")]
pub use kangnam_design_editor_slide as editor_slide;

#[cfg(feature = "editor-site")]
pub use kangnam_design_editor_site as editor_site;

#[cfg(feature = "pptx-write")]
pub use kangnam_design_export_pptx as pptx;
