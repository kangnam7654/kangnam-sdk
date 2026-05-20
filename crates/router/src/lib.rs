#![deny(unsafe_code)]
//! Multi-provider LLM client.
//!
//! This crate exposes a [`Router`] facade for routing requests across multiple
//! configured providers, plus the lower-level object-safe [`LlmProviderDyn`]
//! trait that every backend implements. Callers can either register provider
//! configs in [`Router`] and drive all calls through [`RouteRequest`], or
//! construct one provider directly through [`create_provider`] and call
//! [`LlmProviderDyn::render_dyn`], [`LlmProviderDyn::chat_dyn`], or
//! [`LlmProviderDyn::chat_stream_dyn`] (plus their `_with_options_dyn`
//! counterparts that accept [`LlmRequestOptions`]).
//!
//! # Built-in providers
//!
//! The [`REGISTRY`](self) ships with HTTP backends (`claude`, `codex`,
//! `copilot`, `gemini`, `openai_compat`), local-CLI backends
//! (`claude_local`, `codex_local`, `copilot_local`, `gemini_local`,
//! `pi_local`), a `dummy` offline provider for tests, and an
//! `antigravity` alias for `gemini`. Unknown keys fall back to `dummy`.
//! See [`registered_providers`] for the full list.
//!
//! # Streaming
//!
//! [`chat_stream_dyn`](LlmProviderDyn::chat_stream_dyn) returns a
//! [`BoxStream`] of [`LlmStreamEvent`]. Local-CLI providers emit
//! incremental [`LlmStreamEvent::Delta`] events as tokens arrive and a
//! terminal [`LlmStreamEvent::End`] carrying the full accumulated
//! [`LlmResponse`]. HTTP providers emit a single Delta + End by default.
//!
//! # Example
//!
//! ```no_run
//! use kangnam_router::{ChatMessage, ProviderConfig, RouteRequest, Router};
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let router = Router::new().with_provider(
//!     "default",
//!     ProviderConfig::new("dummy", "", "", ""),
//! );
//! let request = RouteRequest::chat(vec![ChatMessage::user("hello")]);
//! let resp = router.chat(request).await?;
//! println!("{}", resp.rendered_text);
//! # Ok(()) }
//! ```

pub mod error;
pub use error::LlmError;

pub mod claude;
pub mod claude_local;
pub mod cli_utils;
pub mod codex;
pub mod codex_local;
pub mod context;
pub mod copilot;
pub mod copilot_local;
pub mod dummy;
pub mod gemini;
pub mod gemini_local;
pub mod openai_compat;
pub mod pi_local;
pub mod pricing;
pub mod router;

pub use router::{
    ProviderConfig, ProviderIntegrationCandidate, ProviderIntegrationStatus, RenderRequest,
    RouteRequest, Router, RouterAttachPath, SubscriptionAuthSurface, SubscriptionProviderBenchmark,
    provider_integration_candidates, subscription_provider_benchmarks,
};

/// The kind of provider backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    /// Native HTTP API (e.g. Anthropic, OpenAI, Gemini).
    Http,
    /// OpenAI-compatible HTTP API.
    OpenAiCompatible,
    /// Local CLI tool (e.g. `claude`, `gh copilot`).
    LocalCli,
    /// Test dummy / unknown fallback.
    Dummy,
}

/// How the provider reports token usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageSupport {
    /// Usage is never reported.
    None,
    /// Usage is only reported on the final response object.
    FinalOnly,
    /// Usage is streamed in chunks and reported on the final response object.
    Streaming,
}

/// Comprehensive capability registry for an LLM provider.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderCapabilities {
    pub kind: ProviderKind,
    pub usage_support: UsageSupport,
    pub supports_streaming: bool,
    pub supports_tool_calling: bool,
    pub supports_parallel_tool_calls: bool,
    pub supports_image_input: bool,
    pub supports_image_url: bool,
    pub supports_reasoning_effort: bool,
    pub supports_thinking_budget: bool,
    pub supports_prompt_cache: bool,
    pub supports_model_listing: bool,
    pub supports_web_search: bool,
    pub supports_local_read: bool,
    pub estimates_cost: bool,
}

impl ProviderCapabilities {
    /// Safe defaults for an unknown provider.
    pub fn dummy() -> Self {
        Self {
            kind: ProviderKind::Dummy,
            usage_support: UsageSupport::None,
            supports_streaming: false,
            supports_tool_calling: false,
            supports_parallel_tool_calls: false,
            supports_image_input: false,
            supports_image_url: false,
            supports_reasoning_effort: false,
            supports_thinking_budget: false,
            supports_prompt_cache: false,
            supports_model_listing: false,
            supports_web_search: false,
            supports_local_read: false,
            estimates_cost: false,
        }
    }
}

/// Consolidated usage and cost information.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct LlmUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_creation_input_tokens: u32,
    pub cache_read_input_tokens: u32,
    pub estimated_cost_usd: f64,
}

impl LlmUsage {
    /// Adds another usage struct to this one.
    pub fn add(&mut self, other: &Self) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.cache_creation_input_tokens = self
            .cache_creation_input_tokens
            .saturating_add(other.cache_creation_input_tokens);
        self.cache_read_input_tokens = self
            .cache_read_input_tokens
            .saturating_add(other.cache_read_input_tokens);
        self.estimated_cost_usd += other.estimated_cost_usd;
    }
}

use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;

/// One turn of a conversation.
///
/// `role` is `"user"` or `"assistant"`. Providers that don't support
/// multi-turn history fall back to the last `"user"` message.
///
/// Construct messages via the provided constructors (`::user()`,
/// `::assistant()`, `::user_with_image()`, `::user_multipart()`,
/// `::tool_result()`) rather than struct literals. The `#[non_exhaustive]`
/// attribute prevents external crates from constructing struct literals
/// directly, ensuring forward compatibility as new fields are added.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    pub content: Vec<ChatContent>,
}

