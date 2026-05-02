//! Error type for chat-core storage operations.
//!
//! Backend-specific error types convert into `Backend(String)` via
//! `From` impls in their respective modules. `NotFound` is reserved
//! for "this id doesn't exist" so callers can branch on it without
//! string-matching.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("not found")]
    NotFound,
    #[error("backend error: {0}")]
    Backend(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;
