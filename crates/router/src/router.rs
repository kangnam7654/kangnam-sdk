//! High-level router facade over the provider registry.
//!
//! The lower-level API in this crate exposes individual providers through
//! [`crate::create_provider`]. [`Router`] sits one level above that: hosts
//! register provider configurations once, then send every request through the
//! same `chat` / `chat_stream` surface.

use std::collections::HashMap;

use futures::stream::BoxStream;
use serde_json::Value;

use crate::{
    ChatMessage, LlmError, LlmProviderDyn, LlmRequestOptions, LlmResponse, LlmStreamEvent,
    create_provider,
};

pub const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";
pub const OPENCLAW_GATEWAY_BASE_URL: &str = "http://127.0.0.1:18789/v1";
pub const OPENCLAW_DEFAULT_MODEL: &str = "openclaw/default";
pub const HERMES_AGENT_BASE_URL: &str = "http://127.0.0.1:8642/v1";
pub const HERMES_AGENT_DEFAULT_MODEL: &str = "hermes-agent";
pub const OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434/v1";
pub const LM_STUDIO_BASE_URL: &str = "http://127.0.0.1:1234/v1";
pub const HERMES_SUBSCRIPTION_PROXY_BASE_URL: &str = "http://127.0.0.1:8645/v1";

/// How a researched provider or agent can be connected to this router.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIntegrationStatus {
    /// No new provider implementation is needed; use `openai_compat`.
    ReadyViaOpenAiCompat,
    /// Needs a local CLI/RPC provider implementation.
    PlannedLocalCli,
    /// Needs a native HTTP provider implementation.
    PlannedNativeHttp,
}

/// Research-backed integration candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderIntegrationCandidate {
    pub key: &'static str,
    pub display_name: &'static str,
    pub status: ProviderIntegrationStatus,
    pub notes: &'static str,
}

/// Subscription or account-auth surface observed in peer products.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionAuthSurface {
    /// Uses a locally installed vendor CLI/account store.
    LocalCliAccount,
    /// Uses OAuth/device-code credentials managed by the peer product.
    ProductManagedOAuth,
    /// Exposes product-managed subscription credentials through a local
    /// OpenAI-compatible proxy.
    LocalOpenAiCompatibleProxy,
    /// Plain API key or bearer token; not subscription-specific.
    ApiKey,
}

