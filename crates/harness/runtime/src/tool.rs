//! `DesignTool` trait + execution context + result shape.
//!
//! Every executable tool implements [`DesignTool::execute`]. The host
//! supplies a [`ToolCtx`] holding side-channel callbacks (fs / web /
//! image gen / preview / pending-form registration). Tools return one
//! of three variants:
//!
//! - [`ToolResult::Success`] — terminal, content is fed back to the model
//! - [`ToolResult::AwaitUser`] — turn is suspended until the host posts
//!   a response over the chat-rpc channel keyed by `await_id`
//! - [`ToolResult::Failed`] — tool reports an error; harness wraps it
//!   into the model's tool-result payload
//!
//! `AwaitUser` is the new shape `harness-core::Tool` couldn't express.
//! The chat-server side maintains a `HashMap<await_id, oneshot::Sender>`
//! (mirroring the existing `PendingPermissions` map) and resumes the
//! task once the matching response arrives.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::sync::oneshot;

/// Errors a tool can raise. `Other` is the catch-all for tool-specific
/// failures that don't fit the structured shapes; keep tool-specific
/// error strings concise — they get embedded verbatim in the model's
/// tool_result content block.
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

/// What the tool is asking the host to wait for. Tagged so the chat-rpc
/// layer can route the eventual response to the right method
/// (`cli.questionFormResponse` vs `cli.previewResult`, etc).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AwaitKind {
    /// `<question-form>` posted; waiting for `cli.questionFormResponse`.
    QuestionForm,
    /// Preview render requested; waiting for `cli.previewResult`.
    Preview,
    /// Generic permission prompt (already exists, kept here for symmetry).
    Permission,
}

/// Outcome of a single [`DesignTool::execute`] call.
///
/// `Debug` is implemented manually because `oneshot::Receiver<Value>`
/// doesn't itself implement `Debug` — we render the await as
/// `AwaitUser(kind, await_id)`.
pub enum ToolResult {
    /// Synchronous success. `content` is what the harness feeds back to
    /// the model as the tool_result content block.
    Success { content: Value },
    /// Turn suspended — the host should hold the agent task and resume
    /// it with the user's response. `await_id` is the correlation key
    /// used by the chat-rpc response method to look up the receiver.
    AwaitUser {
        await_id: String,
        kind: AwaitKind,
        /// Payload sent to the frontend describing what's being awaited
        /// (form schema for `ask`, artifact path for `preview`).
        payload: Value,
        /// Receiver the harness `await`s on. The host wires the matching
        /// `oneshot::Sender` into its pending-await map and fires it
        /// from the chat-rpc handler.
        receiver: oneshot::Receiver<Value>,
    },
    /// Tool ran to completion but reports failure. The harness wraps
    /// `error` into the tool_result content block with `is_error: true`.
    Failed { error: String },
}

/// Side-channel callbacks the host wires into every tool execution.
///
/// Tools never touch the filesystem, network, or chat transport
/// directly — they go through this context so a test harness can swap
/// in fakes and the production runtime can apply hooks / permissions
/// uniformly.
///
/// All callback fields use `Arc<dyn ...>` so the context is cheap to
/// clone across the suspend/resume boundary.
#[derive(Clone)]
pub struct ToolCtx {
    /// Project working directory — the root tools should treat as the
    /// canonical writable area. Absolute path.
    pub working_dir: PathBuf,
    /// Conversation / session id — included in await_id correlation
    /// keys to scope pending receivers per session.
    pub session_id: String,
    pub fs: Arc<dyn FsCallbacks>,
    pub web: Arc<dyn WebCallbacks>,
    pub image: Arc<dyn ImageCallbacks>,
    /// Frontend bridge — used by `preview` and `ask` to push the
    /// AwaitUser payload to the WS clients. The receiver half of the
    /// oneshot is returned to the tool inside `ToolResult::AwaitUser`.
    pub bridge: Arc<dyn FrontendBridge>,
}

#[async_trait]
pub trait FsCallbacks: Send + Sync {
    async fn read(&self, path: &std::path::Path) -> Result<Vec<u8>, ToolError>;
    async fn write(&self, path: &std::path::Path, body: &[u8]) -> Result<(), ToolError>;
    /// String-replace one occurrence inside a text file. Mirrors the
    /// open-codesign `strReplace` callback that `tweaks` and the
    /// editor depend on.
    async fn str_replace(
        &self,
        path: &std::path::Path,
        old: &str,
        new: &str,
    ) -> Result<(), ToolError>;
}