impl ChatMessage {
    /// User turn with plain text. Equivalent to v0.3.x
    /// `ChatMessage { role: "user".into(), content: text }`.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![ChatContent::Text(text.into())],
        }
    }

    /// Assistant turn with plain text.
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: vec![ChatContent::Text(text.into())],
        }
    }

    /// User turn with text plus one image block.
    pub fn user_with_image(text: impl Into<String>, image: ChatContent) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![ChatContent::Text(text.into()), image],
        }
    }

    /// User turn with arbitrary multipart blocks (text + images, multiple, etc.).
    pub fn user_multipart(blocks: Vec<ChatContent>) -> Self {
        Self {
            role: "user".to_string(),
            content: blocks,
        }
    }

    /// Tool result feedback (from caller back to model).
    /// `role` is set to `"user"` — Anthropic and OpenAI both expect tool_result
    /// blocks to ride inside a user message.
    pub fn tool_result(
        tool_use_id: impl Into<String>,
        content: impl Into<String>,
        is_error: bool,
    ) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![ChatContent::ToolResult {
                tool_use_id: tool_use_id.into(),
                content: content.into(),
                is_error,
            }],
        }
    }

    /// Assistant turn that emitted one or more tool calls.
    ///
    /// `text` is the optional pre-tool-call narration ("Checking the
    /// weather…"). `calls` are the tool invocations the model emitted.
    /// Used by multi-turn tool-calling loops to record the assistant's
    /// previous turn so the next request body correctly pairs each
    /// subsequent `tool_result` with its originating `tool_call_id`.
    ///
    /// Each [`ToolCall`] becomes a [`ChatContent::ToolUse`] block. The
    /// `text`, when non-empty, is prepended as a [`ChatContent::Text`]
    /// block.
    pub fn assistant_with_tool_calls(text: impl Into<String>, calls: Vec<ToolCall>) -> Self {
        let text = text.into();
        let mut content = Vec::with_capacity(calls.len() + 1);
        if !text.is_empty() {
            content.push(ChatContent::Text(text));
        }
        for c in calls {
            content.push(ChatContent::ToolUse {
                id: c.id,
                name: c.name,
                arguments: c.arguments,
            });
        }
        Self {
            role: "assistant".to_string(),
            content,
        }
    }

    /// Concatenate all `Text` blocks into a single String. Skips
    /// `Image`, `ToolResult`, and `ToolUse` blocks. Useful for legacy
    /// text-only paths (e.g. local CLI prompt construction) and for the
    /// trait default impl of `chat_dyn`.
    pub fn text_content(&self) -> String {
        self.content
            .iter()
            .filter_map(|c| match c {
                ChatContent::Text(s) => Some(s.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

/// Options that modify how a provider executes a request.
///
/// Different providers honor different subsets of these options. Unsupported
/// options are silently ignored. The default value is "no options" which
/// makes existing `chat_dyn` behavior the floor.
#[derive(Debug, Clone, Default)]
pub struct LlmRequestOptions {
    /// Image file paths to attach to the request.
    /// - claude_local: appended as `--image <path>` (verify CLI support; falls back to ignore)
    /// - codex_local: appended as `--image <path>`
    /// - gemini_local: appended to prompt as `@<abs_path>` to trigger read_many_files
    pub image_paths: Vec<PathBuf>,

    /// Working directory for the provider subprocess.
    /// - claude_local: `current_dir(...)` + `--add-dir <path>`
    /// - codex_local: `-C <path>`
    /// - gemini_local: `current_dir(...)` + `--include-directories <path>`
    pub working_dir: Option<PathBuf>,

    /// Allow the model to perform web searches.
    /// - claude_local: adds `WebSearch,WebFetch` to `--allowedTools`
    /// - codex_local: passes `--search`
    /// - gemini_local: ignored (no flag)
    pub allow_web_search: bool,

    /// Allow the model to read local files.
    /// - claude_local: adds `Read` to `--allowedTools`
    /// - codex_local: implicit in sandbox
    /// - gemini_local: implicit via `@path`
    pub allow_local_read: bool,

    /// Maximum agent turns.
    /// - claude_local: `--max-turns <n>`
    /// - other providers: ignored
    pub max_turns: Option<u32>,

    /// Reasoning effort. Source valid values from the provider model catalog
    /// when possible; Codex reasoning models may expose values such as
    /// `"low"`, `"medium"`, `"high"`, and `"xhigh"`.
    /// - codex_local: passes `-c model_reasoning_effort="<value>"`
    /// - other providers: ignored
    pub reasoning_effort: Option<String>,

    /// Maximum total time for this call. None = use the provider's built-in
    /// default. On elapse:
    /// - Blocking calls return `LlmError::Network { msg: "timeout" }`.
    /// - Streams emit `LlmStreamEvent::Error { message: "timeout" }` and end.
    ///
    /// Local CLI providers (`claude_local`, `codex_local`, `gemini_local`)
    /// rely on `kill_on_drop(true)` to reap the subprocess on stream drop.
    /// Honored only on `chat_with_options_dyn` / `chat_stream_with_options_dyn`;
    /// the no-options paths (`chat_dyn` / `chat_stream_dyn`) keep the v0.3.0
    /// hardcoded behavior.
    pub timeout: Option<std::time::Duration>,

    /// Sampling temperature. Higher = more random. Typical range 0.0–2.0.
    /// `None` = provider's built-in default. Honored by HTTP providers
    /// (`claude`, `codex`, `copilot`, `gemini`, `openai_compat`);
    /// local CLI providers (`claude_local`, `codex_local`, `gemini_local`)
    /// silently ignore this field.
    pub temperature: Option<f32>,

    /// Maximum tokens to generate in the response. `None` = provider's
    /// built-in default. Honored by HTTP providers; local CLI silently ignored.
    ///
    /// Note: `claude` requires `max_tokens` at the protocol level. When this
    /// is `None`, it falls back to the existing `MAX_TOKENS = 1024` constant.
    pub max_tokens: Option<u32>,

    /// Nucleus sampling top-p. `None` = provider default. Honored by HTTP
    /// providers; local CLI silently ignored.
    pub top_p: Option<f32>,

    /// Stop sequences. Empty vec = no stops. Honored by HTTP providers;
    /// local CLI silently ignored. The provider may impose its own limit on
    /// the number of stop sequences (e.g. Anthropic max 4).
    pub stop_sequences: Vec<String>,

    /// Tool / function definitions the model may call. Empty = no tools.
    /// Honored by HTTP providers (`claude`, `codex`, `gemini`, `openai_compat`;
    /// `copilot` is experimental). Local CLI providers (`claude_local`,
    /// `codex_local`, `gemini_local`) ignore this field — local CLIs handle
    /// their own internal tool sets (Bash, Read, Edit, etc.) and don't
    /// expose user-defined tool calling via this API.
    pub tools: Vec<ToolDef>,

    /// Strategy for forcing tool use. `None` = provider default (Auto).
    /// Honored on the same providers as `tools`.
    pub tool_choice: Option<ToolChoice>,

    /// Anthropic extended thinking budget (input tokens reserved for
    /// reasoning). `None` = thinking off. Honored by `claude` HTTP and
    /// `claude_local` CLI providers; ignored elsewhere.
    ///
    /// When `Some(n)`, the `claude` HTTP provider also auto-adds the
    /// `anthropic-beta: interleaved-thinking-2025-05-14` header.
    pub thinking_budget_tokens: Option<u32>,

    /// Anthropic prompt caching breakpoints. Empty = no caching.
    /// Honored by `claude` HTTP only. Anthropic limit: max 4 — excess
    /// truncated with `tracing::warn!`. Out-of-range `MessageIndex`
    /// silently skipped with `tracing::warn!`.
    pub cache_breakpoints: Vec<CacheBreakpoint>,
}

/// Tool / function definition the model can call.
///
/// `input_schema` must be a valid JSON Schema (Draft 2020-12) object.
/// The schema is passed through verbatim to the provider — no validation
/// is performed. Callers are responsible for correctness.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolDef {
    /// Machine-readable name (e.g. `"get_weather"`). Must be unique within
    /// a single request's tool list.
    pub name: String,
    /// Human-readable description the model uses to decide when to invoke
    /// the tool.
    pub description: String,
    /// JSON Schema (Draft 2020-12) describing the tool's input parameters.
    pub input_schema: serde_json::Value,
}

/// A tool invocation produced by the model.
///
/// Paired with the corresponding [`ChatContent::ToolResult`] in the next
/// user turn via `id`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    /// Provider-assigned call id, used to pair this call with a
    /// [`ChatContent::ToolResult`] in the subsequent user message.
    pub id: String,
    /// Name of the tool the model chose to call.
    pub name: String,
    /// Parsed JSON arguments matching the tool's `input_schema`.
    pub arguments: serde_json::Value,
}

/// Strategy for forcing tool use.
///
/// Providers translate this enum to their own wire-format representations.
/// `ToolChoice` is intentionally not serialized — each provider has a
/// different JSON shape.
///
/// `#[non_exhaustive]` so future variants (e.g. parallel-call modes) can be
/// added without a breaking change.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum ToolChoice {
    /// Model decides whether to call a tool (provider default).
    Auto,
    /// Model must call at least one tool.
    Required,
    /// Disable tool calling for this request.
    None,
    /// Force a specific tool by name.
    Specific(String),
}

/// Cache marker positions for Anthropic prompt caching.
/// Each variant targets one location in the request that should be
/// cached up to. Anthropic billing: 90% discount on cached read,
/// 5-min TTL, max 4 cache breakpoints per request (excess silently
/// truncated with `tracing::warn!`).
///
/// Honored by the `claude` HTTP provider only; silently ignored
/// by other providers.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum CacheBreakpoint {
    /// Cache the system prompt (and any tool definitions Anthropic
    /// groups with it). Most common case.
    System,
    /// Cache `messages[0..=index]` (inclusive). Useful for caching
    /// long RAG context blocks placed early in the conversation.
    /// Out-of-range indices are silently skipped with `tracing::warn!`.
    ///
    /// Note: if the targeted message's last content block is a `Thinking`
    /// block (Anthropic doesn't support `cache_control` on thinking blocks),
    /// the breakpoint is skipped with `tracing::warn!`.
    MessageIndex(usize),
}

