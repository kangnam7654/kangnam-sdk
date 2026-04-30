use thiserror::Error;

pub type Result<T> = std::result::Result<T, EditorError>;

#[derive(Debug, Error)]
pub enum EditorError {
    #[error("template render failed: {0}")]
    Template(#[from] tera::Error),
    #[error("AI provider error: {0}")]
    Ai(#[from] design_llm::AiError),
    #[error("JSON parse failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("LLM response did not contain expected payload: {0}")]
    MalformedResponse(String),
}
