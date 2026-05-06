#![deny(unsafe_code)]
//! Bridge between [`kangnam_harness_runtime::AgentTool`] and
//! [`kangnam_router::LlmProviderDyn`].
//!
//! The harness runtime provides a domain-agnostic `AgentTool` trait
//! (filesystem capabilities, suspend/resume via `InteractionBridge`,
//! permission checks). The router crate provides multi-provider HTTP
//! clients (Claude, OpenAI-compat / LM Studio, Gemini, Codex, Copilot)
//! with first-class tool-calling on the wire.
//!
//! This crate plumbs the two together with [`LlmAgent`], a builder that:
//!
//! 1. Adapts each [`AgentTool<C>`] into a [`ToolDef`] the provider
//!    advertises to the model.
//! 2. Drives a multi-turn loop: model emits `tool_calls` → bridge
//!    dispatches each to the matching `AgentTool::execute` → results
//!    feed back as `tool_result` blocks → model is queried again until
//!    it produces a text-only answer or the iteration ceiling is hit.
//! 3. Records every assistant tool-call turn with
//!    [`ChatMessage::assistant_with_tool_calls`] so the OpenAI/LM Studio
//!    wire format correctly pairs each [`ToolCall::id`] with its
//!    subsequent [`ChatContent::ToolResult::tool_use_id`].
//!
//! # Example — LM Studio with a single tool
//!
//! ```no_run
//! # use std::sync::Arc;
//! # use async_trait::async_trait;
//! # use serde_json::{json, Value};
//! # use kangnam_harness_runtime::{
//! #     AgentTool, DefaultCapabilities, ToolCtx, ToolResult,
//! #     FsCallbacks, WebCallbacks, InteractionBridge, ToolError, AwaitKind,
//! # };
//! # use tokio::sync::oneshot;
//! # use std::path::Path;
//! use kangnam_harness_llm_bridge::LlmAgent;
//! use kangnam_router::create_provider;
//!
//! struct GetWeather;
//!
//! #[async_trait]
//! impl AgentTool for GetWeather {
//!     fn name(&self) -> &str { "get_weather" }
//!     fn parameters(&self) -> Value {
//!         json!({
//!             "type": "object",
//!             "properties": {"city": {"type": "string"}},
//!             "required": ["city"],
//!         })
//!     }
//!     async fn execute(&self, params: Value, _: &ToolCtx) -> ToolResult {
//!         let city = params["city"].as_str().unwrap_or("?");
//!         ToolResult::Success { content: json!({"city": city, "temp_c": 25}) }
//!     }
//! }
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! # struct Fs; #[async_trait] impl FsCallbacks for Fs {
//! #   async fn read(&self,_:&Path)->Result<Vec<u8>,ToolError>{Ok(vec![])}
//! #   async fn write(&self,_:&Path,_:&[u8])->Result<(),ToolError>{Ok(())}
//! #   async fn str_replace(&self,_:&Path,_:&str,_:&str)->Result<(),ToolError>{Ok(())}
//! # }
//! # struct Web; #[async_trait] impl WebCallbacks for Web {
//! #   async fn fetch(&self,_:&str)->Result<Vec<u8>,ToolError>{Ok(vec![])}
//! # }
//! # struct B; #[async_trait] impl InteractionBridge for B {
//! #   async fn register_question_form(&self,_:&Value)->Result<(String,oneshot::Receiver<Value>),ToolError>{
//! #       let (_,rx)=oneshot::channel();Ok(("x".into(),rx))
//! #   }
//! #   async fn register_preview(&self,_:&Value)->Result<(String,oneshot::Receiver<Value>),ToolError>{
//! #       let (_,rx)=oneshot::channel();Ok(("x".into(),rx))
//! #   }
//! # }
//! let provider = create_provider("openai_compat", "", "qwen2.5-7b-instruct", "http://localhost:1234/v1")?;
//! let ctx = ToolCtx::new("session-1", DefaultCapabilities {
//!     fs: Arc::new(Fs), web: Arc::new(Web), image: None, bridge: Arc::new(B),
//! });
//!
//! let run = LlmAgent::new(provider, ctx)
//!     .with_tool(GetWeather, "Get current weather for a city.")
//!     .with_system_prompt("You are a helpful assistant. Use tools when asked.")
//!     .run("What's the weather in Seoul?")
//!     .await?;
//!
//! println!("{}", run.final_text);
//! # Ok(()) }
//! ```

