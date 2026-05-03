#![cfg_attr(not(test), deny(clippy::unwrap_used))]

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
//!
//! # Use without CLI subprocess sessions
//!
//! `kangnam-chat-core` is a *pure-data + storage* layer with no
//! dependency on the heavier `kangnam-chat-agent` /
//! `kangnam-chat-rpc` / `kangnam-chat-server` crates. Domain apps
//! that already have their own conversation storage (e.g. Travel
//! Planner's `chat_messages` + `plan_revisions` tables) but want
//! the SDK's `Conversation` / `Message` shapes can depend on this
//! crate alone:
//!
//! ```ignore
//! // App-native chat — no CLI agent runtime, no WS server.
//! use kangnam_chat_core::types::{Conversation, Message, NewMessage};
//! use kangnam_chat_core::storage::Storage;
//!
//! let storage: Box<dyn Storage> = /* your sqlite / postgres / in-memory impl */;
//! let conv = storage.create_conversation("planning").await?;
//! storage.append_message(NewMessage {
//!     conversation_id: conv.id.clone(),
//!     role: "user".into(),
//!     content: "Plan a 5-day Seoul trip".into(),
//!     ..Default::default()
//! }).await?;
//! ```
//!
//! The CLI agent layer (`kangnam-chat-agent`), JSON-RPC dispatcher
//! (`kangnam-chat-rpc`), and Axum WebSocket server
//! (`kangnam-chat-server`) are *optional* layers built on top —
//! pick them up only when you want Claude Code / Codex subprocess
//! integration, the suspend/resume tool flow, or a hosted WS endpoint.

pub mod error;
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

pub use error::{Result, StorageError};
pub use storage::Storage;
pub use types::{Conversation, Message, NewMessage, SearchResult};
