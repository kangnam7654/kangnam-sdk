//! Axum WebSocket + MCP server for the chat module.
//!
//! Hosts (Tauri apps, headless services, test runners) build a
//! [`ServerContext`] from their own state — Arc-wrapped CLI manager,
//! DB connection, and pending-permission map plus the broadcast
//! channels that fan out streaming events — and call [`start`] with
//! a [`ServerConfig`] to mount the routes and serve.
//!
//! Routes:
//! - `GET /ws` — JSON-RPC over WebSocket; dispatches `cli.*` methods
//!   via [`kangnam_chat_rpc::dispatch`] and forwards broadcast notifications
//!   to subscribers.
//! - `GET /mcp` — Server-Sent Events handshake for the MCP protocol.
//! - `POST /mcp` — JSON-RPC MCP endpoint exposing one tool, `approve`,
//!   used by Claude Code to request user permission before risky
//!   actions.

use std::sync::{Arc, Mutex as StdMutex};

use rusqlite::Connection;
use tokio::sync::Mutex as AsyncMutex;

use kangnam_chat_agent::CliManager;
use kangnam_chat_rpc::{MessageGuard, PendingPermissions};

pub mod auth;
pub mod broadcast;

mod mcp;
mod router;
mod saver;
mod ws;

pub use auth::{AuthError, AuthHook, AuthParams, UserContext};

/// Resources the server needs from the host application.
///
/// Each field is `Arc`-wrapped so the server task can hold a 'static
/// handle. Hosts typically build this once on startup from their own
/// AppState struct. The two `*_hook` fields default to `None` —
/// hosts opt into auth and per-message guarding by setting them.
pub struct ServerContext {
    pub cli_manager: Arc<AsyncMutex<CliManager>>,
    pub db: Arc<StdMutex<Connection>>,
    pub pending_permissions: Arc<AsyncMutex<PendingPermissions>>,
    pub broadcast_tx: broadcast::BroadcastTx,
    pub enhanced_broadcast_tx: broadcast::EnhancedBroadcastTx,
    /// Optional WebSocket-upgrade authenticator. When `None`, every
    /// upgrade is accepted and the dispatch context carries
    /// `user_id = None`. When `Some`, the upgrade is rejected with
    /// `401` if the hook returns `Err`.
    pub auth_hook: Option<Arc<dyn AuthHook>>,
    /// Optional pre-message check. Called before each
    /// `cli.sendMessage` is forwarded to the manager. Used by hosts
    /// that bill per turn or enforce rate limits — see
    /// [`kangnam_chat_rpc::MessageGuard`].
    pub message_guard: Option<Arc<dyn MessageGuard>>,
}

/// Server-level configuration: bind port, optional static dir, and
/// MCP `serverInfo` identity.
pub struct ServerConfig {
    /// TCP port to bind on `127.0.0.1`.
    pub port: u16,
    /// Optional directory to serve statically at `/`. Used in
    /// production builds where the frontend is bundled alongside
    /// the server.
    pub static_dir: Option<String>,
    /// MCP `serverInfo.name`. Hosts should set this to their own
    /// identifier so Claude Code can distinguish servers.
    pub server_name: String,
    /// MCP `serverInfo.version`.
    pub server_version: String,
    /// MCP tool name namespace prefix. Claude Code surfaces the tool
    /// as `mcp__<server_name>__approve`. Hosts pass this same prefix
    /// to the Claude adapter via `mcp_port` / config and it must
    /// match the `--permission-prompt-tool` value the adapter sends.
    pub mcp_tool_namespace: String,
}

/// Start the HTTP server with WebSocket + MCP routes mounted.
///
/// Spawns the message-saver background task automatically. Blocks
/// until the listener errors or the process is killed.
pub async fn start(
    ctx: Arc<ServerContext>,
    config: ServerConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = router::create_router(ctx.clone(), &config);

    saver::start_message_saver(ctx.clone());

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], config.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    eprintln!("[chat-server] Listening on ws://localhost:{}", config.port);
    axum::serve(listener, app).await?;
    Ok(())
}