/// Recommended Kangnam Router attach path for a benchmarked subscription route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouterAttachPath {
    ExistingProvider(&'static str),
    OpenAiCompatiblePreset(&'static str),
    PlannedProvider(&'static str),
}

/// Benchmark row for subscription-backed provider routes in peer products.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubscriptionProviderBenchmark {
    pub product: &'static str,
    pub route_key: &'static str,
    pub auth_surface: SubscriptionAuthSurface,
    pub attach_path: RouterAttachPath,
    pub notes: &'static str,
}

const PROVIDER_INTEGRATION_CANDIDATES: &[ProviderIntegrationCandidate] = &[
    ProviderIntegrationCandidate {
        key: "openclaw_gateway",
        display_name: "OpenClaw Gateway",
        status: ProviderIntegrationStatus::ReadyViaOpenAiCompat,
        notes: "OpenClaw exposes /v1/chat/completions and /v1/responses on its gateway.",
    },
    ProviderIntegrationCandidate {
        key: "hermes_agent",
        display_name: "Hermes Agent API Server",
        status: ProviderIntegrationStatus::ReadyViaOpenAiCompat,
        notes: "Hermes Agent exposes an OpenAI-compatible /v1/chat/completions endpoint.",
    },
    ProviderIntegrationCandidate {
        key: "pi_coding_agent",
        display_name: "Pi Coding Agent",
        status: ProviderIntegrationStatus::PlannedLocalCli,
        notes: "Pi exposes print/json/rpc CLI modes, so it should be a local CLI/RPC provider.",
    },
    ProviderIntegrationCandidate {
        key: "inflection_pi",
        display_name: "Inflection Pi",
        status: ProviderIntegrationStatus::PlannedNativeHttp,
        notes: "Inflection exposes Pi models through its own API shape; third-party gateways may be OpenAI-compatible.",
    },
    ProviderIntegrationCandidate {
        key: "openrouter",
        display_name: "OpenRouter",
        status: ProviderIntegrationStatus::ReadyViaOpenAiCompat,
        notes: "OpenRouter is OpenAI-compatible and routes to many model providers.",
    },
    ProviderIntegrationCandidate {
        key: "ollama",
        display_name: "Ollama",
        status: ProviderIntegrationStatus::ReadyViaOpenAiCompat,
        notes: "Ollama serves a local OpenAI-compatible endpoint under /v1.",
    },
    ProviderIntegrationCandidate {
        key: "lm_studio",
        display_name: "LM Studio",
        status: ProviderIntegrationStatus::ReadyViaOpenAiCompat,
        notes: "LM Studio serves a local OpenAI-compatible endpoint.",
    },
    ProviderIntegrationCandidate {
        key: "vllm",
        display_name: "vLLM",
        status: ProviderIntegrationStatus::ReadyViaOpenAiCompat,
        notes: "vLLM can serve OpenAI-compatible chat completions.",
    },
];

const SUBSCRIPTION_PROVIDER_BENCHMARKS: &[SubscriptionProviderBenchmark] = &[
    SubscriptionProviderBenchmark {
        product: "OpenClaw",
        route_key: "openai-codex",
        auth_surface: SubscriptionAuthSurface::ProductManagedOAuth,
        attach_path: RouterAttachPath::ExistingProvider("codex_local"),
        notes: "ChatGPT Plus/Pro Codex OAuth; OpenClaw stores/refreshes auth profiles.",
    },
    SubscriptionProviderBenchmark {
        product: "OpenClaw",
        route_key: "anthropic-claude-cli",
        auth_surface: SubscriptionAuthSurface::LocalCliAccount,
        attach_path: RouterAttachPath::ExistingProvider("claude_local"),
        notes: "Claude Pro/Max style CLI reuse; safest Kangnam path is the local Claude CLI provider.",
    },
    SubscriptionProviderBenchmark {
        product: "OpenClaw",
        route_key: "github-copilot",
        auth_surface: SubscriptionAuthSurface::ProductManagedOAuth,
        attach_path: RouterAttachPath::ExistingProvider("copilot_local"),
        notes: "Copilot subscription/device auth can reuse a local GitHub CLI token.",
    },
    SubscriptionProviderBenchmark {
        product: "OpenClaw",
        route_key: "gemini-cli-oauth",
        auth_surface: SubscriptionAuthSurface::LocalCliAccount,
        attach_path: RouterAttachPath::ExistingProvider("gemini_local"),
        notes: "Gemini CLI/OAuth-style route maps to the local Gemini provider surface.",
    },
    SubscriptionProviderBenchmark {
        product: "Hermes Agent",
        route_key: "nous-portal",
        auth_surface: SubscriptionAuthSurface::ProductManagedOAuth,
        attach_path: RouterAttachPath::OpenAiCompatiblePreset("hermes_subscription_proxy"),
        notes: "Hermes can proxy paid Nous Portal subscription credentials through a local OpenAI-compatible endpoint.",
    },
    SubscriptionProviderBenchmark {
        product: "Hermes Agent",
        route_key: "openai-codex",
        auth_surface: SubscriptionAuthSurface::ProductManagedOAuth,
        attach_path: RouterAttachPath::ExistingProvider("codex_local"),
        notes: "Hermes supports ChatGPT OAuth/Codex credentials and can import Codex CLI auth.",
    },
    SubscriptionProviderBenchmark {
        product: "Hermes Agent",
        route_key: "google-gemini-cli",
        auth_surface: SubscriptionAuthSurface::ProductManagedOAuth,
        attach_path: RouterAttachPath::ExistingProvider("gemini_local"),
        notes: "Hermes benchmarks Gemini OAuth as a subscription/free-tier route with explicit policy warnings.",
    },
    SubscriptionProviderBenchmark {
        product: "Hermes Agent",
        route_key: "xai-oauth",
        auth_surface: SubscriptionAuthSurface::ProductManagedOAuth,
        attach_path: RouterAttachPath::PlannedProvider("xai_oauth"),
        notes: "SuperGrok OAuth is a subscription route; Kangnam has no native xAI OAuth provider yet.",
    },
    SubscriptionProviderBenchmark {
        product: "Pi Coding Agent",
        route_key: "openai-codex",
        auth_surface: SubscriptionAuthSurface::ProductManagedOAuth,
        attach_path: RouterAttachPath::ExistingProvider("codex_local"),
        notes: "Pi supports ChatGPT Plus/Pro Codex subscription auth via /login.",
    },
    SubscriptionProviderBenchmark {
        product: "Pi Coding Agent",
        route_key: "claude-pro-max",
        auth_surface: SubscriptionAuthSurface::ProductManagedOAuth,
        attach_path: RouterAttachPath::ExistingProvider("claude_local"),
        notes: "Pi supports Claude Pro/Max subscription auth via /login.",
    },
    SubscriptionProviderBenchmark {
        product: "Pi Coding Agent",
        route_key: "github-copilot",
        auth_surface: SubscriptionAuthSurface::ProductManagedOAuth,
        attach_path: RouterAttachPath::ExistingProvider("copilot_local"),
        notes: "Pi supports GitHub Copilot subscription auth; Kangnam can attach through the local Copilot route.",
    },
    SubscriptionProviderBenchmark {
        product: "Pi Coding Agent",
        route_key: "pi-rpc",
        auth_surface: SubscriptionAuthSurface::ProductManagedOAuth,
        attach_path: RouterAttachPath::ExistingProvider("pi_local"),
        notes: "Pi itself can act as a subscription-auth broker through print/json CLI modes.",
    },
];

/// Returns researched candidates that can be exposed by host UIs.
pub fn provider_integration_candidates() -> &'static [ProviderIntegrationCandidate] {
    PROVIDER_INTEGRATION_CANDIDATES
}