use std::sync::Arc;

use kangnam_harness_runtime::{AgentTool, DefaultCapabilities, ToolCtx, ToolResult};
use kangnam_router::{
    ChatMessage, LlmError, LlmProviderDyn, LlmRequestOptions, ToolCall, ToolDef,
};
use serde_json::{Value, json};

/// Bridge errors. `Llm` wraps provider failures; `UnknownTool` fires
/// when the model invokes a tool name not present in the registry;
/// `MaxIterations` fires when the loop exceeds [`LlmAgent::with_max_iterations`].
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("provider error: {0}")]
    Llm(#[from] LlmError),

    #[error("model invoked unknown tool '{name}' (registered: {registered:?})")]
    UnknownTool {
        name: String,
        registered: Vec<String>,
    },

    /// A tool requested suspend/resume via `ToolResult::AwaitUser`. The
    /// bridge cannot drive an interactive turn inside a non-interactive
    /// LLM loop, so it surfaces this as a hard error. Hosts that want
    /// interactive tools should drive the loop themselves and call
    /// `AgentTool::execute` outside the bridge.
    #[error(
        "tool '{tool_name}' suspended turn awaiting user input ({kind:?}); \
         bridge does not support interactive tools — drive the loop manually \
         or strip interactive tools before invoking the bridge"
    )]
    SuspendedTurn {
        tool_name: String,
        await_id: String,
        kind: kangnam_harness_runtime::AwaitKind,
    },

    #[error("max iterations exceeded ({max}): model still emitting tool_calls")]
    MaxIterations { max: u32 },
}

/// One tool invocation captured during a [`LlmAgent::run`].
#[derive(Debug, Clone)]
pub struct ToolInvocation {
    /// The tool call as the model emitted it.
    pub call: ToolCall,
    /// Serialized result string fed back to the model (the same content
    /// that appears as [`kangnam_router::ChatContent::ToolResult::content`]).
    pub result: String,
    /// `true` when the tool returned [`ToolResult::Failed`]; the
    /// corresponding `tool_result` block was sent with `is_error: true`.
    pub is_error: bool,
}

/// Outcome of a [`LlmAgent::run`].
#[derive(Debug)]
pub struct AgentRun {
    /// Full conversation history including the user input, every
    /// assistant turn (with tool_calls when applicable), and every
    /// tool_result the bridge fed back.
    pub messages: Vec<ChatMessage>,
    /// The model's terminal text answer — the assistant turn that did
    /// not emit any tool_calls.
    pub final_text: String,
    /// Every tool the model invoked, in dispatch order.
    pub tool_invocations: Vec<ToolInvocation>,
    /// Number of model round-trips performed (1 = no tool calls; 2+ =
    /// at least one tool-call iteration).
    pub iterations: u32,
}

/// Adapter wrapping an `AgentTool` with its model-facing description.
/// `AgentTool` exposes `name()` and `parameters()` but no description,
/// so the bridge captures it externally.
struct RegisteredTool<C> {
    tool: Arc<dyn AgentTool<C>>,
    description: String,
}

/// Multi-turn LLM agent with tool dispatch.
///
/// Generic over a capability bundle `C`; defaults to
/// [`DefaultCapabilities`]. Domains with their own capability struct
/// (Travel Planner, finance advisors) instantiate `LlmAgent<TheirCaps>`
/// and register tools that implement `AgentTool<TheirCaps>`.
pub struct LlmAgent<C = DefaultCapabilities> {
    provider: Box<dyn LlmProviderDyn>,
    tools: Vec<RegisteredTool<C>>,
    ctx: ToolCtx<C>,
    system_prompt: String,
    options: LlmRequestOptions,
    max_iterations: u32,
}

