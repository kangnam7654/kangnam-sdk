//! `AgentTool` trait + execution context + result shape.
//!
//! Domain-agnostic runtime for tools an AI agent can call. The host
//! supplies a [`ToolCtx`] holding session metadata + side-channel
//! capabilities (filesystem, web, image generation, user
//! interaction, or any host-defined pluggable capability bundle).
//! Tools return one of three variants:
//!
//! - [`ToolResult::Success`] — terminal, content is fed back to the
//!   model as the tool_result content block.
//! - [`ToolResult::AwaitUser`] — turn is suspended until the host
//!   posts a response through its interaction channel keyed by
//!   `await_id`.
//!   Use cases: `<question-form>` posts, artifact previews, user
//!   selections from a candidate list, approval prompts, etc.
//! - [`ToolResult::Failed`] — tool ran to completion but reports
//!   failure; harness wraps `error` into the model's tool-result
//!   content block with `is_error: true`.
//!
//! `AwaitUser` is the new shape `harness-core::Tool` couldn't
//! express. Hosts maintain a `HashMap<await_id, oneshot::Sender>` or
//! equivalent pending-response registry and resume the task once the
//! matching response arrives.
//!
//! # Designed for multiple consumers
//!
//! This crate originated to back the `kangnam-sdk` design family
//! (Phase 4) but is intentionally not design-specific. Other
//! consumers — Travel Planner, research/automation agents, finance
//! advisors — provide their own capability bundles and tool
//! implementations on top of the same trait surface.

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

/// What the tool is asking the host to wait for. Tagged so the host
/// interaction layer can route the eventual response to the right
/// handler (for example question forms, previews, selections, and
/// approvals).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AwaitKind {
    /// `<question-form>` posted; waiting for a form response.
    QuestionForm,
    /// Preview render requested; waiting for a preview response.
    Preview,
    /// Generic permission prompt (already exists, kept here for symmetry).
    Permission,
    /// User picks one or more items from a candidate list. Generic
    /// — Travel Planner uses this for accommodation / spot picking.
    Selection,
    /// User approves or rejects a proposed change. Use for
    /// itinerary patches, money transfers, risky writes, etc.
    Approval,
}

