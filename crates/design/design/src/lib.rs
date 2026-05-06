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

#[cfg(feature = "craft")]
pub use kangnam_design_craft as craft;

#[cfg(feature = "prompt-template")]
pub use kangnam_design_prompt_template as prompt_template;

#[cfg(feature = "contracts")]
pub use kangnam_design_contracts as contracts;

#[cfg(feature = "html-template")]
pub use kangnam_design_html_template as html_template;

#[cfg(feature = "skill")]
pub use kangnam_design_skill as skill;

#[cfg(feature = "system")]
pub use kangnam_design_system as system;

#[cfg(feature = "direction")]
pub use kangnam_design_direction as direction;

#[cfg(feature = "prompt")]
pub use kangnam_design_prompt as prompt;

#[cfg(feature = "artifact")]
pub use kangnam_design_artifact as artifact;
