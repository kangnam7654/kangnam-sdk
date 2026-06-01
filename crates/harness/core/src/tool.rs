use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::sync::oneshot;

use crate::Scope;

/// A capability the model can invoke. Either a built-in (Read/Edit/Bash/...),
/// a tool exposed by an MCP server, or a custom tool the consumer registers
/// at runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// JSON Schema describing the tool's input. Stored as opaque JSON to avoid
    /// pulling in a schema crate from `harness-core`.
    #[serde(default)]
    pub parameters: serde_json::Value,
    pub source: ToolSource,
    #[serde(default = "default_scope")]
    pub scope: Scope,
}

fn default_scope() -> Scope {
    Scope::User
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolSource {
    /// Harness-provided tool such as Read, Edit, Bash.
    Builtin(BuiltinTool),
    /// A tool exposed by a connected MCP server.
    Mcp { server: String, tool: String },
    /// A custom tool registered by the consuming application; the harness
    /// only stores metadata, the implementation is provided at runtime.
    Custom { handler: String },
}

/// Enumeration of harness built-ins. Kept open-ended via `Other` so storage
/// roundtrips don't break when a newer version adds tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinTool {
    Read,
    Write,
    Edit,
    Bash,
    Glob,
    Grep,
    WebFetch,
    WebSearch,
    Other(String),
}

/// Errors an executable tool can raise.
///
/// `Other` is the catch-all for tool-specific failures that don't fit the
/// structured shapes; keep tool-specific error strings concise because they
/// are often fed back to the model as tool-result content.
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("invalid arguments: {0}")]
    InvalidArgs(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("{0}")]
    Other(String),
}

/// What the tool is asking the host to wait for.
///
/// Tagged so the host interaction layer can route the eventual response to
/// the right handler, for example question forms, previews, selections, and
/// approvals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AwaitKind {
    QuestionForm,
    Preview,
    Permission,
    Selection,
    Approval,
}

/// Outcome of a single [`AgentTool::execute`] call.
///
/// `Debug` is implemented manually because `oneshot::Receiver<Value>` doesn't
/// itself implement `Debug`.
pub enum ToolResult {
    /// Synchronous success. `content` is what the runner feeds back to the
    /// model as the tool-result content block.
    Success { content: Value },
    /// Turn suspended until the host posts a response through its interaction
    /// channel. `await_id` is the host-owned correlation key.
    AwaitUser {
        await_id: String,
        kind: AwaitKind,
        payload: Value,
        receiver: oneshot::Receiver<Value>,
    },
    /// Tool ran to completion but reports failure. The runner should surface
    /// this as an error tool-result.
    Failed { error: String },
}

/// Side-channel callbacks the host wires into every tool execution.
///
/// `ToolCtx` is generic over a capability bundle `C`. The default bundle is
/// intentionally small and optional where possible so non-design consumers can
/// pass `None` for capabilities they do not use.
#[derive(Clone)]
pub struct ToolCtx<C = DefaultCapabilities> {
    /// Project working directory. `None` for non-workspace tools.
    pub working_dir: Option<PathBuf>,
    /// Conversation/session id used by host correlation layers.
    pub session_id: String,
    /// App-supplied host capabilities.
    pub capabilities: C,
}

/// Default SDK capability bundle.
///
/// Hosts with domain-specific providers should define their own capability
/// struct and implement `AgentTool<TheirCapabilities>`.
#[derive(Clone)]
pub struct DefaultCapabilities {
    pub fs: Arc<dyn FsCallbacks>,
    pub web: Arc<dyn WebCallbacks>,
    pub image: Option<Arc<dyn ImageCallbacks>>,
    pub bridge: Arc<dyn InteractionBridge>,
}

impl<C> ToolCtx<C> {
    /// Construct a `ToolCtx` with no working directory.
    pub fn new(session_id: impl Into<String>, capabilities: C) -> Self {
        Self {
            working_dir: None,
            session_id: session_id.into(),
            capabilities,
        }
    }

    /// Set a working directory.
    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }

    /// Resolve a relative path inside `working_dir`.
    pub fn resolve_path(&self, rel: impl AsRef<std::path::Path>) -> Option<PathBuf> {
        self.working_dir.as_ref().map(|w| w.join(rel))
    }

    /// Return the workspace root, or a structured error if the tool needs one.
    pub fn require_working_dir(&self) -> Result<&PathBuf, ToolError> {
        self.working_dir
            .as_ref()
            .ok_or_else(|| ToolError::InvalidArgs("tool requires a working directory".into()))
    }
}

#[async_trait]
pub trait FsCallbacks: Send + Sync {
    async fn read(&self, path: &std::path::Path) -> Result<Vec<u8>, ToolError>;
    async fn write(&self, path: &std::path::Path, body: &[u8]) -> Result<(), ToolError>;
    async fn str_replace(
        &self,
        path: &std::path::Path,
        old: &str,
        new: &str,
    ) -> Result<(), ToolError>;
}

