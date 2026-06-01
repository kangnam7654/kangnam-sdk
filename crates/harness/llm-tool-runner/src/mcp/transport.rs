//! Transport abstraction for MCP JSON-RPC messages.
//!
//! `McpTransport` is the trait every transport implements. Production
//! ships [`StdioTransport`] (spawned child process, stdin/stdout
//! pipes, line-delimited JSON-RPC). Tests use [`InMemoryTransport`]
//! to script canned responses without a real subprocess.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin};
use tokio::sync::{Mutex as TokioMutex, oneshot};
use tokio::time::timeout;

use super::types::McpError;

/// JSON-RPC 2.0 request frame the client emits.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct JsonRpcRequest {
    pub(crate) jsonrpc: &'static str,
    pub(crate) id: String,
    pub(crate) method: String,
    pub(crate) params: Value,
}

/// JSON-RPC 2.0 response frame the server emits.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct JsonRpcResponse {
    #[allow(dead_code)]
    #[serde(default)]
    pub(crate) jsonrpc: Option<String>,
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) result: Option<Value>,
    #[serde(default)]
    pub(crate) error: Option<JsonRpcErrorBody>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct JsonRpcErrorBody {
    pub(crate) code: i64,
    pub(crate) message: String,
    #[serde(default)]
    pub(crate) data: Option<Value>,
}

/// Async transport for MCP JSON-RPC. `request` returns the parsed
/// `result` value or maps the structured `error` envelope into
/// [`McpError::Server`].
#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn request(&self, method: &str, params: Value) -> Result<Value, McpError>;
}

// ── stdio transport ─────────────────────────────────────────────────

/// Stdio transport — spawns a child process, writes line-delimited
/// JSON-RPC requests to its stdin, reads responses from its stdout
/// in a background task, and routes each response back to the
/// awaiting request via a `oneshot::Sender` keyed on request id.
///
/// Cancel-safe: drop the transport to send SIGKILL via
/// `Child::kill_on_drop(true)`. In-flight requests resolve to
/// [`McpError::Closed`] when the reader task observes EOF.
pub struct StdioTransport {
    /// Pending response senders, keyed by JSON-RPC request id. The
    /// reader task pops + fires when a response arrives.
    pending: Arc<StdMutex<HashMap<String, oneshot::Sender<JsonRpcResponse>>>>,
    /// Writer half of the child's stdin. Wrapped in `tokio::sync::Mutex`
    /// because the lock is held across the `.await` of each
    /// `write_all` / `flush` call — `std::sync::MutexGuard` is `!Send`
    /// across awaits, which breaks `LlmAgent::run` (boxed future is
    /// `Send`-bounded).
    stdin: TokioMutex<ChildStdin>,
    /// Per-request timeout. Defaults to 30s; configurable via
    /// [`Self::with_timeout`].
    timeout: Duration,
    /// Owns the child process so it lives as long as the transport.
    /// Not used after construction; the `_` prefix marks it as a
    /// lifetime anchor. Wrapped in `Mutex` to satisfy `Sync` even
    /// though only `Drop` accesses it.
    _child: StdMutex<Option<Child>>,
}

