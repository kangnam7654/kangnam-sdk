//! Generic HTML zone editor for the kangnam-sdk design family.
//!
//! `ZoneEditor` operates on raw HTML strings and `data-edit-zone="..."`
//! markers — domain-agnostic across slides, landing pages, dashboards,
//! mobile mockups, and motion frames. Depends only on `design-llm`.

pub mod error;
pub mod zone_editor;

pub use error::{EditorError, Result};
pub use zone_editor::{
    BatchEditEvent, BatchEditInput, BatchZone, EditPromptInput, ZoneEditError, ZoneEditEvent,
    ZoneEditor,
};
