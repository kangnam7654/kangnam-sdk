//! LLM-driven slide editor for canvas-sdk.
//!
//! Composes [`canvas_llm`] + [`canvas_slide_doc`] to provide:
//! - `Generator` — prompt → `Deck` stream (Task 15)
//! - `ZoneEditor` — slide + screenshot + instruction → element HTML (Task 16)
//! - `SectionEditor` — site section regenerate / add (Task 17)
//! - `SiteGenerator` — prompt → `SiteDoc` stream (Task 18)
//!
//! Prompt templates live next to the code and are embedded via `include_str!`
//! so the crate has no runtime path dependency on the calling application's
//! filesystem layout.

pub mod error;
pub mod generator;
pub mod section_editor;
pub mod site_generator;
pub mod zone_editor;

pub use error::{EditorError, Result};
pub use generator::{CanvasGenerator, GenerationError, GenerationEvent};
pub use section_editor::{
    SectionAddEvent, SectionAddInput, SectionEditError, SectionEditEvent, SectionEditInput,
    SectionEditor,
};
pub use site_generator::{SiteGenerationError, SiteGenerationEvent, SiteGenerator};
pub use zone_editor::{
    BatchEditEvent, BatchEditInput, BatchZone, EditPromptInput, ZoneEditError, ZoneEditEvent,
    ZoneEditor,
};