#[async_trait]
pub trait WebCallbacks: Send + Sync {
    /// Fetch a URL as bytes. Tools should pass a `User-Agent` if the
    /// host implementation lets them; the trait deliberately keeps the
    /// surface tiny so test fakes are trivial.
    async fn fetch(&self, url: &str) -> Result<Vec<u8>, ToolError>;
}

#[async_trait]
pub trait ImageCallbacks: Send + Sync {
    /// Generate an image from a prompt and write it to `out`. Returns
    /// the resolved absolute path on success (host may rewrite to
    /// canonicalize or sandbox).
    async fn generate(&self, prompt: &str, out: &std::path::Path) -> Result<PathBuf, ToolError>;
}

#[async_trait]
pub trait FrontendBridge: Send + Sync {
    /// Register a pending QuestionForm await and return a fresh
    /// correlation id + receiver. The host's chat-rpc layer looks up
    /// `await_id` when `cli.questionFormResponse` fires.
    async fn register_question_form(
        &self,
        payload: &Value,
    ) -> Result<(String, oneshot::Receiver<Value>), ToolError>;

    /// Same shape, but for preview render requests resolved by
    /// `cli.previewResult`.
    async fn register_preview(
        &self,
        payload: &Value,
    ) -> Result<(String, oneshot::Receiver<Value>), ToolError>;
}

#[async_trait]
pub trait DesignTool: Send + Sync {
    /// Stable string id used in registries and permission patterns
    /// (`mcp__kangnam__preview`, `kangnam.brand_asset_extract`, etc).
    fn name(&self) -> &str;

    /// JSON Schema describing `params`. Returned to the model when the
    /// tool is advertised. Stored as opaque JSON to avoid pulling a
    /// schema crate.
    fn parameters(&self) -> Value;

    async fn execute(&self, params: Value, ctx: &ToolCtx) -> ToolResult;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    struct FakeFs;
    #[async_trait]
    impl FsCallbacks for FakeFs {
        async fn read(&self, _: &Path) -> Result<Vec<u8>, ToolError> { Ok(b"hi".to_vec()) }
        async fn write(&self, _: &Path, _: &[u8]) -> Result<(), ToolError> { Ok(()) }
        async fn str_replace(&self, _: &Path, _: &str, _: &str) -> Result<(), ToolError> { Ok(()) }
    }
    struct FakeWeb;
    #[async_trait]
    impl WebCallbacks for FakeWeb {
        async fn fetch(&self, _: &str) -> Result<Vec<u8>, ToolError> { Ok(vec![]) }
    }
    struct FakeImg;
    #[async_trait]
    impl ImageCallbacks for FakeImg {
        async fn generate(&self, _: &str, p: &Path) -> Result<PathBuf, ToolError> { Ok(p.to_path_buf()) }
    }
    struct FakeBridge;
    #[async_trait]
    impl FrontendBridge for FakeBridge {
        async fn register_question_form(&self, _: &Value) -> Result<(String, oneshot::Receiver<Value>), ToolError> {
            let (_tx, rx) = oneshot::channel();
            Ok(("await-1".into(), rx))
        }
        async fn register_preview(&self, _: &Value) -> Result<(String, oneshot::Receiver<Value>), ToolError> {
            let (_tx, rx) = oneshot::channel();
            Ok(("await-2".into(), rx))
        }
    }

    fn ctx() -> ToolCtx {
        ToolCtx {
            working_dir: PathBuf::from("/tmp"),
            session_id: "s1".into(),
            fs: Arc::new(FakeFs),
            web: Arc::new(FakeWeb),
            image: Arc::new(FakeImg),
            bridge: Arc::new(FakeBridge),
        }
    }

    struct EchoTool;
    #[async_trait]
    impl DesignTool for EchoTool {
        fn name(&self) -> &str { "echo" }
        fn parameters(&self) -> Value { serde_json::json!({"type": "object"}) }
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
            other => panic!("expected Success, got {:?}", other),
        }
    }

}

impl std::fmt::Debug for ToolResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolResult::Success { content } => write!(f, "Success({})", content),
            ToolResult::AwaitUser { await_id, kind, .. } => {
                write!(f, "AwaitUser({:?}, {})", kind, await_id)
            }
            ToolResult::Failed { error } => write!(f, "Failed({})", error),
        }
    }
}