/// Source of an image attachment.
///
/// `#[non_exhaustive]` — future variants (e.g. `FilePath`) can be added
/// without breaking existing `match` arms in external crates.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum ImageSource {
    /// Pre-encoded base64 string. The caller is responsible for encoding
    /// raw bytes before constructing this variant.
    Base64(String),
    /// Public URL the provider fetches on the server side.
    Url(String),
}

/// One block of a multipart message.
///
/// `#[non_exhaustive]` — future block types can be added without breaking
/// existing `match` arms in external crates.
///
/// `From<&str>` and `From<String>` impls produce [`Text`](Self::Text)
/// variants for ergonomic construction:
///
/// ```rust
/// # use kangnam_router::ChatContent;
/// let c: ChatContent = "hello".into();
/// assert_eq!(c, ChatContent::Text("hello".into()));
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum ChatContent {
    /// Plain text block.
    Text(String),
    /// Image block. `mime_type` should be a valid MIME type such as
    /// `"image/png"` or `"image/jpeg"`.
    Image {
        /// Source of the image data.
        source: ImageSource,
        /// MIME type, e.g. `"image/png"`, `"image/jpeg"`.
        mime_type: String,
    },
    /// Tool result block — caller's response to a model-initiated
    /// [`ToolCall`]. Role must be `"user"` (both Anthropic and OpenAI
    /// require tool results inside a user message).
    ToolResult {
        /// Id matching the [`ToolCall::id`] this result answers.
        tool_use_id: String,
        /// Serialized result payload, typically JSON or plain text.
        content: String,
        /// `true` if the tool execution produced an error.
        is_error: bool,
    },
    /// Tool invocation block — assistant turn that records a tool call
    /// the model previously emitted. Used by multi-turn tool-calling
    /// loops to replay history so the provider can correlate the
    /// subsequent [`ToolResult`] with the originating call.
    ///
    /// Wire format per provider:
    /// - OpenAI / OpenAI-compat / Copilot: folded into the assistant
    ///   message's `tool_calls` array.
    /// - Anthropic: emitted as a `tool_use` content block.
    /// - Gemini: emitted as a `functionCall` part on the model turn.
    /// - Local CLI providers: skipped (CLI handles its own internal
    ///   tools).
    ///
    /// Construct via [`ChatMessage::assistant_with_tool_calls`] rather
    /// than struct literal.
    ToolUse {
        /// Provider-assigned call id. Must match the
        /// [`ChatContent::ToolResult::tool_use_id`] in the next user
        /// turn so the model can pair them.
        id: String,
        /// Tool name the model chose to invoke.
        name: String,
        /// Parsed JSON arguments the model produced for this call.
        arguments: serde_json::Value,
    },
}

