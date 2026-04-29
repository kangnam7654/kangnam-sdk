//! Shared error type for the editor crate.
//!
//! All four editor pipelines (generator, zone_editor, section_editor,
//! site_generator) share one [`EditorError`] so callers can match one
//! exhaustive enum. Upstream error variants (`tera`, `AiError`,
//! `serde_json`) are wrapped via `#[from]` so `?` works with the original
//! types.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, EditorError>;

#[derive(Debug, Error)]
pub enum EditorError {
    #[error("template render failed: {0}")]
    Template(#[from] tera::Error),
    #[error("AI provider error: {0}")]
    Ai(#[from] canvas_llm::AiError),
    #[error("JSON parse failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("LLM response did not contain expected payload: {0}")]
    MalformedResponse(String),
}
