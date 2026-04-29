//! Pluggable persistence trait.
//!
//! The chat module persists two kinds of records — conversations and
//! their messages — and exposes a small set of CRUD-ish operations on
//! top. Hosts pick a backend by enabling the corresponding feature
//! flag (`sqlite`, `postgres`) and pass an `Arc<dyn Storage>` to the
//! RPC dispatcher / saver.
//!
//! ## Why async
//!
//! sqlx is async, rusqlite is sync. The trait is async so a single
//! caller signature works for both; the SQLite impl wraps blocking
//! calls in `tokio::task::spawn_blocking`.
//!
//! ## What's NOT here
//!
//! - Connection pooling, transactions across multiple ops — backends
//!   handle their own.
//! - Search ranking / pagination — the current chat-sdk only needs
//!   simple LIKE search and full-load reads. Add as needed.

use async_trait::async_trait;
use thiserror::Error;

use crate::types::{Conversation, Message, NewMessage, SearchResult};

/// Errors a storage backend can surface to callers.
///
/// Backend-specific error types convert into `Backend(String)` via
/// `From` impls in their respective modules. `NotFound` is reserved
/// for "this id doesn't exist" so callers can branch on it without
/// string-matching.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("not found")]
    NotFound,
    #[error("backend error: {0}")]
    Backend(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}

pub type StorageResult<T> = Result<T, StorageError>;

/// Async, object-safe persistence interface.
///
/// All methods return `StorageResult` so callers can branch on
/// `NotFound` without parsing error strings. Implementors are
/// expected to be cheaply cloneable (e.g. wrap a `sqlx::PgPool` or
/// `Arc<Mutex<rusqlite::Connection>>`) so callers can hand out
/// `Arc<dyn Storage>` freely.
#[async_trait]
pub trait Storage: Send + Sync {
    /// Run any backend-specific schema setup. Idempotent.
    async fn migrate(&self) -> StorageResult<()>;

    // -- conversations --

    async fn list_conversations(&self) -> StorageResult<Vec<Conversation>>;

    async fn create_conversation(
        &self,
        cli_provider: &str,
        model: Option<&str>,
    ) -> StorageResult<Conversation>;

    async fn get_conversation(&self, id: &str) -> StorageResult<Option<Conversation>>;

    /// True if a conversation with this id exists. Cheaper than
    /// `get_conversation` because it doesn't allocate a row.
    async fn conversation_exists(&self, id: &str) -> StorageResult<bool>;

    /// Insert a conversation row with the given id if it doesn't
    /// already exist. Used by chat-rpc's lazy-create path on first
    /// `cli.sendMessage` after `cli.startSession`.
    async fn ensure_conversation(
        &self,
        id: &str,
        cli_provider: &str,
    ) -> StorageResult<()>;

    async fn delete_conversation(&self, id: &str) -> StorageResult<()>;

    async fn delete_all_conversations(&self) -> StorageResult<()>;

    async fn update_title(&self, id: &str, title: &str) -> StorageResult<()>;

    async fn toggle_pin(&self, id: &str) -> StorageResult<()>;

    /// If the conversation's title is still the placeholder
    /// (`"New Chat"`), derive a title from the first non-empty line
    /// of `user_message` (truncated to 40 chars). No-op otherwise.
    async fn auto_title_if_needed(
        &self,
        conversation_id: &str,
        user_message: &str,
    ) -> StorageResult<()>;

    // -- messages --

    async fn get_messages(&self, conversation_id: &str) -> StorageResult<Vec<Message>>;

    async fn add_message(
        &self,
        conversation_id: &str,
        msg: NewMessage<'_>,
    ) -> StorageResult<Message>;

    async fn search_messages(&self, query: &str) -> StorageResult<Vec<SearchResult>>;
}

// -- shared helpers --

/// Compute the auto-title from a user message. Backends share this
/// logic so the title text stays identical across SQLite and Postgres.
pub fn derive_auto_title(user_message: &str) -> Option<String> {
    let trimmed = user_message.trim();
    if trimmed.is_empty() {
        return None;
    }
    let first_line = trimmed.lines().next().unwrap_or(trimmed);
    let title = if first_line.chars().count() > 40 {
        let cutoff = first_line
            .char_indices()
            .nth(40)
            .map(|(i, _)| i)
            .unwrap_or(first_line.len());
        format!("{}…", &first_line[..cutoff])
    } else {
        first_line.to_string()
    };
    Some(title)
}

#[allow(dead_code)] // unused when neither sqlite nor postgres feature is enabled
pub(crate) fn now_ts() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