impl From<&str> for ChatContent {
    fn from(s: &str) -> Self {
        ChatContent::Text(s.to_string())
    }
}

impl From<String> for ChatContent {
    fn from(s: String) -> Self {
        ChatContent::Text(s)
    }
}

impl ChatContent {
    /// Read an image file, base64-encode it, and infer the MIME type from the
    /// file extension.
    ///
    /// Supported extensions and their MIME types:
    /// - `png`  → `"image/png"`
    /// - `jpg` / `jpeg` → `"image/jpeg"`
    /// - `gif`  → `"image/gif"`
    /// - `webp` → `"image/webp"`
    /// - `bmp`  → `"image/bmp"`
    /// - anything else → `"application/octet-stream"`
    ///
    /// # Errors
    /// Returns `std::io::Error` if the file cannot be read.
    ///
    /// # Example
    /// ```no_run
    /// # use kangnam_router::ChatContent;
    /// let content = ChatContent::image_from_path("photo.png")?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn image_from_path(path: impl AsRef<std::path::Path>) -> Result<Self, std::io::Error> {
        use base64::Engine as _;
        let path = path.as_ref();
        let bytes = std::fs::read(path)?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let mime_type = match path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref()
        {
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("gif") => "image/gif",
            Some("webp") => "image/webp",
            Some("bmp") => "image/bmp",
            _ => "application/octet-stream",
        };
        Ok(ChatContent::Image {
            source: ImageSource::Base64(encoded),
            mime_type: mime_type.to_string(),
        })
    }

    /// Detect the MIME type of an image from its base64-encoded bytes using
    /// magic-byte sniffing via the [`infer`] crate.
    ///
    /// Only the first 32 decoded bytes are needed for detection; the full
    /// base64 payload is forwarded as-is so no extra allocation is required
    /// beyond the detection probe.
    ///
    /// # Errors
    /// Returns `Err("invalid base64")` if `data` is not valid standard base64.
    /// Returns `Err("could not detect image MIME from base64 data")` if the
    /// magic bytes do not correspond to a known image format.
    ///
    /// # Example
    /// ```no_run
    /// # use kangnam_router::ChatContent;
    /// # use base64::Engine as _;
    /// let png: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    /// let b64 = base64::engine::general_purpose::STANDARD.encode(png);
    /// let content = ChatContent::image_from_base64_auto(b64)?;
    /// # Ok::<(), &'static str>(())
    /// ```
    pub fn image_from_base64_auto(data: impl Into<String>) -> Result<Self, &'static str> {
        use base64::Engine as _;
        let data = data.into();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&data)
            .map_err(|_| "invalid base64")?;
        let head = &decoded[..decoded.len().min(32)];
        let mime_type = infer::get(head)
            .ok_or("could not detect image MIME from base64 data")?
            .mime_type()
            .to_string();
        Ok(ChatContent::Image {
            source: ImageSource::Base64(data),
            mime_type,
        })
    }
}

/// Result of a completed LLM request.
///
/// `rendered_text` holds the full assistant output. `estimated_cost_usd`
/// is provider-specific; providers without a known pricing model return
/// `0.0`. Token counts are `None` when the provider does not expose them.
/// `tool_calls` is empty when the model produced only text.
///
/// `#[non_exhaustive]` prevents external crates from constructing struct
/// literals directly — use `..Default::default()` for forward compatibility.
#[non_exhaustive]
#[derive(Debug, Default)]
pub struct LlmResponse {
    /// Full assistant output text.
    pub rendered_text: String,
    /// Concrete model name as reported by the provider.
    pub model: String,
    /// Estimated USD cost, or `0.0` when unknown.
    pub estimated_cost_usd: f64,
    /// Input (prompt) tokens consumed. None if the provider does not report it.
    pub input_tokens: Option<u32>,
    /// Output (completion) tokens generated. None if the provider does not report it.
    pub output_tokens: Option<u32>,
    /// Tool calls produced by the model on this response. Empty if the model
    /// produced only text. For streaming, these are accumulated and exposed
    /// on the terminal [`LlmStreamEvent::End`] event.
    pub tool_calls: Vec<ToolCall>,
    /// Accumulated reasoning/thinking text (separate from `rendered_text`).
    /// `None` if the model produced no thinking content. Populated by
    /// `claude` / `claude_local` (extended thinking blocks),
    /// `codex` / `openai_compat` (o-series reasoning_content),
    /// `gemini` (3.x thinking parts).
    pub thinking_text: Option<String>,
    /// Anthropic only: tokens written to cache this turn (uncached
    /// input for cache breakpoints that produced a new cache entry).
    pub cache_creation_input_tokens: Option<u32>,
    /// Anthropic only: tokens read from cache this turn — billed at
    /// 10% of the standard input rate.
    pub cache_read_input_tokens: Option<u32>,
}

