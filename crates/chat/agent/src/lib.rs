//! Persistent CLI agent session manager.
//!
//! Spawns Claude Code / Codex CLI as long-running subprocesses, parses
//! their stream-json output into rich semantic events ([`UnifiedMessage`]
//! and [`ClaudeEnhancedEvent`]), and exposes them through a
//! transport-agnostic [`AgentEventSink`].
//!
//! Hosts (Tauri apps, standalone servers, test harnesses) plug in their
//! own sink and feed events to whatever transport they use — WebSocket,
//! Tauri event, in-memory channel, etc.
//!
//! # Quick layout
//! - [`adapter`] — `CliAdapter` trait, one impl per provider
//! - [`adapters`] — `claude` / `codex` parser implementations
//! - [`manager`] — `CliManager` orchestrating CLI session lifecycle
//! - [`http_manager`] — `HttpSessionManager` for HTTP LLM providers
//!   (Anthropic, OpenAI, Gemini, etc. via `llm-router`). Uses the
//!   `kangnam_chat_core::Storage` trait for history persistence.
//! - [`sink`] — `AgentEventSink` trait
//! - [`registry`] — provider metadata
//! - [`types`] — `UnifiedMessage`, `ClaudeEnhancedEvent`, etc.

pub mod adapter;
pub mod adapters;
pub mod http_manager;
pub mod manager;
pub mod registry;
pub mod sink;
pub mod types;

pub use adapter::CliAdapter;
pub use http_manager::{HttpSessionManager, ProviderFactory, StaticConfig};
pub use manager::CliManager;
pub use sink::AgentEventSink;
pub use types::{ClaudeEnhancedEvent, CliStatus, UnifiedMessage};
