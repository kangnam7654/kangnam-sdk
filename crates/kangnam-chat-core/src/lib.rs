//! Pure domain layer for the chat module.
//!
//! Contents:
//! - [`types`] — `Conversation`, `Message`, `SearchResult`, `NewMessage`
//!   (no DB imports — safe to depend on with any feature combo).
//! - [`storage`] — `Storage` async trait + `StorageError`. Pluggable
//!   persistence; pick a backend with a feature flag.
//! - [`json_rpc`] — JSON-RPC 2.0 request/response/notification types.
//! - [`conversations`] (feature `sqlite`) — legacy rusqlite free
//!   functions kept for backwards compatibility with chat-rpc and
//!   chat-server before they migrate to the trait.
//! - [`migrations`] (feature `sqlite`) — DDL for the SQLite schema.
//! - [`export`] (feature `sqlite`) — Markdown / JSON exporters.
//! - [`sqlite_storage`] (feature `sqlite`) — `SqliteStorage` impl.
//! - [`postgres_storage`] (feature `postgres`) — `PostgresStorage` impl.

pub mod json_rpc;
pub mod storage;
pub mod types;

#[cfg(feature = "sqlite")]
pub mod conversations;
#[cfg(feature = "sqlite")]
pub mod export;
#[cfg(feature = "sqlite")]
pub mod migrations;
#[cfg(feature = "sqlite")]
pub mod sqlite_storage;

#[cfg(feature = "postgres")]
pub mod postgres_storage;

pub use storage::{Storage, StorageError, StorageResult};
pub use types::{Conversation, Message, NewMessage, SearchResult};