impl<C: Send + Sync + 'static> LlmAgent<C> {
    /// Construct a new agent. The provider is typically created via
    /// [`kangnam_router::create_provider`]; for LM Studio use
    /// `("openai_compat", "", "<model>", "http://localhost:1234/v1")`.
    pub fn new(provider: Box<dyn LlmProviderDyn>, ctx: ToolCtx<C>) -> Self {
        Self {
            provider,
            tools: Vec::new(),
            ctx,
            system_prompt: String::new(),
            options: LlmRequestOptions::default(),
            max_iterations: 8,
        }
    }

    /// Register a tool. `description` is what the model sees alongside
    /// the tool's JSON Schema parameters — keep it concise and
    /// behavior-focused.
    #[must_use]
    pub fn with_tool<T>(mut self, tool: T, description: impl Into<String>) -> Self
    where
        T: AgentTool<C> + 'static,
    {
        self.tools.push(RegisteredTool {
            tool: Arc::new(tool),
            description: description.into(),
        });
        self
    }

    /// Register an already-boxed tool. Useful when tools are
    /// constructed dynamically.
    #[must_use]
    pub fn with_boxed_tool(
        mut self,
        tool: Arc<dyn AgentTool<C>>,
        description: impl Into<String>,
    ) -> Self {
        self.tools.push(RegisteredTool {
            tool,
            description: description.into(),
        });
        self
    }

    /// Set the system prompt sent on every turn.
    #[must_use]
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Override the iteration ceiling (default `8`). Each iteration is
    /// one provider round-trip; the loop returns
    /// [`BridgeError::MaxIterations`] if the model is still requesting
    /// tools after this many turns.
    #[must_use]
    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max;
        self
    }

    /// Provide the per-request options forwarded to every model call.
    /// `tools` and `tool_choice` are overridden by the bridge from the
    /// registered tools; everything else (temperature, max_tokens,
    /// stop_sequences, …) flows through verbatim.
    #[must_use]
    pub fn with_options(mut self, options: LlmRequestOptions) -> Self {
        self.options = options;
        self
    }

    /// Drive the multi-turn loop with `user_input` as the initial user
    /// message. Returns the full `AgentRun` (history + final text +
    /// invocation log) on success.
    pub async fn run(&self, user_input: impl Into<String>) -> Result<AgentRun, BridgeError> {
        let mut messages = vec![ChatMessage::user(user_input)];
        let mut tool_invocations: Vec<ToolInvocation> = Vec::new();

        // Inject tool advertisements into options each call (clone so
        // caller-provided options remain unchanged across iterations).
        let mut options = self.options.clone();
        options.tools = self
            .tools
            .iter()
            .map(|rt| ToolDef {
                name: rt.tool.name().to_string(),
                description: rt.description.clone(),
                input_schema: rt.tool.parameters(),
            })
            .collect();

        for iter in 0..self.max_iterations {
            let resp = self
                .provider
                .chat_with_options_dyn(&self.system_prompt, &messages, &options, &json!({}))
                .await?;

            if resp.tool_calls.is_empty() {
                // Terminal answer — record as a plain assistant turn
                // and return.
                let final_text = resp.rendered_text.clone();
                messages.push(ChatMessage::assistant(final_text.clone()));
                return Ok(AgentRun {
                    messages,
                    final_text,
                    tool_invocations,
                    iterations: iter + 1,
                });
            }

            // Record the assistant turn with tool_calls so the next
            // request body re-emits them on the wire (OpenAI/LM Studio
            // require tool_call_id pairing to its originating
            // assistant message).
            messages.push(ChatMessage::assistant_with_tool_calls(
                resp.rendered_text.clone(),
                resp.tool_calls.clone(),
            ));

            for call in &resp.tool_calls {
                let registered = self.tools.iter().find(|rt| rt.tool.name() == call.name);
                let rt = registered.ok_or_else(|| BridgeError::UnknownTool {
                    name: call.name.clone(),
                    registered: self.tools.iter().map(|t| t.tool.name().to_string()).collect(),
                })?;

                let outcome = rt.tool.execute(call.arguments.clone(), &self.ctx).await;
                let (result_text, is_error) = match outcome {
                    ToolResult::Success { content } => match content {
                        Value::String(s) => (s, false),
                        other => (other.to_string(), false),
                    },
                    ToolResult::Failed { error } => (error, true),
                    ToolResult::AwaitUser {
                        await_id,
                        kind,
                        ..
                    } => {
                        return Err(BridgeError::SuspendedTurn {
                            tool_name: call.name.clone(),
                            await_id,
                            kind,
                        });
                    }
                };

                tool_invocations.push(ToolInvocation {
                    call: call.clone(),
                    result: result_text.clone(),
                    is_error,
                });
                messages.push(ChatMessage::tool_result(&call.id, result_text, is_error));
            }
        }

        Err(BridgeError::MaxIterations {
            max: self.max_iterations,
        })
    }

    /// Number of registered tools.
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }
}

#[cfg(feature = "test-util")]
pub mod test_util;