impl LlmResponse {
    /// Returns the consolidated usage and cost for this response.
    pub fn usage(&self) -> LlmUsage {
        LlmUsage {
            input_tokens: self.input_tokens.unwrap_or(0),
            output_tokens: self.output_tokens.unwrap_or(0),
            cache_creation_input_tokens: self.cache_creation_input_tokens.unwrap_or(0),
            cache_read_input_tokens: self.cache_read_input_tokens.unwrap_or(0),
            estimated_cost_usd: self.estimated_cost_usd,
        }
    }
}

/// Event emitted by [`LlmProviderDyn::chat_stream_dyn`] and
/// [`LlmProviderDyn::chat_stream_with_options_dyn`].
///
/// A successful stream yields zero or more [`Delta`](Self::Delta) events,
/// zero or more [`Thinking`](Self::Thinking) events, zero or more
/// [`Usage`](Self::Usage) snapshots, and zero or more
/// [`ToolCall`](Self::ToolCall) events, followed by exactly one terminal
/// [`End`](Self::End). A failing stream yields an [`Error`](Self::Error)
/// and ends. Consumers that only read `End` get the full accumulated text
/// in `End.total.rendered_text`.
///
/// `#[non_exhaustive]` — future variants can be added without breaking
/// existing `match` arms in external crates. All `match` blocks must
/// include a `_ => { }` wildcard arm.
#[non_exhaustive]
pub enum LlmStreamEvent {
    /// An incremental chunk of assistant output.
    Delta { text: String },
    /// Terminal event with the full accumulated response.
    End { total: LlmResponse },
    /// Terminal error event. No further events follow.
    Error { message: String },
    /// Model produced a tool call. Emitted after the corresponding
    /// `tool_use` block fully accumulates server-side; partial-JSON
    /// streaming is not exposed in v0.4.0. Multiple `ToolCall` events
    /// per stream are possible (parallel tool use).
    ToolCall { call: ToolCall },
    /// Incremental reasoning/thinking text. Emitted as the model
    /// produces internal reasoning before/between answer text.
    /// Accumulated total appears in `End { total: { thinking_text } }`.
    Thinking { text: String },
    /// Periodic usage snapshot. Emit cadence varies per provider:
    /// - Anthropic: per `message_delta` (real progressive)
    /// - Gemini: per SSE chunk (most granular)
    /// - OpenAI / OpenAI-compat / Codex: only the final chunk before
    ///   `End` (effectively redundant with `End`, but fires earlier
    ///   so caller can update UI before terminal event)
    /// - Local CLI: only on `result` message before `End`
    ///
    /// Throttle: providers SHOULD throttle to ≥50 output token delta
    /// to avoid stream noise. Caller-side throttling not required.
    Usage {
        input_tokens: Option<u32>,
        output_tokens: u32,
        estimated_cost_usd: f64,
    },
}

/// A model available from an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListModel {
    pub name: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_token_limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_token_limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_reasoning_level: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_reasoning_levels: Vec<ReasoningLevel>,
}

/// A reasoning effort level supported by a specific model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningLevel {
    pub effort: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// List available models for a given provider.
///
/// Returns an empty vec for providers that don't support model listing
/// (dummy, codex, copilot). For `gemini` and `gemini_local`, returns models
/// from the Google Gemini API or local CLI respectively. For `claude`, queries
/// the Anthropic models API.
pub async fn list_models(provider: &str, api_key: &str) -> Result<Vec<ListModel>, LlmError> {
    list_models_with_base_url(provider, api_key, "").await
}

/// List available models for a provider with an explicit base URL.
///
/// Same as [`list_models`] but allows passing a custom `base_url` — required
/// for `openai_compat` and useful when pointing `gemini` / `claude` at a
/// proxy. Providers without a listing API return an empty vec.
pub async fn list_models_with_base_url(
    provider: &str,
    api_key: &str,
    base_url: &str,
) -> Result<Vec<ListModel>, LlmError> {
    match provider {
        "gemini" => gemini::list_models(api_key).await,
        "gemini_local" => gemini_local::list_models().await,
        "claude" => claude::list_models(api_key).await,
        "claude_local" => claude_local::list_models().await,
        "codex" => Ok(Vec::new()),
        "codex_local" => codex_local::list_models().await,
        "copilot" => Ok(Vec::new()),
        "copilot_local" => copilot_local::list_models(api_key).await,
        "openai_compat" => openai_compat::list_models(base_url, api_key).await,
        "pi_local" => pi_local::list_models().await,
        "dummy" => Ok(Vec::new()),
        _ => Ok(Vec::new()),
    }
}

/// Object-safe trait implemented by every LLM provider.
///
/// All methods take `&self` and return boxed futures / streams so the
/// trait can be used as `Box<dyn LlmProviderDyn>`. Construct a provider
/// through [`create_provider`] rather than implementing this by hand.
///
/// The three surfaces are:
/// - [`render_dyn`](Self::render_dyn) — single-shot prompt.
/// - [`chat_dyn`](Self::chat_dyn) — multi-turn. Default discards history.
/// - [`chat_stream_dyn`](Self::chat_stream_dyn) — streaming. Default falls
///   back to `chat_dyn` + one Delta + End.
///
/// The `_with_options_dyn` variants accept [`LlmRequestOptions`].
pub trait LlmProviderDyn: Send + Sync {
    /// Best-effort context window for the configured model.
    ///
    /// Providers that have local model metadata can return this synchronously.
    /// Hosts with API-backed model catalogs should prefer
    /// [`context::resolve_model_context_window_tokens`].
    fn context_window_tokens(&self) -> Option<usize> {
        None
    }

