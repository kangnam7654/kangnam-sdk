# llm-router

[![CI](https://github.com/kangnam7654/llm-router/actions/workflows/ci.yml/badge.svg)](https://github.com/kangnam7654/llm-router/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Latest release](https://img.shields.io/github/v/tag/kangnam7654/llm-router?label=release&sort=semver)](https://github.com/kangnam7654/llm-router/releases)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)

Multi-provider LLM client for Rust — one router facade, one object-safe provider trait, built-in HTTP/local/subscription providers, typed per-request options, token-aware streaming.

## Purpose

Library for calling LLM backends through one common API. Register concrete provider configs in `Router`, submit a `RenderRequest` or `RouteRequest`, and receive normalized `LlmResponse` / `LlmStreamEvent` output. Lower-level call sites can still construct one provider directly through `LlmProviderDyn`. Supports:

- Multi-provider routing with aliases and a default provider via `Router`.
- Single-prompt routing via `RenderRequest`.
- Single-shot `render_dyn`, multi-turn `chat_dyn`, streaming `chat_stream_dyn`.
- Per-request options (image attachments, working directory, web search, reasoning effort, turn limits) via `chat_with_options_dyn` / `chat_stream_with_options_dyn`.
- Incremental `Delta { text }` events from local CLI providers as tokens arrive.
- Token usage (`input_tokens`, `output_tokens`) and cost estimates on `LlmResponse`.
- Model discovery via `list_models` for providers that expose it.

A `dummy` provider is included for tests and examples that require no credentials.

## Installation

```toml
[dependencies]
llm-router = { git = "https://github.com/kangnam7654/llm-router", tag = "v0.5.1" }
```

Requires Rust 1.85+ (edition 2024) and a tokio runtime.

## Built-in Providers

| Key             | Backend                              | Notes                                     |
| :-------------- | :----------------------------------- | :---------------------------------------- |
| `claude`        | Anthropic Messages API (HTTP)        | Requires `ANTHROPIC_API_KEY`.             |
| `claude_local`  | `claude` CLI                         | Streaming deltas, honors CLI options.     |
| `codex`         | OpenAI Chat Completions (HTTP)       | Requires `OPENAI_API_KEY`.                |
| `codex_local`   | `codex` CLI                          | Streaming deltas, honors CLI options.     |
| `copilot`       | GitHub Copilot Chat (HTTP)           | Requires a GitHub Copilot token.          |
| `copilot_local` | GitHub Copilot via `gh` auth          | Uses local GitHub CLI login.              |
| `gemini`        | Google Generative Language (HTTP)    | Requires `GEMINI_API_KEY`.                |
| `gemini_local`  | `gemini` CLI                         | Streaming deltas, honors CLI options.     |
| `pi_local`      | Pi Coding Agent CLI                  | Uses Pi subscription `/login` state.      |
| `antigravity`   | Alias for `gemini`                   | Legacy-config compatibility.              |
| `openai_compat` | Any OpenAI-compatible HTTP endpoint  | Requires `base_url`; streams with usage.  |
| `dummy`         | Offline echo provider                | Tests and examples, no credentials.       |

Unknown provider keys fall back to `dummy`.

### Provider Capabilities

The router normalizes provider features into a queryable `ProviderCapabilities` struct.

| Key | Tool Calling | Streaming | Usage/Cost | Vision | Reasoning Effort | Model List | Local Read | Web Search |
|---|---|---|---|---|---|---|---|---|
| `claude` | Yes | Yes (Streaming) | Yes / Yes | Yes | No | Yes | No | No |
| `codex` | Yes | Yes (FinalOnly) | Yes / Yes | Yes | Yes | Yes | No | No |
| `gemini` | Yes | Yes (Streaming) | Yes / No | Yes | No | Yes | No | Yes |
| `openai_compat` | Yes | Yes (FinalOnly) | Yes / No | Yes | No | Yes | No | No |
| `copilot` | No | Yes (None) | No / No | No | No | No | No | No |
| `*_local` (CLIs)| No | Yes (varies) | varies | No | No | No | Yes | No |

## Quick Start

```rust
use llm_router::{ChatMessage, ProviderConfig, RouteRequest, Router};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let router = Router::new().with_provider(
        "default",
        ProviderConfig::new("dummy", "", "", ""),
    );

    let request = RouteRequest::chat(vec![ChatMessage::user("hello")])
        .with_system_prompt("you are a helpful assistant");
    let resp = router.chat(request).await?;

    println!("{}", resp.rendered_text);
    if let (Some(ip), Some(op)) = (resp.input_tokens, resp.output_tokens) {
        println!("tokens: in={ip} out={op}");
    }
    Ok(())
}
```

### OpenAI-Compatible Gateways

Several agent gateways and local runtimes do not need a new native provider;
they can be registered through `ProviderConfig` presets:

```rust
use llm_router::{ProviderConfig, Router};

let router = Router::new()
    .with_provider("pi", ProviderConfig::pi_local("openai", "openai/gpt-5"))
    .with_provider("copilot", ProviderConfig::copilot_local("claude-sonnet-4.6"))
    .with_provider("openclaw", ProviderConfig::openclaw_gateway("local-token", ""))
    .with_provider("hermes", ProviderConfig::hermes_agent("local-token", ""))
    .with_provider(
        "nous-via-hermes",
        ProviderConfig::hermes_subscription_proxy("sk-unused", "Hermes-4-70B"),
    )
    .with_provider(
        "openrouter",
        ProviderConfig::openrouter("sk-or-...", "anthropic/claude-sonnet-latest"),
    )
    .with_provider("ollama", ProviderConfig::ollama("llama3.1:8b"))
    .with_provider("lmstudio", ProviderConfig::lm_studio("local-model"))
    .with_provider(
        "vllm",
        ProviderConfig::vllm("http://127.0.0.1:8000/v1", "Qwen/Qwen3"),
    );
```

`provider_integration_candidates()` exposes the researched attach path for
these gateways plus planned native/CLI follow-ups such as Pi.
`subscription_provider_benchmarks()` separately tracks subscription/OAuth
routes benchmarked from OpenClaw, Hermes, and Pi.

## Streaming with Delta Events

`chat_stream_dyn` (and `chat_stream_with_options_dyn`) emit `LlmStreamEvent::Delta { text }` as tokens arrive from the provider, followed by a terminal `End { total }` carrying the full `LlmResponse`. The `_local` CLI providers emit incremental deltas; HTTP providers emit a single Delta + End by default.

```rust
use futures::StreamExt;
use llm_router::{create_provider, ChatMessage, LlmStreamEvent};
use serde_json::json;

let provider = create_provider("claude_local", "", "sonnet", "")?;
let messages = vec![ChatMessage::user("stream me something")];
let mut stream = provider.chat_stream_dyn("", &messages, &json!({}));
while let Some(event) = stream.next().await {
    match event {
        LlmStreamEvent::Delta { text } => print!("{text}"),
        LlmStreamEvent::End { total }  => println!("\n[done; model={}]", total.model),
        LlmStreamEvent::Error { message } => eprintln!("[error] {message}"),
        LlmStreamEvent::ToolCall { call } => println!("\n[tool_call] {} {}", call.name, call.arguments),
        _ => {}
    }
}
```

Consumers that only read `End` continue to work — `End.total.rendered_text` holds the full accumulated text.

## Tool Calling

HTTP providers (`claude`, `codex`, `gemini`, `openai_compat`; `copilot` experimental)
support native tool/function calling.

```rust
use llm_router::{ChatMessage, LlmRequestOptions, ToolDef, ToolChoice};
use serde_json::json;

let opts = LlmRequestOptions {
    tools: vec![ToolDef {
        name: "get_weather".into(),
        description: "Get current weather".into(),
        input_schema: json!({
            "type": "object",
            "properties": {"city": {"type": "string"}},
            "required": ["city"]
        }),
    }],
    tool_choice: Some(ToolChoice::Auto),
    ..Default::default()
};

let messages = vec![ChatMessage::user("Weather in Seoul?")];
let resp = provider.chat_with_options_dyn("", &messages, &opts, &json!({})).await?;
for call in &resp.tool_calls {
    let result = my_tool_executor(&call.name, &call.arguments);
    // Continue with ChatMessage::tool_result(&call.id, &result, false)
}
```

llm-router does NOT execute tools — caller receives `ToolCall`, executes, and
sends `ChatContent::ToolResult` back via `ChatMessage::tool_result(...)`.

## Multimodal Input

HTTP providers support image content via `ChatContent::Image`.

```rust
use llm_router::{ChatContent, ChatMessage, ImageSource};

// From file path (auto-detects MIME):
let img = ChatContent::image_from_path("/tmp/photo.png")?;

// From base64 with auto MIME detection:
let img = ChatContent::image_from_base64_auto(base64_data)?;

// Explicit:
let img = ChatContent::Image {
    source: ImageSource::Base64(data),
    mime_type: "image/png".into(),
};

let messages = vec![ChatMessage::user_with_image("describe", img)];
```

Gemini does NOT support `ImageSource::Url` (use Files API); other providers do.

## Per-Request Options

`LlmRequestOptions` carries typed per-call knobs. Providers honor the subset they support; unsupported fields are silently ignored.

```rust
use llm_router::{create_provider, ChatMessage, LlmRequestOptions};
use serde_json::json;
use std::path::PathBuf;
use std::time::Duration;

let provider = create_provider("codex_local", "", "gpt-5", "")?;
let opts = LlmRequestOptions {
    image_paths: vec![PathBuf::from("/tmp/diagram.png")],
    working_dir: Some(PathBuf::from("/tmp/project")),
    allow_web_search: true,
    reasoning_effort: Some("high".into()),
    max_turns: Some(4),
    timeout: Some(Duration::from_secs(180)),  // NEW in v0.3.1
    ..Default::default()
};
let messages = vec![ChatMessage::user("analyze")];
let resp = provider
    .chat_with_options_dyn("", &messages, &opts, &json!({}))
    .await?;
```

| Field              | claude_local        | codex_local                          | gemini_local            |
| :----------------- | :------------------ | :----------------------------------- | :---------------------- |
| `image_paths`      | `--image <path>`    | `--image <path>`                     | `@<abs_path>` in prompt |
| `working_dir`      | `--add-dir <path>`  | `-C <path>`                          | `--include-directories` |
| `allow_web_search` | `WebSearch,WebFetch` via `--allowedTools` | `--search`         | ignored                 |
| `allow_local_read` | `Read` via `--allowedTools` | implicit (sandbox)           | implicit (`@path`)      |
| `max_turns`        | `--max-turns <n>`   | ignored                              | ignored                 |
| `reasoning_effort` | ignored             | `-c model_reasoning_effort="<v>"`    | ignored                 |

**`timeout: Option<Duration>`** — per-call total deadline. Honored by all 8 providers: HTTP via `RequestBuilder::timeout`, local CLI via routing through the streaming path (`tokio::process::Command` + `kill_on_drop`).

**Sampling params (`temperature`, `max_tokens`, `top_p`, `stop_sequences`)** — honored by HTTP providers only; local CLI silently ignores. `claude` falls back to `MAX_TOKENS = 1024` when `max_tokens` is `None`; `gemini` falls back to its hardcoded temperature and token defaults.

## Model Discovery

```rust
use llm_router::list_models;

let models = list_models("gemini", "YOUR_API_KEY").await?;
for m in models {
    println!("{} — {}", m.name, m.display_name);
}
```

`list_models_with_base_url` is the same but takes an explicit `base_url` (required for `openai_compat`). Providers without a listing API return an empty vec.

## API Overview

- `Router` — high-level facade that registers provider configs and routes normalized requests.
- `ProviderConfig { provider, api_key, model, base_url }` — one concrete provider configuration.
- `RenderRequest { provider, system_prompt, user_input, result_json }` — common single-prompt request envelope used by `Router`.
- `RouteRequest { provider, system_prompt, messages, options, result_json }` — common chat request envelope used by `Router`.
- `provider_integration_candidates()` — researched attach paths for gateways/agents such as OpenClaw, Hermes, Pi, OpenRouter, Ollama, LM Studio, and vLLM.
- `subscription_provider_benchmarks()` — benchmarked subscription/OAuth routes such as ChatGPT Codex, Claude Pro/Max, GitHub Copilot, Gemini OAuth, and Nous Portal proxy paths.
- `LlmProviderDyn` — object-safe trait every provider implements.
- `ChatMessage` — one turn of conversation; multipart `Vec<ChatContent>` content. Constructed via `::user`, `::assistant`, `::user_with_image`, `::user_multipart`, `::tool_result`.
- `ChatContent::{Text, Image, ToolUse, ToolResult}` — multipart content blocks (`#[non_exhaustive]`).
- `ImageSource::{Base64, Url}` — image payload source (`#[non_exhaustive]`).
- `ToolDef { name, description, input_schema }` — tool advertised to the model.
- `ToolCall { id, name, arguments }` — tool invocation emitted by the model.
- `ToolChoice::{Auto, Any, None, Tool { name }}` — caller's tool-selection policy.
- `LlmResponse { rendered_text, model, estimated_cost_usd, input_tokens, output_tokens, tool_calls }` — provider output.
- `LlmStreamEvent::{Delta, End, Error, ToolCall}` — streaming event enum (`#[non_exhaustive]`).
- `LlmRequestOptions { image_paths, working_dir, allow_web_search, allow_local_read, max_turns, reasoning_effort, timeout, temperature, max_tokens, top_p, stop_sequences, tools, tool_choice }` — typed per-call options.
- `ListModel { name, display_name, description, input_token_limit, output_token_limit }` — model-catalog row.
- `create_provider(provider, api_key, model, base_url) -> Result<Box<dyn LlmProviderDyn>, LlmError>` — factory.
- `list_models(provider, api_key) -> Result<Vec<ListModel>, LlmError>` — default-base-url listing.
- `list_models_with_base_url(provider, api_key, base_url)` — explicit base URL.
- `registered_providers() -> Vec<&'static str>` — registered provider keys.

## Examples

See `examples/`:

- `cargo run --example minimal` — round-trips a single turn through the `dummy` provider.

## Stability

v0.x — API may change between minor versions. See [CHANGELOG.md](CHANGELOG.md) for migration notes. v1.0 will commit to semver.

## Contributing

Contributions welcome — bug reports, feature requests, and PRs.

### Bug reports / feature requests

Use the [GitHub issue templates](https://github.com/kangnam7654/llm-router/issues/new/choose). For bugs, please include a minimal Rust snippet that reproduces the issue and the version (or git SHA) you're on.

### Pull requests

1. Fork + branch from `main`.
2. Match existing code style:
   - `cargo fmt --all` before committing
   - `cargo clippy --all-targets -- -D warnings` clean
   - `cargo test --lib` passes. Tests requiring external resources (e.g. a live `codex` CLI installation) are marked with `#[ignore]` and skipped by default; CI does not run them. Use `cargo test --lib -- --include-ignored` locally if you have the CLI and want to exercise them.
3. For new features, follow the **Design-First** pattern used in this repo:
   - Spec doc under `docs/specs/YYYY-MM-DD-<feature>.md` describing API, file changes, and decisions.
   - Implementation plan under `docs/plans/YYYY-MM-DD-<feature>.plan.md` with task breakdown.
   - PR description links both.
   - See [`docs/specs/2026-04-27-thinking-caching-streaming-usage.md`](docs/specs/2026-04-27-thinking-caching-streaming-usage.md) for an example.
4. Update `CHANGELOG.md` under `[Unreleased]` with your change.
5. If your change is breaking, add an entry to `docs/migration/v<NEXT>.md`.

### Provider-specific contributions

When adding a new provider:
- New file under `src/<provider>.rs` mirroring the structure of `src/dummy.rs` (smallest reference impl).
- Implement `LlmProviderDyn` trait — only methods you support; defaults forward sensibly.
- Add to the registry in `src/lib.rs::REGISTRY`.
- Tests use [`wiremock`](https://crates.io/crates/wiremock) for HTTP mocking; no real API credentials in CI.

### Reporting security issues

Don't open a public issue for security vulnerabilities. Email the maintainer or use [GitHub Security Advisories](https://github.com/kangnam7654/llm-router/security/advisories/new).

## License

MIT. See [LICENSE](LICENSE).