impl StdioTransport {
    /// Spawn `command` with the given `args` and start the JSON-RPC
    /// reader loop. Returns once the child has been spawned and the
    /// stdin/stdout pipes are wired up — does **not** wait for the
    /// child's `initialize` handshake; call
    /// [`super::client::McpClient::new_stdio`] for the full handshake.
    pub async fn spawn(command: &str, args: &[&str]) -> Result<Self, McpError> {
        let mut cmd = tokio::process::Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .map_err(|e| McpError::Transport(format!("spawn '{command}' failed: {e}")))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpError::Transport("child stdin not piped".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpError::Transport("child stdout not piped".into()))?;

        let pending: Arc<StdMutex<HashMap<String, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(StdMutex::new(HashMap::new()));
        let pending_clone = Arc::clone(&pending);

        // Reader task: line-by-line parse, route by id.
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            loop {
                match reader.next_line().await {
                    Ok(Some(line)) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<JsonRpcResponse>(trimmed) {
                            Ok(resp) => {
                                if let Some(tx) = pending_clone
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner())
                                    .remove(&resp.id)
                                {
                                    let _ = tx.send(resp);
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "mcp stdio: malformed JSON-RPC frame ({e}); line: {trimmed}"
                                );
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::debug!("mcp stdio: child closed stdout (EOF)");
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("mcp stdio: read error: {e}");
                        break;
                    }
                }
            }
            // On exit, fail every still-pending request so callers
            // don't hang forever.
            let mut map = pending_clone.lock().unwrap_or_else(|e| e.into_inner());
            map.clear();
        });

        Ok(Self {
            pending,
            stdin: TokioMutex::new(stdin),
            timeout: Duration::from_secs(30),
            _child: StdMutex::new(Some(child)),
        })
    }

    /// Override the default 30s per-request timeout.
    #[must_use]
    pub fn with_timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn request(&self, method: &str, params: Value) -> Result<Value, McpError> {
        let id = uuid::Uuid::new_v4().to_string();
        let req = JsonRpcRequest {
            jsonrpc: "2.0",
            id: id.clone(),
            method: method.to_string(),
            params,
        };
        let line = serde_json::to_string(&req)?;

        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id.clone(), tx);

        // Write under a tokio Mutex so concurrent callers don't
        // interleave their newlines on the wire.
        {
            let mut guard = self.stdin.lock().await;
            guard
                .write_all(line.as_bytes())
                .await
                .map_err(|e| McpError::Transport(format!("write stdin: {e}")))?;
            guard
                .write_all(b"\n")
                .await
                .map_err(|e| McpError::Transport(format!("write newline: {e}")))?;
            guard
                .flush()
                .await
                .map_err(|e| McpError::Transport(format!("flush stdin: {e}")))?;
        }

        let resp = match timeout(self.timeout, rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => {
                self.pending
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&id);
                return Err(McpError::Closed { request_id: id });
            }
            Err(_) => {
                self.pending
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&id);
                return Err(McpError::Timeout {
                    method: method.to_string(),
                    seconds: self.timeout.as_secs(),
                });
            }
        };

        if let Some(err) = resp.error {
            return Err(McpError::Server {
                code: err.code,
                message: err.message,
                data: err.data,
            });
        }
        Ok(resp.result.unwrap_or(Value::Null))
    }
}

// ── in-memory transport (for tests) ─────────────────────────────────

/// In-memory transport. Construct with a list of `(method, response)`
/// scripted entries; [`request`](McpTransport::request) returns the
/// pre-canned response for the matching method on each call. Unknown
/// methods raise [`McpError::Server`] with code -32601.
///
/// Records every observed call so tests can assert on dispatch.
///
/// `Clone` shares state via `Arc` so tests typically clone once
/// before constructing an `McpClient` and inspect [`Self::observed`]
/// on the clone after the loop runs.
#[derive(Clone)]
pub struct InMemoryTransport {
    /// `method -> (response_value, optional_error)` lookup.
    responses: Arc<StdMutex<HashMap<String, ScriptedResponse>>>,
    observed: Arc<StdMutex<Vec<ObservedRequest>>>,
}

/// One scripted entry. `Ok(value)` returns successfully; `Err(...)` is
/// surfaced as a JSON-RPC `error` envelope mapped to
/// [`McpError::Server`].
#[derive(Clone)]
pub enum ScriptedResponse {
    Ok(Value),
    Error {
        code: i64,
        message: String,
        data: Option<Value>,
    },
}

#[derive(Debug, Clone)]
pub struct ObservedRequest {
    pub method: String,
    pub params: Value,
}

impl InMemoryTransport {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(StdMutex::new(HashMap::new())),
            observed: Arc::new(StdMutex::new(Vec::new())),
        }
    }

    /// Register the response a given method should return. Last write
    /// wins for the same method.
    #[must_use]
    pub fn with_response(self, method: &str, response: ScriptedResponse) -> Self {
        self.responses
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(method.to_string(), response);
        self
    }

    pub fn observed(&self) -> Vec<ObservedRequest> {
        self.observed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

impl Default for InMemoryTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpTransport for InMemoryTransport {
    async fn request(&self, method: &str, params: Value) -> Result<Value, McpError> {
        self.observed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(ObservedRequest {
                method: method.to_string(),
                params: params.clone(),
            });

        let lookup = self
            .responses
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(method)
            .cloned();

        match lookup {
            Some(ScriptedResponse::Ok(v)) => Ok(v),
            Some(ScriptedResponse::Error {
                code,
                message,
                data,
            }) => Err(McpError::Server {
                code,
                message,
                data,
            }),
            None => Err(McpError::Server {
                code: -32601,
                message: format!("method '{method}' not scripted on InMemoryTransport"),
                data: None,
            }),
        }
    }
}