    /// Returns the provider key (e.g., "claude", "openai_compat").
    fn provider_key(&self) -> &'static str {
        "unknown"
    }

    /// Returns the capabilities of this provider.
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::dummy()
    }

    fn render_dyn(
        &self,
        system_prompt: &str,
        user_input: &str,
        result_json: &Value,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + '_>,
    >;

    /// Multi-turn conversation.
    /// WARNING: default implementation discards all history except the last user message.
    /// Override this in any production LLM provider that supports multi-turn conversations.
    fn chat_dyn(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
        result_json: &Value,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + '_>,
    > {
        let last_input = messages
            .iter()
            .rfind(|m| m.role == "user")
            .map(|m| m.text_content())
            .unwrap_or_default();
        self.render_dyn(system_prompt, &last_input, result_json)
    }

    /// Streaming variant. Default falls back to `chat_dyn` and emits a single Delta + End.
    fn chat_stream_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        result_json: &'a Value,
    ) -> BoxStream<'a, LlmStreamEvent> {
        let fut = self.chat_dyn(system_prompt, messages, result_json);
        Box::pin(async_stream::stream! {
            match fut.await {
                Ok(resp) => {
                    yield LlmStreamEvent::Delta { text: resp.rendered_text.clone() };
                    yield LlmStreamEvent::End { total: resp };
                }
                Err(e) => yield LlmStreamEvent::Error { message: e.to_string() },
            }
        })
    }

    /// Multi-turn chat with explicit per-request options.
    ///
    /// Default implementation discards options and forwards to `chat_dyn`.
    /// Providers that honor any options (e.g. `_local` providers wiring CLI
    /// flags) override this method.
    fn chat_with_options_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        _options: &'a LlmRequestOptions,
        result_json: &'a Value,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + 'a>,
    > {
        self.chat_dyn(system_prompt, messages, result_json)
    }

    /// Streaming variant with options. Default forwards to `chat_stream_dyn`.
    fn chat_stream_with_options_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        _options: &'a LlmRequestOptions,
        result_json: &'a Value,
    ) -> BoxStream<'a, LlmStreamEvent> {
        self.chat_stream_dyn(system_prompt, messages, result_json)
    }
}

/// Factory function signature: `(api_key, model, base_url) → boxed provider`.
pub type ProviderFactory =
    fn(api_key: &str, model: &str, base_url: &str) -> Result<Box<dyn LlmProviderDyn>, LlmError>;

/// Built-in provider registry, keyed by lowercase provider name.
/// `antigravity` is an alias for `gemini` preserved from legacy configs.
static REGISTRY: LazyLock<HashMap<&'static str, ProviderFactory>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("claude", claude::make as ProviderFactory);
    m.insert("claude_local", claude_local::make as ProviderFactory);
    m.insert("codex", codex::make as ProviderFactory);
    m.insert("codex_local", codex_local::make as ProviderFactory);
    m.insert("copilot", copilot::make as ProviderFactory);
    m.insert("copilot_local", copilot_local::make as ProviderFactory);
    m.insert("gemini", gemini::make as ProviderFactory);
    m.insert("antigravity", gemini::make as ProviderFactory);
    m.insert("gemini_local", gemini_local::make as ProviderFactory);
    m.insert("openai_compat", openai_compat::make as ProviderFactory);
    m.insert("pi_local", pi_local::make as ProviderFactory);
    m.insert("dummy", dummy::make as ProviderFactory);
    m
});

/// Create a provider by name. Unknown names fall back to `dummy`
/// (preserving current behavior).
#[must_use = "create_provider returns a Result; dropping it silently discards provider construction failures"]
pub fn create_provider(
    provider: &str,
    api_key: &str,
    model: &str,
    base_url: &str,
) -> Result<Box<dyn LlmProviderDyn>, LlmError> {
    match REGISTRY.get(provider) {
        Some(factory) => factory(api_key, model, base_url),
        None => dummy::make(api_key, model, base_url),
    }
}

/// List registered provider names in alphabetical order.
#[must_use]
pub fn registered_providers() -> Vec<&'static str> {
    let mut keys: Vec<_> = REGISTRY.keys().copied().collect();
    keys.sort();
    keys
}

/// Returns the capabilities of a specific registered provider.
pub fn provider_capabilities(provider_key: &str) -> ProviderCapabilities {
    if let Some(factory) = REGISTRY.get(provider_key) {
        if let Ok(provider) = factory("dummy", "dummy", "dummy") {
            return provider.capabilities();
        }
    }
    ProviderCapabilities::dummy()
}

/// List the capabilities of all registered providers.
pub fn registered_provider_capabilities() -> Vec<ProviderCapabilities> {
    registered_providers()
        .into_iter()
        .map(provider_capabilities)
        .collect()
}

#[cfg(test)]
mod stream_event_tests {
    use super::*;

    #[test]
    fn tool_call_variant_can_be_constructed() {
        let call = ToolCall {
            id: "toolu_01".into(),
            name: "get_weather".into(),
            arguments: serde_json::json!({"city": "seoul"}),
        };
        let event = LlmStreamEvent::ToolCall { call };
        match event {
            LlmStreamEvent::ToolCall { call } => {
                assert_eq!(call.id, "toolu_01");
                assert_eq!(call.name, "get_weather");
            }
            _ => panic!("expected ToolCall variant"),
        }
    }

    #[test]
    fn thinking_variant_constructs() {
        let e = LlmStreamEvent::Thinking {
            text: "step 1".into(),
        };
        match e {
            LlmStreamEvent::Thinking { text } => assert_eq!(text, "step 1"),
            _ => panic!("expected Thinking"),
        }
    }

    #[test]
    fn usage_variant_constructs() {
        let e = LlmStreamEvent::Usage {
            input_tokens: Some(100),
            output_tokens: 50,
            estimated_cost_usd: 0.001,
        };
        match e {
            LlmStreamEvent::Usage {
                input_tokens,
                output_tokens,
                estimated_cost_usd,
            } => {
                assert_eq!(input_tokens, Some(100));
                assert_eq!(output_tokens, 50);
                assert!((estimated_cost_usd - 0.001).abs() < 1e-9);
            }
            _ => panic!("expected Usage"),
        }
    }
}

#[cfg(test)]
mod request_options_tests {
    use super::*;