/// Returns benchmarked subscription-backed provider routes from peer products.
pub fn subscription_provider_benchmarks() -> &'static [SubscriptionProviderBenchmark] {
    SUBSCRIPTION_PROVIDER_BENCHMARKS
}

/// Static configuration for one concrete provider instance.
///
/// `provider` is the built-in provider key, such as `"claude"`,
/// `"codex_local"`, `"openai_compat"`, or `"dummy"`. The router stores this
/// config under an alias of the host's choosing, so one application can expose
/// multiple configurations for the same provider key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConfig {
    pub provider: String,
    pub api_key: String,
    pub model: String,
    pub base_url: String,
}

impl ProviderConfig {
    /// Build a provider config from string-like values.
    pub fn new(
        provider: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            provider: provider.into(),
            api_key: api_key.into(),
            model: model.into(),
            base_url: base_url.into(),
        }
    }

    /// Instantiate the configured provider through the crate registry.
    pub fn create(&self) -> Result<Box<dyn LlmProviderDyn>, LlmError> {
        create_provider(&self.provider, &self.api_key, &self.model, &self.base_url)
    }

    /// Configure any endpoint that speaks the OpenAI Chat Completions API.
    pub fn openai_compatible(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self::new("openai_compat", api_key, model, base_url)
    }

    /// OpenRouter is OpenAI-compatible and routes to many model providers.
    pub fn openrouter(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self::openai_compatible(OPENROUTER_BASE_URL, api_key, model)
    }

    /// Local OpenClaw Gateway OpenAI-compatible surface.
    pub fn openclaw_gateway(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let model = model.into();
        let model = if model.is_empty() {
            OPENCLAW_DEFAULT_MODEL.to_string()
        } else {
            model
        };
        Self::openai_compatible(OPENCLAW_GATEWAY_BASE_URL, api_key, model)
    }

    /// Local Hermes Agent API server OpenAI-compatible surface.
    pub fn hermes_agent(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let model = model.into();
        let model = if model.is_empty() {
            HERMES_AGENT_DEFAULT_MODEL.to_string()
        } else {
            model
        };
        Self::openai_compatible(HERMES_AGENT_BASE_URL, api_key, model)
    }

    /// Local Hermes subscription proxy for raw model inference through a
    /// Hermes-managed subscription such as Nous Portal.
    pub fn hermes_subscription_proxy(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self::openai_compatible(HERMES_SUBSCRIPTION_PROXY_BASE_URL, api_key, model)
    }

    /// Local Ollama OpenAI-compatible surface.
    pub fn ollama(model: impl Into<String>) -> Self {
        Self::openai_compatible(OLLAMA_BASE_URL, "ollama", model)
    }

    /// Local LM Studio OpenAI-compatible surface.
    pub fn lm_studio(model: impl Into<String>) -> Self {
        Self::openai_compatible(LM_STUDIO_BASE_URL, "lm-studio", model)
    }

    /// Local or remote vLLM OpenAI-compatible surface.
    pub fn vllm(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self::openai_compatible(base_url, "", model)
    }

    /// Local Pi Coding Agent JSON-mode provider.
    ///
    /// `pi_provider` maps to Pi's `--provider` option and may be empty when
    /// the model already includes a provider prefix.
    pub fn pi_local(pi_provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self::new("pi_local", "", model, pi_provider)
    }

    /// Local GitHub Copilot subscription route backed by `gh auth token`.
    pub fn copilot_local(model: impl Into<String>) -> Self {
        Self::new("copilot_local", "", model, "")
    }
}

