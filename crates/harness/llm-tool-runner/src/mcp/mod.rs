//! Model Context Protocol (MCP) client + `AgentTool` adapter.
//!
//! Lets [`crate::LlmAgent`] consume tools advertised by an external
//! MCP server. The server is spawned over stdio (e.g. an `npx`-launched
//! filesystem MCP server, a Python script, or any executable that
//! speaks line-delimited JSON-RPC on stdin/stdout per the MCP spec).
//!
//! # Architecture
//!
//! ```text
//!   ┌─ LlmAgent (autonomous tool-call loop)
//!   │     │
//!   │     ▼ via `with_mcp_stdio(...)`
//!   │   McpAgentTool ── implements AgentTool<C>
//!   │     │
//!   │     ▼ delegates execute() → call_tool()
//!   │   McpClient (Arc) ── JSON-RPC over McpTransport
//!   │     │
//!   │     ▼
//!   └─ StdioTransport ── child process (stdin/stdout pipes)
//! ```
//!
//! `McpTransport` is a trait so tests can plug in an in-memory
//! transport (`InMemoryTransport`) that pre-canned-responds to every
//! request id. Production builds use [`StdioTransport`].
//!
//! # MCP wire protocol
//!
//! Spec: <https://modelcontextprotocol.io/specification/>
//!
//! - **Framing**: line-delimited JSON (newline-separated JSON-RPC 2.0
//!   request/response objects on the wire).
//! - **Handshake**: client sends `initialize` with `clientInfo` and
//!   capability hints; server replies with `serverInfo` and its
//!   capabilities.
//! - **Tool discovery**: `tools/list` returns
//!   `{ tools: [{ name, description, inputSchema }, ...] }`. Cursor
//!   pagination is part of the spec but we treat the result as the
//!   complete list for now (round 24 will add `nextCursor` support).
//! - **Tool dispatch**: `tools/call { name, arguments }` returns
//!   `{ content: [{type, text|...}, ...], isError? }`. We flatten the
//!   `text`-typed content blocks into a single result string so the
//!   LLM sees a clean payload.
//! - **Errors**: JSON-RPC wraps tool failures in `error: { code, message }`
//!   or in `result.isError == true` with text content describing the
//!   failure. Both map to `McpError::Server` / [`crate::ToolInvocation::is_error`].
//!
//! # Scope
//!
//! v0.3.x ships **stdio transport only**. SSE / WebSocket transports,
//! ping-pong heartbeats, cursor pagination, and capability negotiation
//! beyond `initialize` are deferred to round 24. The public API is
//! shaped so adding them is backwards-compatible.

mod agent_tool;
mod client;
pub mod transport;
mod types;

pub use agent_tool::McpAgentTool;
pub use client::{ClientInfo, McpClient};
pub use transport::{InMemoryTransport, McpTransport, StdioTransport};
pub use types::{McpError, McpTool, McpToolContent, McpToolResult, ServerInfo};