/// Outcome of a single [`AgentTool::execute`] call.
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
    /// used by the host response method to look up the receiver.
    AwaitUser {
        await_id: String,
        kind: AwaitKind,
        /// Payload sent to the frontend describing what's being awaited
        /// (form schema for QuestionForm, artifact path for Preview,
        /// candidate list for Selection, etc).
        payload: Value,
        /// Receiver the harness `await`s on. The host wires the matching
        /// `oneshot::Sender` into its pending-await map and fires it
        /// from the interaction response handler.
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
/// # Capability bundle
///
/// `ToolCtx` is generic over a capability bundle `C`. The default,
/// [`DefaultCapabilities`], carries the four capabilities the design
/// family uses (filesystem, web fetch, image generation, interaction
/// bridge). Other consumers — Travel Planner, research agents,
/// finance advisors — define their own capability struct with their
/// domain providers (map / accommodation / money advice / …) and
/// instantiate `ToolCtx<TheirCapabilities>`.
///
/// ```ignore
/// // Travel Planner
/// pub struct TravelCapabilities {
///     pub web: Arc<dyn WebSearch>,
///     pub map: Arc<dyn MapProvider>,
///     pub accommodation: Arc<dyn AccommodationProvider>,
///     pub interaction: Arc<dyn InteractionBridge>,
/// }
///
/// impl AgentTool<TravelCapabilities> for AccommodationSearchTool {
///     async fn execute(&self, params: Value, ctx: &ToolCtx<TravelCapabilities>)
///         -> ToolResult { /* ctx.capabilities.accommodation.search(...) */ }
/// }
/// ```
///
/// `working_dir` is `Option<PathBuf>` — capabilities that have no
/// canonical workspace (Travel itinerary research, money advice)
/// pass `None`.
#[derive(Clone)]
pub struct ToolCtx<C = DefaultCapabilities> {
    /// Project working directory — the root tools should treat as the
    /// canonical writable area when one applies. `None` for
    /// non-workspace tools (e.g. pure web search, map lookup).
    pub working_dir: Option<PathBuf>,
    /// Conversation / session id — included in await_id correlation
    /// keys to scope pending receivers per session.
    pub session_id: String,
    /// App-supplied bundle of host capabilities. The design family
    /// uses [`DefaultCapabilities`]; other domains define their own
    /// struct.
    pub capabilities: C,
}

/// The default capability bundle — design family + Travel Planner
/// share the same shape because both need fs / web / interaction.
///
/// `image` is design-specific but kept here for now to avoid breaking
/// design-tools; future split:
/// - `CoreCapabilities` (fs, web, bridge) — minimum any consumer needs
/// - `DesignCapabilities` (CoreCapabilities + image)
/// - `TravelCapabilities` (CoreCapabilities + map + accommodation + …)
///
/// This split is deferred to a follow-up commit because the only
/// current `image` consumer is `gen_image` and design-tools imports
/// would churn. For now the field is `Option<Arc<dyn ImageCallbacks>>`
/// so non-design consumers can pass `None`.
#[derive(Clone)]
pub struct DefaultCapabilities {
    pub fs: Arc<dyn FsCallbacks>,
    pub web: Arc<dyn WebCallbacks>,
    pub image: Option<Arc<dyn ImageCallbacks>>,
    /// Interaction bridge — used by tools that suspend the agent
    /// turn awaiting a user response (forms, previews, selections,
    /// approvals).
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

    /// Convenience: resolve a relative path inside `working_dir`.
    /// Returns `None` if the context has no workspace — tools that
    /// require one should check first and return a structured
    /// `ToolError::InvalidArgs` so the agent gets a clear message.
    pub fn resolve_path(&self, rel: impl AsRef<std::path::Path>) -> Option<PathBuf> {
        self.working_dir.as_ref().map(|w| w.join(rel))
    }

    /// Convenience: workspace root, or an InvalidArgs error if the
    /// caller's tool needs one.
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

/// Suspend/resume bridge between agent tools and the host UI.
///
/// Implementors mint a fresh correlation id, push the payload to the
/// host (for example over JSON-RPC, Tauri events, or an in-memory
/// channel), and return the
/// receiver half of a oneshot. The agent task `await`s the receiver;
/// the host's response handler fires the matching sender once the
/// user has responded.
///
/// The named methods (`register_question_form`, `register_preview`)
/// are convenience hooks for common interaction kinds. Hosts that need
/// additional interaction kinds can implement `register_await`
/// directly.
#[async_trait]
pub trait InteractionBridge: Send + Sync {
    /// Register an arbitrary await of the supplied `kind` with the
    /// host. Default impl dispatches to `register_question_form` /
    /// `register_preview` for the two named cases; everything else
    /// (Selection / Approval / Permission) requires the host to
    /// override this method.
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

    /// Register a pending QuestionForm await and return a fresh
    /// correlation id + receiver. The host interaction layer looks up
    /// `await_id` when the form response arrives.
    async fn register_question_form(
        &self,
        payload: &Value,
    ) -> Result<(String, oneshot::Receiver<Value>), ToolError>;

    /// Same shape, but for preview render requests.
    async fn register_preview(
        &self,
        payload: &Value,
    ) -> Result<(String, oneshot::Receiver<Value>), ToolError>;
}

/// A tool an AI agent can call.
///
/// Implementors wire their own host capabilities into `execute` via
/// the [`ToolCtx`] callbacks. Test fakes swap the callbacks for
/// recorded mock implementations; production hosts wire fs / web /
/// image / interaction bridges to real services.
///
/// Generic over a capability bundle `C`; the default,
/// [`DefaultCapabilities`], works for the design family. Domains
/// with their own provider types (Travel Planner's
/// `TravelCapabilities`, research agents, …) implement
/// `AgentTool<TheirCapabilities>` so the runtime can host both
/// kinds of tools simultaneously.
#[async_trait]
pub trait AgentTool<C = DefaultCapabilities>: Send + Sync {
    /// Stable string id used in registries and permission patterns
    /// (`mcp__kangnam__preview`, `kangnam.brand_asset_extract`,
    /// `travel.accommodation_search`, etc).
    fn name(&self) -> &str;

    /// JSON Schema describing `params`. Returned to the model when the
    /// tool is advertised. Stored as opaque JSON to avoid pulling a
    /// schema crate.
    fn parameters(&self) -> Value;

    async fn execute(&self, params: Value, ctx: &ToolCtx<C>) -> ToolResult;
}

#[cfg(test)]
mod tests {
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
    struct FakeImg;
    #[async_trait]
    impl ImageCallbacks for FakeImg {
        async fn generate(&self, _: &str, p: &Path) -> Result<PathBuf, ToolError> {
            Ok(p.to_path_buf())
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
                image: Some(Arc::new(FakeImg)),
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
            other => panic!("expected Success, got {:?}", other),
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

    #[tokio::test]
    async fn register_await_default_rejects_unknown_kinds() {
        let bridge = FakeBridge;
        let payload = serde_json::json!({});
        let err = bridge
            .register_await(AwaitKind::Selection, &payload)
            .await
            .unwrap_err();
        match err {
            ToolError::Other(msg) => assert!(msg.contains("Selection")),
            other => panic!("expected Other, got {:?}", other),
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