/// A normalized request that can be routed to any registered provider.
#[derive(Debug, Clone)]
pub struct RouteRequest {
    pub provider: Option<String>,
    pub system_prompt: String,
    pub messages: Vec<ChatMessage>,
    pub options: LlmRequestOptions,
    pub result_json: Value,
}

impl RouteRequest {
    /// Create a chat request with no system prompt and default options.
    pub fn chat(messages: Vec<ChatMessage>) -> Self {
        Self {
            provider: None,
            system_prompt: String::new(),
            messages,
            options: LlmRequestOptions::default(),
            result_json: Value::Null,
        }
    }

    /// Route this request to a named provider alias.
    pub fn to_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    /// Set the system prompt sent with this request.
    pub fn with_system_prompt(mut self, system_prompt: impl Into<String>) -> Self {
        self.system_prompt = system_prompt.into();
        self
    }

    /// Set typed per-request options.
    pub fn with_options(mut self, options: LlmRequestOptions) -> Self {
        self.options = options;
        self
    }

    /// Set provider-specific compatibility JSON for legacy call sites.
    pub fn with_result_json(mut self, result_json: Value) -> Self {
        self.result_json = result_json;
        self
    }
}

/// A normalized single-prompt request.
#[derive(Debug, Clone)]
pub struct RenderRequest {
    pub provider: Option<String>,
    pub system_prompt: String,
    pub user_input: String,
    pub result_json: Value,
}

impl RenderRequest {
    /// Create a render request with no system prompt.
    pub fn new(user_input: impl Into<String>) -> Self {
        Self {
            provider: None,
            system_prompt: String::new(),
            user_input: user_input.into(),
            result_json: Value::Null,
        }
    }

    /// Route this request to a named provider alias.
    pub fn to_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    /// Set the system prompt sent with this request.
    pub fn with_system_prompt(mut self, system_prompt: impl Into<String>) -> Self {
        self.system_prompt = system_prompt.into();
        self
    }

    /// Set provider-specific compatibility JSON for legacy call sites.
    pub fn with_result_json(mut self, result_json: Value) -> Self {
        self.result_json = result_json;
        self
    }
}

/// Multi-provider router with a single common request API.
#[derive(Debug, Clone, Default)]
pub struct Router {
    providers: HashMap<String, ProviderConfig>,
    default_provider: Option<String>,
}

impl Router {
    /// Create an empty router.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a provider under an alias.
    ///
    /// The first provider registered becomes the default unless a default was
    /// set explicitly with [`Self::set_default_provider`].
    pub fn register_provider(
        &mut self,
        alias: impl Into<String>,
        config: ProviderConfig,
    ) -> &mut Self {
        let alias = alias.into();
        if self.default_provider.is_none() {
            self.default_provider = Some(alias.clone());
        }
        self.providers.insert(alias, config);
        self
    }

