use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("not found: {kind} id={id}")]
    NotFound { kind: &'static str, id: String },

    #[cfg(feature = "sqlite")]
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error(transparent)]
    Core(#[from] kangnam_harness_core::HarnessError),

    #[error("{0}")]
    Other(String),
}