#[async_trait]
pub trait WebCallbacks: Send + Sync {
    async fn fetch(&self, url: &str) -> Result<Vec<u8>, ToolError>;
}

#[async_trait]
pub trait ImageCallbacks: Send + Sync {
    async fn generate(&self, prompt: &str, out: &std::path::Path) -> Result<PathBuf, ToolError>;
}

/// Suspend/resume bridge between agent tools and the host UI.
#[async_trait]
pub trait InteractionBridge: Send + Sync {
    async fn register_await(
        &self,
        kind: AwaitKind,
        payload: &Value,
    ) -> Result<(String, oneshot::Receiver<Value>), ToolError> {
        match kind {
            AwaitKind::QuestionForm => self.register_question_form(payload).await,
            AwaitKind::Preview => self.register_preview(payload).await,
            other => Err(ToolError::Other(format!(
                "InteractionBridge has no handler for AwaitKind::{:?}",
                other
            ))),
        }
    }

    async fn register_question_form(
        &self,
        payload: &Value,
    ) -> Result<(String, oneshot::Receiver<Value>), ToolError>;

    async fn register_preview(
        &self,
        payload: &Value,
    ) -> Result<(String, oneshot::Receiver<Value>), ToolError>;
}

/// A tool an AI agent can call.
///
/// The contract is domain-neutral: execution semantics and permission
/// decisions stay in the consuming application's `AgentTool` implementation
/// and host capability bundle.
#[async_trait]
pub trait AgentTool<C = DefaultCapabilities>: Send + Sync {
    /// Stable string id used in registries and permission patterns.
    fn name(&self) -> &str;

    /// JSON Schema describing `params`.
    fn parameters(&self) -> Value;

    async fn execute(&self, params: Value, ctx: &ToolCtx<C>) -> ToolResult;
}

impl std::fmt::Debug for ToolResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolResult::Success { content } => write!(f, "Success({content})"),
            ToolResult::AwaitUser { await_id, kind, .. } => {
                write!(f, "AwaitUser({kind:?}, {await_id})")
            }
            ToolResult::Failed { error } => write!(f, "Failed({error})"),
        }
    }
}

#[cfg(test)]
mod execution_tests {
    use super::*;
    use std::path::Path;

    struct FakeFs;
    #[async_trait]
    impl FsCallbacks for FakeFs {
        async fn read(&self, _: &Path) -> Result<Vec<u8>, ToolError> {
            Ok(b"hi".to_vec())
        }

        async fn write(&self, _: &Path, _: &[u8]) -> Result<(), ToolError> {
            Ok(())
        }

        async fn str_replace(&self, _: &Path, _: &str, _: &str) -> Result<(), ToolError> {
            Ok(())
        }
    }

    struct FakeWeb;
    #[async_trait]
    impl WebCallbacks for FakeWeb {
        async fn fetch(&self, _: &str) -> Result<Vec<u8>, ToolError> {
            Ok(vec![])
        }
    }

    struct FakeBridge;
    #[async_trait]
    impl InteractionBridge for FakeBridge {
        async fn register_question_form(
            &self,
            _: &Value,
        ) -> Result<(String, oneshot::Receiver<Value>), ToolError> {
            let (_tx, rx) = oneshot::channel();
            Ok(("await-1".into(), rx))
        }

        async fn register_preview(
            &self,
            _: &Value,
        ) -> Result<(String, oneshot::Receiver<Value>), ToolError> {
            let (_tx, rx) = oneshot::channel();
            Ok(("await-2".into(), rx))
        }
    }

    fn ctx() -> ToolCtx {
        ToolCtx {
            working_dir: Some(PathBuf::from("/tmp")),
            session_id: "s1".into(),
            capabilities: DefaultCapabilities {
                fs: Arc::new(FakeFs),
                web: Arc::new(FakeWeb),
                image: None,
                bridge: Arc::new(FakeBridge),
            },
        }
    }

    struct EchoTool;
    #[async_trait]
    impl AgentTool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }

        fn parameters(&self) -> Value {
            serde_json::json!({"type": "object"})
        }

        async fn execute(&self, params: Value, _: &ToolCtx) -> ToolResult {
            ToolResult::Success { content: params }
        }
    }

    #[tokio::test]
    async fn echo_tool_round_trips_params() {
        let tool = EchoTool;
        let ctx = ctx();
        match tool.execute(serde_json::json!({"x": 1}), &ctx).await {
            ToolResult::Success { content } => assert_eq!(content["x"], 1),
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn register_await_dispatches_named_kinds() {
        let bridge = FakeBridge;
        let payload = serde_json::json!({});
        let (id, _rx) = bridge
            .register_await(AwaitKind::QuestionForm, &payload)
            .await
            .unwrap();
        assert_eq!(id, "await-1");
        let (id, _rx) = bridge
            .register_await(AwaitKind::Preview, &payload)
            .await
            .unwrap();
        assert_eq!(id, "await-2");
    }
}
