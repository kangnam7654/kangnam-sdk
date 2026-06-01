#![deny(unsafe_code)]
//! LLM tool-call runner between [`kangnam_harness_core::AgentTool`] and
//! [`kangnam_router::LlmProviderDyn`].
//!
//! The harness core provides a domain-agnostic `AgentTool` trait
//! (filesystem capabilities, suspend/resume via `InteractionBridge`,
//! permission checks). The router crate provides multi-provider HTTP
//! clients (Claude, OpenAI-compat / LM Studio, Gemini, Codex, Copilot)
//! with first-class tool-calling on the wire.
//!
//! This crate plumbs the two together with [`LlmAgent`], a builder that:
//!
//! 1. Adapts each [`AgentTool<C>`] into a [`ToolDef`] the provider
//!    advertises to the model.
//! 2. Drives a multi-turn loop: model emits `tool_calls` → runner
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
//! # use kangnam_harness_core::{
//! #     AgentTool, DefaultCapabilities, ToolCtx, ToolResult,
//! #     FsCallbacks, WebCallbacks, InteractionBridge, ToolError, AwaitKind,
//! # };
//! # use tokio::sync::oneshot;
//! # use std::path::Path;
//! use kangnam_harness_llm_tool_runner::LlmAgent;
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

use kangnam_harness_core::{AgentTool, DefaultCapabilities, ToolCtx, ToolResult};
use kangnam_router::{
    ChatMessage, LlmError, LlmProviderDyn, LlmRequestOptions, ToolCall, ToolDef,
    context::{ContextWindowBudget, compact_messages_for_window},
};
use serde_json::{Value, json};

/// Runner errors. `Llm` wraps provider failures; `UnknownTool` fires
/// when the model invokes a tool name not present in the registry.
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("provider error: {0}")]
    Llm(#[from] LlmError),

    #[error("provider does not support a required feature: {0}")]
    UnsupportedProviderFeature(String),

    #[error("model invoked unknown tool '{name}' (registered: {registered:?})")]
    UnknownTool {
        name: String,
        registered: Vec<String>,
    },

    /// A tool requested suspend/resume via `ToolResult::AwaitUser`. The
    /// runner cannot drive an interactive turn inside a non-interactive
    /// LLM loop, so it surfaces this as a hard error. Hosts that want
    /// interactive tools should drive the loop themselves and call
    /// `AgentTool::execute` outside the runner.
    #[error(
        "tool '{tool_name}' suspended turn awaiting user input ({kind:?}); \
         runner does not support interactive tools — drive the loop manually \
         or strip interactive tools before invoking the runner"
    )]
    SuspendedTurn {
        tool_name: String,
        await_id: String,
        kind: kangnam_harness_core::AwaitKind,
    },

    #[deprecated(
        since = "0.3.1",
        note = "run now returns AgentRun { stop_reason: MaxIterations } with partial history"
    )]
    #[error("max iterations exceeded ({max}): model still emitting tool_calls")]
    MaxIterations { max: u32 },

    #[error("invalid message history: {0}")]
    InvalidMessages(String),

    /// MCP server interaction failed during tool discovery (e.g.
    /// `with_mcp_server_stdio` couldn't spawn the server, complete the
    /// `initialize` handshake, or `tools/list` returned an error).
    #[error("MCP server '{server_label}': {source}")]
    Mcp {
        server_label: String,
        #[source]
        source: mcp::McpError,
    },
}