    #[test]
    fn default_options_are_empty() {
        let opts = LlmRequestOptions::default();
        assert!(opts.image_paths.is_empty());
        assert!(opts.working_dir.is_none());
        assert!(!opts.allow_web_search);
        assert!(!opts.allow_local_read);
        assert!(opts.max_turns.is_none());
        assert!(opts.reasoning_effort.is_none());
        assert!(opts.timeout.is_none());
        assert!(opts.temperature.is_none());
        assert!(opts.max_tokens.is_none());
        assert!(opts.top_p.is_none());
        assert!(opts.stop_sequences.is_empty());
        assert!(opts.tools.is_empty());
        assert!(opts.tool_choice.is_none());
        assert!(opts.thinking_budget_tokens.is_none());
        assert!(opts.cache_breakpoints.is_empty());
    }

    #[test]
    fn options_timeout_can_be_set() {
        use std::time::Duration;
        let opts = LlmRequestOptions {
            timeout: Some(Duration::from_millis(500)),
            ..Default::default()
        };
        assert_eq!(opts.timeout, Some(Duration::from_millis(500)));
    }

    #[test]
    fn options_can_be_constructed_via_struct_update() {
        let opts = LlmRequestOptions {
            allow_web_search: true,
            max_turns: Some(5),
            ..Default::default()
        };
        assert!(opts.allow_web_search);
        assert_eq!(opts.max_turns, Some(5));
    }
}

#[cfg(test)]
mod response_token_tests {
    use super::*;

    #[test]
    fn response_default_token_fields_are_none() {
        let resp = LlmResponse {
            rendered_text: String::new(),
            model: String::new(),
            estimated_cost_usd: 0.0,
            input_tokens: None,
            output_tokens: None,
            tool_calls: Vec::new(),
            ..Default::default()
        };
        assert_eq!(resp.input_tokens, None);
        assert_eq!(resp.output_tokens, None);
    }

    #[test]
    fn default_response_has_empty_tool_calls() {
        let r = LlmResponse::default();
        assert!(r.tool_calls.is_empty());
        assert_eq!(r.rendered_text, "");
        assert_eq!(r.model, "");
        assert_eq!(r.estimated_cost_usd, 0.0);
        assert!(r.input_tokens.is_none());
        assert!(r.output_tokens.is_none());
        assert!(r.thinking_text.is_none());
        assert!(r.cache_creation_input_tokens.is_none());
        assert!(r.cache_read_input_tokens.is_none());
    }

    #[test]
    fn response_default_via_struct_update_works() {
        let r = LlmResponse {
            rendered_text: "test".into(),
            ..Default::default()
        };
        assert_eq!(r.rendered_text, "test");
        assert!(r.tool_calls.is_empty());
        assert!(r.thinking_text.is_none());
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn registry_has_all_expected_providers() {
        let names = registered_providers();
        for expected in [
            "claude",
            "claude_local",
            "codex",
            "codex_local",
            "copilot",
            "copilot_local",
            "gemini",
            "antigravity",
            "gemini_local",
            "openai_compat",
            "pi_local",
            "dummy",
        ] {
            assert!(
                names.contains(&expected),
                "registry missing provider: {expected}"
            );
        }
    }

    #[test]
    fn create_provider_dummy_succeeds() {
        let provider = create_provider("dummy", "", "", "");
        assert!(provider.is_ok(), "dummy must never fail");
    }

    #[test]
    fn create_provider_claude_without_key_returns_missing_config() {
        let result = create_provider("claude", "", "anything", "");
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected MissingConfig for claude but got Ok"),
        };
        assert!(
            matches!(err, LlmError::MissingConfig { ref provider, .. } if provider == "claude"),
            "expected MissingConfig for claude, got: {err:?}"
        );
    }

    #[test]
    fn create_provider_unknown_falls_back_to_dummy() {
        let provider = create_provider("unknown_xyz", "", "", "");
        assert!(
            provider.is_ok(),
            "unknown provider must fall back to dummy (current behavior)"
        );
    }
}

#[cfg(test)]
mod new_types_tests {
    use super::*;

    #[test]
    fn chat_content_from_str_creates_text() {
        let c: ChatContent = "hello".into();
        assert_eq!(c, ChatContent::Text("hello".into()));
    }

    #[test]
    fn chat_content_from_string_creates_text() {
        let c: ChatContent = String::from("hello").into();
        assert_eq!(c, ChatContent::Text("hello".into()));
    }

    #[test]
    fn tool_def_serializes_round_trip() {
        let def = ToolDef {
            name: "get_weather".into(),
            description: "Get weather for a city".into(),
            input_schema: serde_json::json!({"type": "object"}),
        };
        let s = serde_json::to_string(&def).unwrap();
        let back: ToolDef = serde_json::from_str(&s).unwrap();
        assert_eq!(back.name, def.name);
        assert_eq!(back.description, def.description);
        assert_eq!(back.input_schema, def.input_schema);
    }

    #[test]
    fn tool_call_serializes_round_trip() {
        let call = ToolCall {
            id: "toolu_01".into(),
            name: "get_weather".into(),
            arguments: serde_json::json!({"city": "seoul"}),
        };
        let s = serde_json::to_string(&call).unwrap();
        let back: ToolCall = serde_json::from_str(&s).unwrap();
        assert_eq!(back.id, call.id);
        assert_eq!(back.name, call.name);
        assert_eq!(back.arguments, call.arguments);
    }

    #[test]
    fn tool_choice_variants_are_distinct() {
        let cases = [
            ToolChoice::Auto,
            ToolChoice::Required,
            ToolChoice::None,
            ToolChoice::Specific("get_weather".into()),
        ];
        // Ensure PartialEq works
        assert_eq!(cases[0], ToolChoice::Auto);
        assert_ne!(cases[0], cases[1]);
        assert_eq!(cases[3], ToolChoice::Specific("get_weather".into()));
    }

