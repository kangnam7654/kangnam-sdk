//! High-level MCP client: handshake + tool listing + tool dispatch.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::transport::{InMemoryTransport, McpTransport, StdioTransport};
use super::types::{McpError, McpTool, McpToolResult, ServerInfo};

/// Information the client advertises about itself during `initialize`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self {
            name: "kangnam-harness-llm-bridge".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }
}

/// MCP protocol version we advertise on `initialize`. Servers built
/// against newer revisions still accept this; the spec is forward-
/// compatible at the handshake level.
const PROTOCOL_VERSION: &str = "2024-11-05";

/// MCP client. Wraps a transport + remembers the server's identity
/// after the `initialize` handshake. Cheap to clone — internal state
/// is `Arc`-shared so the same client can back multiple
/// [`super::McpAgentTool`] adapters without re-handshaking.
#[derive(Clone)]
pub struct McpClient {
    transport: Arc<dyn McpTransport>,
    server_info: Arc<std::sync::Mutex<Option<ServerInfo>>>,
}

impl McpClient {
    /// Construct a client over the given transport. The handshake is
    /// **not** performed automatically — call [`Self::initialize`] before
    /// `list_tools` / `call_tool` for correctness.
    pub fn new(transport: Arc<dyn McpTransport>) -> Self {
        Self {
            transport,
            server_info: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Spawn a stdio MCP server with `command` + `args`, run the
    /// handshake, and return a ready-to-use client. One-stop helper
    /// covering the most common case.
    pub async fn new_stdio(
        command: &str,
        args: &[&str],
        client_info: ClientInfo,
    ) -> Result<Self, McpError> {
        let transport = Arc::new(StdioTransport::spawn(command, args).await?);
        let client = Self::new(transport);
        client.initialize(client_info).await?;
        Ok(client)
    }

    /// Convenience constructor for tests — wraps an `InMemoryTransport`
    /// directly. Skips the handshake; tests that exercise it should
    /// call [`Self::initialize`] explicitly.
    pub fn from_in_memory(transport: InMemoryTransport) -> Self {
        Self::new(Arc::new(transport))
    }

    /// Send the `initialize` request and store the server's reply for
    /// later inspection via [`Self::server_info`].
    pub async fn initialize(&self, client_info: ClientInfo) -> Result<ServerInfo, McpError> {
        let params = json!({
            "protocolVersion": PROTOCOL_VERSION,
            "clientInfo": {
                "name": client_info.name,
                "version": client_info.version,
            },
            // Capability hints — kept minimal; servers ignore unknown keys.
            "capabilities": {
                "tools": {},
            },
        });

        let result = self.transport.request("initialize", params).await?;
        let server_info = parse_server_info(&result)?;

        if let Some(v) = &server_info.protocol_version {
            if v != PROTOCOL_VERSION {
                tracing::warn!(
                    "mcp: server protocol version {v} differs from client {PROTOCOL_VERSION}; \
                     proceeding optimistically"
                );
            }
        }

        *self.server_info.lock().unwrap_or_else(|e| e.into_inner()) = Some(server_info.clone());
        Ok(server_info)
    }

    /// Snapshot of the server info captured during `initialize`.
    pub fn server_info(&self) -> Option<ServerInfo> {
        self.server_info
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Discover tools the server advertises. Cursor pagination is part
    /// of the MCP spec — for v0.3.x we treat the first page as the full
    /// list and log a `tracing::warn!` if the server signals more
    /// pages. Round 24 will pull additional pages.
    pub async fn list_tools(&self) -> Result<Vec<McpTool>, McpError> {
        let result = self.transport.request("tools/list", json!({})).await?;

        if result
            .get("nextCursor")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty())
        {
            tracing::warn!(
                "mcp: server returned nextCursor on tools/list — pagination not yet implemented; \
                 returning first page only"
            );
        }

        let tools = result
            .get("tools")
            .ok_or_else(|| McpError::Protocol("tools/list missing 'tools' field".into()))?;
        let tools: Vec<McpTool> = serde_json::from_value(tools.clone())?;
        Ok(tools)
    }

    /// Invoke a tool by name with the given JSON-encoded arguments.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<McpToolResult, McpError> {
        let result = self
            .transport
            .request(
                "tools/call",
                json!({
                    "name": name,
                    "arguments": arguments,
                }),
            )
            .await?;
        let parsed: McpToolResult = serde_json::from_value(result)?;
        Ok(parsed)
    }

    /// Direct transport access for callers that need to issue methods
    /// the high-level API doesn't model yet. The typical use case is
    /// `prompts/list` or `resources/list`.
    pub fn transport(&self) -> &Arc<dyn McpTransport> {
        &self.transport
    }
}

fn parse_server_info(result: &serde_json::Value) -> Result<ServerInfo, McpError> {
    // MCP spec puts {name, version} under `serverInfo` and the protocol
    // version at top-level. Handle both shapes (some servers put name
    // at top level for backwards compat).
    let protocol_version = result
        .get("protocolVersion")
        .and_then(|v| v.as_str())
        .map(String::from);

    if let Some(info) = result.get("serverInfo") {
        let mut parsed: ServerInfo = serde_json::from_value(info.clone())
            .map_err(|e| McpError::Protocol(format!("serverInfo deserialise: {e}")))?;
        if parsed.protocol_version.is_none() {
            parsed.protocol_version = protocol_version;
        }
        return Ok(parsed);
    }

    if let (Some(name), Some(version)) = (
        result.get("name").and_then(|v| v.as_str()),
        result.get("version").and_then(|v| v.as_str()),
    ) {
        return Ok(ServerInfo {
            name: name.into(),
            version: version.into(),
            protocol_version,
        });
    }

    Err(McpError::Protocol(
        "initialize response missing serverInfo".into(),
    ))
}