/// Why an [`AgentRun`] stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentRunStopReason {
    /// The model returned a terminal assistant message without tool calls.
    FinalAnswer,
    /// The iteration ceiling was reached while the model was still requesting
    /// tools. The returned `AgentRun` contains the partial message history and
    /// every tool invocation completed before the limit.
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
    /// The model's terminal text answer. Empty when `stop_reason` is
    /// `MaxIterations`.
    pub final_text: String,
    /// Every tool the model invoked, in dispatch order.
    pub tool_invocations: Vec<ToolInvocation>,
    /// Number of model round-trips performed (1 = no tool calls; 2+ =
    /// at least one tool-call iteration).
    pub iterations: u32,
    /// Total accumulated token usage and cost across all iterations.
    pub total_usage: kangnam_router::LlmUsage,
    /// Why the run stopped.
    pub stop_reason: AgentRunStopReason,
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
    context_window_budget: Option<ContextWindowBudget>,
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
            context_window_budget: None,
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
    /// one provider round-trip; if the model is still requesting tools after
    /// this many turns, the loop returns an [`AgentRun`] with
    /// [`AgentRunStopReason::MaxIterations`] and the partial history.
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

    /// Enable deterministic context compaction before each model call.
    ///
    /// Older turns are folded into one summary message when the estimated
    /// request would exceed `max_context_tokens`. Recent turns stay intact,
    /// and tool-call/tool-result pairs are kept together so provider wire
    /// formats remain valid.
    #[must_use]
    pub fn with_context_window_tokens(mut self, max_context_tokens: usize) -> Self {
        self.context_window_budget = Some(ContextWindowBudget::new(max_context_tokens));
        self
    }

    /// Enable deterministic context compaction with explicit budget knobs.
    #[must_use]
    pub fn with_context_window_budget(mut self, budget: ContextWindowBudget) -> Self {
        self.context_window_budget = Some(budget);
        self
    }

    /// Drive the multi-turn loop with `user_input` as the initial user
    /// message. Returns the full `AgentRun` (history + final text +
    /// invocation log) on success.
    pub async fn run(&self, user_input: impl Into<String>) -> Result<AgentRun, BridgeError> {
        self.run_messages(vec![ChatMessage::user(user_input)]).await
    }

    /// Drive the multi-turn loop with an existing conversation history.
    ///
    /// `messages` must be non-empty and end with a user turn. The bridge
    /// appends assistant tool-call turns, tool results, and the final
    /// assistant answer to this history as the loop progresses.
    pub async fn run_messages(
        &self,
        mut messages: Vec<ChatMessage>,
    ) -> Result<AgentRun, BridgeError> {
        if !self.tools.is_empty() && !self.provider.capabilities().supports_tool_calling {
            return Err(BridgeError::UnsupportedProviderFeature(format!(
                "provider '{}' does not support tool calling",
                self.provider.provider_key()
            )));
        }

        if messages.is_empty() {
            return Err(BridgeError::InvalidMessages(
                "message history must not be empty".into(),
            ));
        }
        if messages.last().is_some_and(|m| m.role != "user") {
            return Err(BridgeError::InvalidMessages(
                "message history must end with a user turn".into(),
            ));
        }
        let mut tool_invocations: Vec<ToolInvocation> = Vec::new();
        let mut total_usage = kangnam_router::LlmUsage::default();

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
            let request_messages;
            let provider_budget = self.context_window_budget.clone().or_else(|| {
                self.provider
                    .context_window_tokens()
                    .map(ContextWindowBudget::new)
            });
            let messages_for_request = if let Some(budget) = &provider_budget {
                let compacted = compact_messages_for_window(&self.system_prompt, &messages, budget);
                if compacted.compacted {
                    tracing::info!(
                        original_tokens = compacted.original_tokens,
                        compacted_tokens = compacted.compacted_tokens,
                        message_count = compacted.messages.len(),
                        "compacted LLM context before provider call"
                    );
                }
                request_messages = compacted.messages;
                request_messages.as_slice()
            } else {
                messages.as_slice()
            };

            let resp = self
                .provider
                .chat_with_options_dyn(
                    &self.system_prompt,
                    messages_for_request,
                    &options,
                    &json!({}),
                )
                .await?;

            total_usage.add(&resp.usage());

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
                    total_usage,
                    stop_reason: AgentRunStopReason::FinalAnswer,
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
                    registered: self
                        .tools
                        .iter()
                        .map(|t| t.tool.name().to_string())
                        .collect(),
                })?;

                let outcome = rt.tool.execute(call.arguments.clone(), &self.ctx).await;
                let (result_text, is_error) = match outcome {
                    ToolResult::Success { content } => match content {
                        Value::String(s) => (s, false),
                        other => (other.to_string(), false),
                    },
                    ToolResult::Failed { error } => (error, true),
                    ToolResult::AwaitUser { await_id, kind, .. } => {
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

        Ok(AgentRun {
            messages,
            final_text: String::new(),
            tool_invocations,
            iterations: self.max_iterations,
            total_usage,
            stop_reason: AgentRunStopReason::MaxIterations {
                max: self.max_iterations,
            },
        })
    }

    /// Number of registered tools.
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Spawn an MCP server over stdio, run the handshake, list its
    /// tools, and register each one as an [`mcp::McpAgentTool`].
    ///
    /// `command` + `args` are forwarded to `tokio::process::Command`
    /// (e.g. `("npx", &["-y", "@modelcontextprotocol/server-everything"])`).
    /// `server_label` is used in error messages so multi-server agents
    /// can tell which spawn failed.
    ///
    /// Returns the augmented agent on success. The MCP client is held
    /// internally by each `McpAgentTool` so the spawned process lives
    /// as long as the agent (or until the stdio reader observes EOF).
    ///
    /// # Errors
    ///
    /// Wraps every MCP-side failure in [`BridgeError::Mcp`].
    pub async fn with_mcp_server_stdio(
        mut self,
        server_label: impl Into<String>,
        command: &str,
        args: &[&str],
    ) -> Result<Self, BridgeError> {
        let server_label = server_label.into();
        let client = mcp::McpClient::new_stdio(command, args, mcp::ClientInfo::default())
            .await
            .map_err(|source| BridgeError::Mcp {
                server_label: server_label.clone(),
                source,
            })?;

        let tools = client
            .list_tools()
            .await
            .map_err(|source| BridgeError::Mcp {
                server_label: server_label.clone(),
                source,
            })?;

        for tool in tools {
            let description = tool.description.clone().unwrap_or_default();
            let adapter: mcp::McpAgentTool<C> = mcp::McpAgentTool::new(client.clone(), tool);
            self = self.with_boxed_tool(Arc::new(adapter), description);
        }
        Ok(self)
    }

    /// Register every tool advertised by an existing [`mcp::McpClient`].
    /// Lower-level than [`Self::with_mcp_server_stdio`] — useful when
    /// the client was constructed against a custom transport (e.g.
    /// [`mcp::InMemoryTransport`] in tests, or a future SSE transport).
    pub async fn with_mcp_client(
        mut self,
        server_label: impl Into<String>,
        client: mcp::McpClient,
    ) -> Result<Self, BridgeError> {
        let server_label = server_label.into();
        let tools = client
            .list_tools()
            .await
            .map_err(|source| BridgeError::Mcp {
                server_label: server_label.clone(),
                source,
            })?;
        for tool in tools {
            let description = tool.description.clone().unwrap_or_default();
            let adapter: mcp::McpAgentTool<C> = mcp::McpAgentTool::new(client.clone(), tool);
            self = self.with_boxed_tool(Arc::new(adapter), description);
        }
        Ok(self)
    }
}

pub mod mcp;

#[cfg(feature = "test-util")]
pub mod test_util;