    #[test]
    fn image_source_variants_construct_correctly() {
        let b = ImageSource::Base64("AAAA".into());
        let u = ImageSource::Url("https://example.com/x.png".into());
        match b {
            ImageSource::Base64(d) => assert_eq!(d, "AAAA"),
            _ => panic!("expected Base64"),
        }
        match u {
            ImageSource::Url(s) => assert_eq!(s, "https://example.com/x.png"),
            _ => panic!("expected Url"),
        }
    }

    #[test]
    fn chat_content_image_construction() {
        let c = ChatContent::Image {
            source: ImageSource::Base64("AAAA".into()),
            mime_type: "image/png".into(),
        };
        if let ChatContent::Image { mime_type, .. } = c {
            assert_eq!(mime_type, "image/png");
        } else {
            panic!("expected Image variant");
        }
    }

    #[test]
    fn chat_content_tool_result_construction() {
        let c = ChatContent::ToolResult {
            tool_use_id: "toolu_01".into(),
            content: "25°C".into(),
            is_error: false,
        };
        if let ChatContent::ToolResult {
            tool_use_id,
            content,
            is_error,
        } = c
        {
            assert_eq!(tool_use_id, "toolu_01");
            assert_eq!(content, "25°C");
            assert!(!is_error);
        } else {
            panic!("expected ToolResult variant");
        }
    }
}

#[cfg(test)]
mod chat_message_tests {
    use super::*;

    #[test]
    fn user_constructor_creates_text_block() {
        let msg = ChatMessage::user("hello");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, vec![ChatContent::Text("hello".into())]);
    }

    #[test]
    fn assistant_constructor_creates_text_block() {
        let msg = ChatMessage::assistant("hi there");
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, vec![ChatContent::Text("hi there".into())]);
    }

    #[test]
    fn user_with_image_combines_text_and_image() {
        let img = ChatContent::Image {
            source: ImageSource::Base64("AAAA".into()),
            mime_type: "image/png".into(),
        };
        let msg = ChatMessage::user_with_image("describe", img.clone());
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content.len(), 2);
        assert_eq!(msg.content[0], ChatContent::Text("describe".into()));
        assert_eq!(msg.content[1], img);
    }

    #[test]
    fn tool_result_uses_user_role() {
        let msg = ChatMessage::tool_result("toolu_01", "25C", false);
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content.len(), 1);
        if let ChatContent::ToolResult {
            tool_use_id,
            content,
            is_error,
        } = &msg.content[0]
        {
            assert_eq!(tool_use_id, "toolu_01");
            assert_eq!(content, "25C");
            assert!(!*is_error);
        } else {
            panic!("expected ToolResult");
        }
    }

    #[test]
    fn text_content_concatenates_text_blocks() {
        let msg = ChatMessage::user_multipart(vec![
            ChatContent::Text("Hello, ".into()),
            ChatContent::Image {
                source: ImageSource::Base64("AAAA".into()),
                mime_type: "image/png".into(),
            },
            ChatContent::Text("world!".into()),
        ]);
        assert_eq!(msg.text_content(), "Hello, world!");
    }

    #[test]
    fn text_content_skips_tool_result() {
        let mut msg = ChatMessage::user("ignore me");
        msg.content.push(ChatContent::ToolResult {
            tool_use_id: "x".into(),
            content: "y".into(),
            is_error: false,
        });
        assert_eq!(msg.text_content(), "ignore me");
    }
}

#[cfg(test)]
mod image_helpers_tests {
    use super::*;
    use std::io::Write;

    /// Tiny valid PNG bytes (1×1 transparent pixel) — magic header for testing.
    const TINY_PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
        0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49,
        0x44, 0x41, 0x54, 0x78, 0x9C, 0x62, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A,
        0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn image_from_path_with_png_extension_succeeds() {
        let mut tmp = std::env::temp_dir();
        tmp.push(format!("llm_router_test_{}.png", uuid::Uuid::new_v4()));
        let mut f = std::fs::File::create(&tmp).unwrap();
        f.write_all(TINY_PNG).unwrap();
        f.sync_all().unwrap();

        let c = ChatContent::image_from_path(&tmp).expect("read");
        std::fs::remove_file(&tmp).ok();

        match c {
            ChatContent::Image {
                source: ImageSource::Base64(_),
                mime_type,
            } => {
                assert_eq!(mime_type, "image/png");
            }
            _ => panic!("expected Image/Base64"),
        }
    }

    #[test]
    fn image_from_path_with_unknown_extension_uses_octet_stream() {
        let mut tmp = std::env::temp_dir();
        tmp.push(format!("llm_router_test_{}.xyz", uuid::Uuid::new_v4()));
        let mut f = std::fs::File::create(&tmp).unwrap();
        f.write_all(b"garbage").unwrap();

        let c = ChatContent::image_from_path(&tmp).expect("read");
        std::fs::remove_file(&tmp).ok();

        match c {
            ChatContent::Image { mime_type, .. } => {
                assert_eq!(mime_type, "application/octet-stream");
            }
            _ => panic!("expected Image"),
        }
    }

    #[test]
    fn image_from_base64_auto_detects_png() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD.encode(TINY_PNG);
        let c = ChatContent::image_from_base64_auto(b64).expect("detect");
        match c {
            ChatContent::Image { mime_type, .. } => assert_eq!(mime_type, "image/png"),
            _ => panic!("expected Image"),
        }
    }

    #[test]
    fn image_from_base64_auto_returns_err_for_garbage() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"this is not a valid image");
        let result = ChatContent::image_from_base64_auto(b64);
        assert!(result.is_err());
    }

    #[test]
    fn image_from_base64_auto_returns_err_for_invalid_base64() {
        let result = ChatContent::image_from_base64_auto("not-valid-base64!!");
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod cache_breakpoint_tests {
    use super::*;

    #[test]
    fn cache_breakpoint_variants_construct() {
        let _s = CacheBreakpoint::System;
        let _m = CacheBreakpoint::MessageIndex(3);
    }

    #[test]
    fn cache_breakpoint_clones() {
        let b = CacheBreakpoint::MessageIndex(5);
        let _c = b.clone();
    }
}
