//! LLM-driven slide deck generator + section editor for the kangnam-sdk
//! design family. Composes `design-llm` + `design-doc-slide`.
//!
//! - `CanvasGenerator` — prompt → `Deck` stream.
//! - `SectionEditor` — slide section regenerate / add.
//!
//! Prompt templates live next to the code and are embedded via `include_str!`.

pub mod error;
pub mod generator;
pub mod section_editor;

pub use error::{EditorError, Result};
pub use generator::{CanvasGenerator, GenerationError, GenerationEvent};
pub use section_editor::{
    SectionAddEvent, SectionAddInput, SectionEditError, SectionEditEvent, SectionEditInput,
    SectionEditor,
};