    /// Builder-style registration helper.
    pub fn with_provider(mut self, alias: impl Into<String>, config: ProviderConfig) -> Self {
        self.register_provider(alias, config);
        self
    }

    /// Set the default provider alias used when a request does not choose one.
    ///
    /// The alias may be registered later, but calls will fail until it exists.
    pub fn set_default_provider(&mut self, alias: impl Into<String>) -> &mut Self {
        self.default_provider = Some(alias.into());
        self
    }

    /// Builder-style default provider helper.
    pub fn with_default_provider(mut self, alias: impl Into<String>) -> Self {
        self.set_default_provider(alias);
        self
    }

    /// Return a provider config by alias.
    pub fn provider_config(&self, alias: &str) -> Option<&ProviderConfig> {
        self.providers.get(alias)
    }

    /// List registered aliases in stable alphabetical order.
    pub fn provider_aliases(&self) -> Vec<&str> {
        let mut aliases: Vec<_> = self.providers.keys().map(String::as_str).collect();
        aliases.sort_unstable();
        aliases
    }

    /// Instantiate a provider by alias.
    pub fn create_provider(&self, alias: &str) -> Result<Box<dyn LlmProviderDyn>, LlmError> {
        self.providers
            .get(alias)
            .ok_or_else(|| missing_provider(alias))?
            .create()
    }

    /// Route one single-prompt request and return the completed response.
    pub async fn render(&self, request: RenderRequest) -> Result<LlmResponse, LlmError> {
        let alias = self.resolve_alias(request.provider.as_deref())?;
        let provider = self.create_provider(&alias)?;
        provider
            .render_dyn(
                &request.system_prompt,
                &request.user_input,
                &request.result_json,
            )
            .await
    }

    /// Route one request and return the completed response.
    pub async fn chat(&self, request: RouteRequest) -> Result<LlmResponse, LlmError> {
        let alias = self.resolve_alias(request.provider.as_deref())?;
        let provider = self.create_provider(&alias)?;
        provider
            .chat_with_options_dyn(
                &request.system_prompt,
                &request.messages,
                &request.options,
                &request.result_json,
            )
            .await
    }

    /// Route one request as a normalized stream.
    ///
    /// This stream owns the provider instance it creates, so callers can hold
    /// the stream without borrowing the router.
    pub fn chat_stream(&self, request: RouteRequest) -> BoxStream<'static, LlmStreamEvent> {
        let alias = match self.resolve_alias(request.provider.as_deref()) {
            Ok(alias) => alias,
            Err(err) => {
                return Box::pin(async_stream::stream! {
                    yield LlmStreamEvent::Error { message: err.to_string() };
                });
            }
        };
        let config = match self.providers.get(&alias).cloned() {
            Some(config) => config,
            None => {
                let err = missing_provider(&alias);
                return Box::pin(async_stream::stream! {
                    yield LlmStreamEvent::Error { message: err.to_string() };
                });
            }
        };

        Box::pin(async_stream::stream! {
            match config.create() {
                Ok(provider) => {
                    let mut stream = provider.chat_stream_with_options_dyn(
                        &request.system_prompt,
                        &request.messages,
                        &request.options,
                        &request.result_json,
                    );
                    while let Some(event) = futures::StreamExt::next(&mut stream).await {
                        yield event;
                    }
                }
                Err(err) => {
                    yield LlmStreamEvent::Error { message: err.to_string() };
                }
            }
        })
    }

    fn resolve_alias(&self, requested: Option<&str>) -> Result<String, LlmError> {
        match requested.or(self.default_provider.as_deref()) {
            Some(alias) if self.providers.contains_key(alias) => Ok(alias.to_string()),
            Some(alias) => Err(missing_provider(alias)),
            None => Err(LlmError::MissingConfig {
                provider: "router".into(),
                reason: "no provider requested and no default provider configured".into(),
            }),
        }
    }
}

fn missing_provider(alias: &str) -> LlmError {
    LlmError::MissingConfig {
        provider: alias.to_string(),
        reason: "provider alias is not registered in router".into(),
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;

    fn dummy_config() -> ProviderConfig {
        ProviderConfig::new("dummy", "", "", "")
    }

    #[tokio::test]
    async fn chat_routes_to_default_provider() {
        let router = Router::new().with_provider("default", dummy_config());
        let request = RouteRequest::chat(vec![ChatMessage::user("hello")])
            .with_result_json(serde_json::json!({"advice": "default route ok"}));

        let response = router.chat(request).await.unwrap();

        assert_eq!(response.model, "dummy-v1");
        assert!(response.rendered_text.contains("default route ok"));
    }

    #[tokio::test]
    async fn render_routes_to_named_provider_alias() {
        let router = Router::new()
            .with_provider("default", dummy_config())
            .with_provider("fortune", dummy_config());
        let request = RenderRequest::new("hello")
            .to_provider("fortune")
            .with_result_json(serde_json::json!({"overall": {"score": 91}}));

        let response = router.render(request).await.unwrap();

        assert_eq!(response.model, "dummy-v1");
        assert!(response.rendered_text.contains("91점"));
    }

    #[tokio::test]
    async fn chat_routes_to_named_provider_alias() {
        let router = Router::new()
            .with_provider("slow", dummy_config())
            .with_provider("fast", dummy_config())
            .with_default_provider("slow");
        let request = RouteRequest::chat(vec![ChatMessage::user("pick fast")])
            .to_provider("fast")
            .with_result_json(serde_json::json!({"advice": "named route ok"}));

        let response = router.chat(request).await.unwrap();

        assert_eq!(response.model, "dummy-v1");
        assert!(response.rendered_text.contains("named route ok"));
    }

    #[tokio::test]
    async fn missing_provider_alias_is_an_error() {
        let router = Router::new().with_provider("default", dummy_config());
        let request = RouteRequest::chat(vec![ChatMessage::user("hello")]).to_provider("missing");

        let error = router.chat(request).await.unwrap_err();

        assert!(matches!(error, LlmError::MissingConfig { provider, .. } if provider == "missing"));
    }

    #[tokio::test]
    async fn stream_owns_provider_and_yields_terminal_event() {
        let router = Router::new().with_provider("default", dummy_config());
        let request = RouteRequest::chat(vec![ChatMessage::user("stream hello")]);

        let mut stream = router.chat_stream(request);
        let mut saw_end = false;
        while let Some(event) = stream.next().await {
            if matches!(event, LlmStreamEvent::End { .. }) {
                saw_end = true;
            }
        }

        assert!(saw_end);
    }

    #[tokio::test]
    async fn router_chat_contract_returns_stable_response_shape() {
        let router = Router::new().with_provider("default", dummy_config());
        let request = RouteRequest::chat(vec![ChatMessage::user("contract")]);

        let response = router.chat(request).await.unwrap();

        assert!(!response.rendered_text.is_empty());
        assert!(!response.model.is_empty());
        assert_eq!(response.estimated_cost_usd, 0.0);
        assert_eq!(response.input_tokens, None);
        assert_eq!(response.output_tokens, None);
        assert!(response.tool_calls.is_empty());
        assert!(response.thinking_text.is_none());
    }

    #[tokio::test]
    async fn router_stream_contract_emits_single_terminal_end_on_success() {
        let router = Router::new().with_provider("default", dummy_config());
        let request = RouteRequest::chat(vec![ChatMessage::user("contract stream")]);

        let mut stream = router.chat_stream(request);
        let mut end_count = 0usize;
        let mut saw_delta = false;
        while let Some(event) = stream.next().await {
            match event {
                LlmStreamEvent::Delta { text } => {
                    assert!(!text.is_empty());
                    saw_delta = true;
                }
                LlmStreamEvent::End { total } => {
                    end_count += 1;
                    assert!(!total.rendered_text.is_empty());
                    assert!(!total.model.is_empty());
                }
                LlmStreamEvent::Error { message } => panic!("unexpected stream error: {message}"),
                _ => {}
            }
        }

        assert!(saw_delta);
        assert_eq!(end_count, 1);
    }

    #[tokio::test]
    async fn router_stream_contract_reports_factory_failure_as_error_event() {
        let router = Router::new().with_default_provider("missing");
        let request = RouteRequest::chat(vec![ChatMessage::user("contract stream error")]);

        let events: Vec<_> = router.chat_stream(request).collect().await;

        assert_eq!(events.len(), 1);
        assert!(
            matches!(events.as_slice(), [LlmStreamEvent::Error { message }] if message.contains("missing"))
        );
    }

    #[test]
    fn provider_aliases_are_sorted() {
        let router = Router::new()
            .with_provider("z", dummy_config())
            .with_provider("a", dummy_config());

        assert_eq!(router.provider_aliases(), vec!["a", "z"]);
    }

    #[test]
    fn openai_compatible_presets_use_router_provider_key() {
        let cases = [
            ProviderConfig::openrouter("sk-or-test", "anthropic/claude-sonnet-latest"),
            ProviderConfig::openclaw_gateway("local-token", ""),
            ProviderConfig::hermes_agent("local-token", ""),
            ProviderConfig::hermes_subscription_proxy("unused", "Hermes-4-70B"),
            ProviderConfig::ollama("llama3.1:8b"),
            ProviderConfig::lm_studio("local-model"),
            ProviderConfig::vllm("http://127.0.0.1:8000/v1", "Qwen/Qwen3"),
        ];

        for config in cases {
            assert_eq!(config.provider, "openai_compat");
        }
    }

    #[test]
    fn openclaw_gateway_preset_uses_documented_local_defaults() {
        let config = ProviderConfig::openclaw_gateway("token", "");

        assert_eq!(config.provider, "openai_compat");
        assert_eq!(config.api_key, "token");
        assert_eq!(config.model, OPENCLAW_DEFAULT_MODEL);
        assert_eq!(config.base_url, OPENCLAW_GATEWAY_BASE_URL);
    }

    #[test]
    fn hermes_agent_preset_uses_documented_local_defaults() {
        let config = ProviderConfig::hermes_agent("token", "");

        assert_eq!(config.provider, "openai_compat");
        assert_eq!(config.api_key, "token");
        assert_eq!(config.model, HERMES_AGENT_DEFAULT_MODEL);
        assert_eq!(config.base_url, HERMES_AGENT_BASE_URL);
    }

    #[test]
    fn hermes_subscription_proxy_preset_uses_documented_local_defaults() {
        let config = ProviderConfig::hermes_subscription_proxy("sk-unused", "Hermes-4-70B");

        assert_eq!(config.provider, "openai_compat");
        assert_eq!(config.api_key, "sk-unused");
        assert_eq!(config.model, "Hermes-4-70B");
        assert_eq!(config.base_url, HERMES_SUBSCRIPTION_PROXY_BASE_URL);
    }

    #[test]
    fn local_openai_compatible_presets_use_expected_base_urls() {
        let ollama = ProviderConfig::ollama("qwen2.5-coder:7b");
        let lm_studio = ProviderConfig::lm_studio("local-model");
        let vllm = ProviderConfig::vllm("http://localhost:8000/v1", "local-model");

        assert_eq!(ollama.base_url, OLLAMA_BASE_URL);
        assert_eq!(ollama.api_key, "ollama");
        assert_eq!(lm_studio.base_url, LM_STUDIO_BASE_URL);
        assert_eq!(lm_studio.api_key, "lm-studio");
        assert_eq!(vllm.base_url, "http://localhost:8000/v1");
        assert!(vllm.api_key.is_empty());
    }

    #[test]
    fn pi_local_preset_uses_local_provider_key_and_provider_hint() {
        let config = ProviderConfig::pi_local("openai", "openai/gpt-5");

        assert_eq!(config.provider, "pi_local");
        assert_eq!(config.model, "openai/gpt-5");
        assert_eq!(config.base_url, "openai");
    }

    #[test]
    fn copilot_local_preset_uses_local_provider_key() {
        let config = ProviderConfig::copilot_local("claude-sonnet-4.6");

        assert_eq!(config.provider, "copilot_local");
        assert_eq!(config.model, "claude-sonnet-4.6");
    }

    #[test]
    fn researched_gateway_candidates_are_ready_via_openai_compat() {
        let ready: Vec<_> = provider_integration_candidates()
            .iter()
            .filter(|candidate| candidate.status == ProviderIntegrationStatus::ReadyViaOpenAiCompat)
            .map(|candidate| candidate.key)
            .collect();

        assert!(ready.contains(&"openclaw_gateway"));
        assert!(ready.contains(&"hermes_agent"));
        assert!(ready.contains(&"openrouter"));
        assert!(ready.contains(&"ollama"));
        assert!(ready.contains(&"lm_studio"));
        assert!(ready.contains(&"vllm"));
    }

    #[test]
    fn pi_candidates_are_not_misclassified_as_openai_compatible() {
        let candidates = provider_integration_candidates();
        let pi_coding = candidates
            .iter()
            .find(|candidate| candidate.key == "pi_coding_agent")
            .unwrap();
        let inflection = candidates
            .iter()
            .find(|candidate| candidate.key == "inflection_pi")
            .unwrap();

        assert_eq!(pi_coding.status, ProviderIntegrationStatus::PlannedLocalCli);
        assert_eq!(
            inflection.status,
            ProviderIntegrationStatus::PlannedNativeHttp
        );
    }

    #[test]
    fn subscription_benchmarks_cover_peer_product_oauth_routes() {
        let rows = subscription_provider_benchmarks();
        let has = |product: &str, route_key: &str| {
            rows.iter()
                .any(|row| row.product == product && row.route_key == route_key)
        };

        assert!(has("OpenClaw", "openai-codex"));
        assert!(has("OpenClaw", "anthropic-claude-cli"));
        assert!(has("OpenClaw", "github-copilot"));
        assert!(has("Hermes Agent", "nous-portal"));
        assert!(has("Hermes Agent", "google-gemini-cli"));
        assert!(has("Pi Coding Agent", "openai-codex"));
        assert!(has("Pi Coding Agent", "claude-pro-max"));
        assert!(has("Pi Coding Agent", "github-copilot"));
    }

    #[test]
    fn subscription_benchmarks_map_existing_local_subscription_routes() {
        let existing: Vec<_> = subscription_provider_benchmarks()
            .iter()
            .filter_map(|row| match row.attach_path {
                RouterAttachPath::ExistingProvider(provider) => Some((row.route_key, provider)),
                _ => None,
            })
            .collect();

        assert!(existing.contains(&("openai-codex", "codex_local")));
        assert!(existing.contains(&("anthropic-claude-cli", "claude_local")));
        assert!(existing.contains(&("google-gemini-cli", "gemini_local")));
    }

    #[test]
    fn copilot_and_pi_subscription_routes_are_no_longer_followups() {
        let followups: Vec<_> = subscription_provider_benchmarks()
            .iter()
            .filter_map(|row| match row.attach_path {
                RouterAttachPath::PlannedProvider(provider) => Some((row.route_key, provider)),
                _ => None,
            })
            .collect();

        assert!(!followups.contains(&("github-copilot", "copilot_local_or_oauth")));
        assert!(!followups.contains(&("pi-rpc", "pi_local")));
    }

    #[test]
    fn pi_subscription_broker_now_maps_to_existing_provider() {
        let pi = subscription_provider_benchmarks()
            .iter()
            .find(|row| row.product == "Pi Coding Agent" && row.route_key == "pi-rpc")
            .unwrap();

        assert_eq!(
            pi.attach_path,
            RouterAttachPath::ExistingProvider("pi_local")
        );
    }

    #[test]
    fn copilot_subscription_routes_now_map_to_existing_provider() {
        let rows = subscription_provider_benchmarks();
        let copilot_rows: Vec<_> = rows
            .iter()
            .filter(|row| row.route_key == "github-copilot")
            .collect();

        assert!(!copilot_rows.is_empty());
        assert!(
            copilot_rows.iter().all(|row| {
                row.attach_path == RouterAttachPath::ExistingProvider("copilot_local")
            })
        );
    }
}
