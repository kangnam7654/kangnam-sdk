---
created: 2026-05-09
type: design
status: implemented
project: kangnam-sdk
---
# AI Consult via Harness

## Purpose

Build a reusable SDK-level AI consult engine that moves Lunawave's current in-backend AI advisor logic into `kangnam-sdk`, using `kangnam-harness-runtime` tools and `kangnam-harness-llm-bridge` as the agent/tool-calling loop.

Done when:
- `kangnam-sdk` exposes an `ai-consult` crate that can answer one Korean consult turn from history + user message + optional birth profile.
- The consult agent can call harness tools for saju context and tarot draws instead of hardcoding all fortune context into the prompt.
- Guard behavior from Lunawave is preserved: max turn limit, max message length, safety blocklist, PII redaction, history truncation, and provider alternation normalization.
- Unit tests cover guard paths, persona/context prompt construction, report parsing, saju tool output, tarot tool output, and an LLM tool-call loop with `MockLlmProvider`.

## Scope

This pass implements the SDK crate only. Lunawave backend integration is a follow-up after this crate is green.

No new DB schema is required. The SDK owns domain logic and pure Rust tools; host apps remain responsible for auth, billing, WebSocket transport, persistence, and provider token loading.

## File changes

| Path | Change |
|---|---|
| `/Users/kangnam/projects/kangnam-sdk/Cargo.toml` | Add workspace member `crates/fortune/ai-consult` and workspace dependency `ai-consult = { path = "crates/fortune/ai-consult" }`. |
| `/Users/kangnam/projects/kangnam-sdk/crates/fortune/ai-consult/Cargo.toml` | New package `ai-consult`, edition/rust-version inherited from workspace. Depends on harness runtime, harness llm bridge, router, saju engine, tarot engine, serde, serde_json, thiserror, futures, async-trait. |
| `/Users/kangnam/projects/kangnam-sdk/crates/fortune/ai-consult/src/lib.rs` | Public API exports and module wiring. |
| `/Users/kangnam/projects/kangnam-sdk/crates/fortune/ai-consult/src/agent.rs` | `AiConsultSession` request/response orchestration over `LlmAgent<ConsultCapabilities>`. |
| `/Users/kangnam/projects/kangnam-sdk/crates/fortune/ai-consult/src/types.rs` | Request, response, history, birth profile, config, and error types. |
| `/Users/kangnam/projects/kangnam-sdk/crates/fortune/ai-consult/src/guard.rs` | Safety blocklist, PII redaction, max length/turn checks, history normalization. |
| `/Users/kangnam/projects/kangnam-sdk/crates/fortune/ai-consult/src/persona.rs` | Korean consult persona and optional day-master context prompt builder. |
| `/Users/kangnam/projects/kangnam-sdk/crates/fortune/ai-consult/src/report.rs` | Report prompt constants and `parse_report_sections`/fallback helpers. |
| `/Users/kangnam/projects/kangnam-sdk/crates/fortune/ai-consult/src/tools/mod.rs` | Tool exports and `consult_tools()` factory. |
| `/Users/kangnam/projects/kangnam-sdk/crates/fortune/ai-consult/src/tools/saju.rs` | `SajuContextTool` implementing `AgentTool<ConsultCapabilities>`. |
| `/Users/kangnam/projects/kangnam-sdk/crates/fortune/ai-consult/src/tools/tarot.rs` | `TarotDrawTool` implementing `AgentTool<ConsultCapabilities>`. |
| `/Users/kangnam/projects/kangnam-sdk/crates/harness/llm-bridge/src/lib.rs` | Add `LlmAgent::run_messages(messages)` so consult can pass existing chat history, not only a single prompt. |
| `/Users/kangnam/projects/kangnam-sdk/crates/harness/llm-bridge/tests/loop_with_mock.rs` | Add tests for `run_messages` preserving caller-provided history and rejecting empty/non-user-ending input. |

## Implementation order

1. **Red tests for bridge history support**
   - File: `/Users/kangnam/projects/kangnam-sdk/crates/harness/llm-bridge/tests/loop_with_mock.rs`
   - Add `run_messages_uses_existing_history`.
   - Add `run_messages_rejects_empty_input`.
   - Add `run_messages_rejects_history_not_ending_in_user`.

2. **Bridge implementation**
   - File: `/Users/kangnam/projects/kangnam-sdk/crates/harness/llm-bridge/src/lib.rs`
   - Extract the current loop body from `LlmAgent::run` into `run_messages`.
   - Keep `run(user_input)` as a thin wrapper: `self.run_messages(vec![ChatMessage::user(user_input)]).await`.

3. **Red tests for consult guards and persona**
   - Files: new `ai-consult` crate inline `#[cfg(test)]` modules.
   - Port Lunawave expectations for redaction, blocklist, alternation, prompt length, and report parsing.

4. **Create `ai-consult` crate and pure modules**
   - Files: `Cargo.toml`, `src/lib.rs`, `src/types.rs`, `src/guard.rs`, `src/persona.rs`, `src/report.rs`.
   - Keep all functions I/O-free except the final LLM call in `agent.rs`.

5. **Red tests for tools**
   - File: `/Users/kangnam/projects/kangnam-sdk/crates/fortune/ai-consult/src/tools/saju.rs`
   - Verify `SajuContextTool` returns day master, element balance, and Korean psyche keywords from a known profile.
   - File: `/Users/kangnam/projects/kangnam-sdk/crates/fortune/ai-consult/src/tools/tarot.rs`
   - Verify `TarotDrawTool` returns one-card and three-card JSON envelopes using `TarotEngine`.

6. **Tool implementation**
   - `SajuContextTool::execute` reads `ctx.capabilities.birth_profile`.
   - `TarotDrawTool::execute` reads tool params and calls `TarotEngine::generate`.
   - Missing profile returns `ToolResult::Failed` with a Korean explanation the model can recover from.

7. **Red tests for consult agent loop**
   - File: `/Users/kangnam/projects/kangnam-sdk/crates/fortune/ai-consult/src/agent.rs`
   - Use `kangnam-harness-llm-bridge` with feature `test-util`.
   - Script `MockLlmProvider` to call `consult.saju_context`, then return final Korean text.
   - Assert guard redaction happens before the user message enters the LLM history.

8. **Consult agent implementation**
   - `AiConsultSession::respond` validates and redacts input, normalizes history, builds `LlmAgent<ConsultCapabilities>`, registers tools, runs `run_messages`, and returns final text + tool invocation log.

9. **Verification**
   - `cargo fmt -- /Users/kangnam/projects/kangnam-sdk/crates/harness/llm-bridge/src/lib.rs`
   - `cargo fmt -- /Users/kangnam/projects/kangnam-sdk/crates/fortune/ai-consult/src/lib.rs`
   - `cargo test -p kangnam-harness-llm-bridge --features test-util --test loop_with_mock`
   - `cargo test -p ai-consult`
   - `cargo check -p ai-consult`

## Function/API signatures

```rust
// crates/harness/llm-bridge/src/lib.rs
impl<C: Send + Sync + 'static> LlmAgent<C> {
    pub async fn run_messages(
        &self,
        messages: Vec<kangnam_router::ChatMessage>,
    ) -> Result<AgentRun, BridgeError>;
}
```

```rust
// crates/fortune/ai-consult/src/types.rs
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BirthProfile {
    pub birth_date: String,
    pub birth_time: Option<String>,
    pub calendar_type: Option<String>,
    pub gender: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ConsultRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConsultMessage {
    pub role: ConsultRole,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct ConsultConfig {
    pub max_turns_per_session: usize,
    pub max_message_length: usize,
    pub max_history_messages: usize,
    pub max_agent_iterations: u32,
}

#[derive(Debug, Clone)]
pub struct ConsultCapabilities {
    pub birth_profile: Option<BirthProfile>,
}

pub struct ConsultRequest {
    pub session_id: String,
    pub user_message: String,
    pub history: Vec<ConsultMessage>,
    pub birth_profile: Option<BirthProfile>,
}

pub struct ConsultResponse {
    pub text: String,
    pub messages: Vec<kangnam_router::ChatMessage>,
    pub tool_invocations: Vec<kangnam_harness_llm_bridge::ToolInvocation>,
    pub redacted: bool,
}
```

```rust
// crates/fortune/ai-consult/src/agent.rs
pub struct AiConsultSession {
    provider: Box<dyn kangnam_router::LlmProviderDyn>,
    config: ConsultConfig,
}

impl AiConsultSession {
    pub fn new(provider: Box<dyn kangnam_router::LlmProviderDyn>) -> Self;
    pub fn with_config(self, config: ConsultConfig) -> Self;
    pub async fn respond(&self, request: ConsultRequest) -> Result<ConsultResponse, ConsultError>;
}
```

```rust
// crates/fortune/ai-consult/src/guard.rs
pub fn redact_pii(text: &str) -> (String, bool);
pub fn safety_rejection(message: &str) -> Option<String>;
pub fn normalize_history(
    history: &[ConsultMessage],
    current_user_message: String,
    max_history_messages: usize,
) -> Vec<kangnam_router::ChatMessage>;
pub fn validate_request(
    request: &ConsultRequest,
    config: &ConsultConfig,
) -> Result<(), ConsultError>;
```

```rust
// crates/fortune/ai-consult/src/persona.rs
pub fn build_system_prompt(profile: Option<&BirthProfile>) -> String;
pub fn build_user_saju_context(profile: &BirthProfile) -> Result<UserSajuContext, ConsultError>;
```

```rust
// crates/fortune/ai-consult/src/report.rs
pub struct ReportParts {
    pub summary: String,
    pub advice: String,
    pub encouragement: String,
}

pub fn parse_report_sections(text: &str) -> Option<ReportParts>;
pub fn parse_report_fallback(text: &str) -> ReportParts;
```

```rust
// crates/fortune/ai-consult/src/tools/mod.rs
pub fn consult_tools()
    -> Vec<(std::sync::Arc<dyn kangnam_harness_runtime::AgentTool<ConsultCapabilities>>, &'static str)>;
```

## Constraints

- Keep SDK logic host-agnostic. No sqlx, Axum, wallet, auth, or WebSocket code in `ai-consult`.
- Do not store provider tokens, API keys, or user profile data in SDK globals.
- Keep user-facing copy Korean-first and respectful; use 해요체.
- Preserve Lunawave guard limits unless config overrides them:
  - `MAX_TURNS_PER_SESSION = 20`
  - `MAX_MESSAGE_LENGTH = 2000`
  - `MAX_HISTORY_MESSAGES = 20`
- Use `ChatMessage` from `kangnam-router` because `LlmAgent` already speaks router messages.
- `LlmAgent::run_messages` must require the message list to be non-empty and end with role `"user"`.
- Tool outputs are JSON. Do not make tools produce long natural-language essays; final phrasing belongs to the LLM.
- First pass is non-streaming. Streaming consult over WebSocket remains host integration work unless `llm-bridge` grows a dedicated `run_messages_stream` API.
- Implementation should happen after `kangnam-sdk` branch divergence is resolved or explicitly accepted. Current status observed: `main...origin/main [ahead 2, behind 3]`.

## Decisions

- **Adopted:** Add `crates/fortune/ai-consult` as a separate SDK crate that consumes harness runtime/llm-bridge, saju engine, tarot engine, and router.
- **Rejected:** Put consult logic inside `kangnam-harness-*`; reason: harness is domain-agnostic and should not contain Dalgyeol/Korean fortune persona rules.
- **Rejected:** Keep all saju context in a static system prompt; reason: harness tools let the model request structured saju/tarot context only when needed and keep the base prompt smaller.
- **Rejected for this pass:** SDK-owned persistence or billing; reason: Lunawave already owns session rows and point charging, and SDK should stay reusable.
- **Deferred:** Streaming tool-call loop; reason: current `LlmAgent` has a complete non-streaming tool loop and this feature can ship as a reusable backend service first. A later `run_messages_stream` should be designed separately if the host requires tool-call streaming.

## External dependency inventory

| Dependency | Check-only command | Expected | Fallback |
|---|---|---|---|
| `kangnam-harness-runtime` crate | `grep '^name' /Users/kangnam/projects/kangnam-sdk/crates/harness/runtime/Cargo.toml` | `name = "kangnam-harness-runtime"` | Stop; do not add crate until actual package name is reconciled. |
| `kangnam-harness-llm-bridge` crate | `grep '^name' /Users/kangnam/projects/kangnam-sdk/crates/harness/llm-bridge/Cargo.toml` | `name = "kangnam-harness-llm-bridge"` | Stop; bridge API cannot be assumed. |
| `LlmAgent` surface | `grep -n 'pub struct LlmAgent' /Users/kangnam/projects/kangnam-sdk/crates/harness/llm-bridge/src/lib.rs` | `pub struct LlmAgent<C = DefaultCapabilities>` exists | Add consult as direct router wrapper only after separate design update. |
| `AgentTool` surface | `grep -n 'pub trait AgentTool' /Users/kangnam/projects/kangnam-sdk/crates/harness/runtime/src/tool.rs` | `pub trait AgentTool<C = DefaultCapabilities>` exists | Stop; no harness runtime implementation. |
| `kangnam-router` provider API | `grep -n 'fn chat_with_options_dyn' /Users/kangnam/projects/kangnam-sdk/crates/router/src/lib.rs` | Object-safe provider options API exists | Limit consult to prompt-only `chat_dyn` and defer tool calling. |
| `saju-engine` | `grep '^name' /Users/kangnam/projects/kangnam-sdk/crates/fortune/saju-engine/Cargo.toml` | `name = "saju-engine"` | Stop; do not duplicate saju calculations. |
| `tarot-engine` | `grep '^name' /Users/kangnam/projects/kangnam-sdk/crates/fortune/tarot-engine/Cargo.toml` | `name = "tarot-engine"` | Ship saju-only consult tools; document tarot as deferred. |
| LLM provider/API key | Host app runtime config, not checked in SDK | Provider is passed as `Box<dyn LlmProviderDyn>` | Tests use `MockLlmProvider`; production host decides fallback provider. |
| Network access | None for unit tests | No live LLM call in default tests | Live LM Studio tests stay ignored/opt-in outside this crate. |

Inventory already verified manually for the current local tree before this draft:
- `kangnam-harness-runtime`, `kangnam-harness-llm-bridge`, `saju-engine`, and `tarot-engine` package names exist.
- `LlmAgent<C = DefaultCapabilities>` and `AgentTool<C = DefaultCapabilities>` exist.
- `SajuEngine::generate` and `TarotEngine::generate` exist and return `(serde_json::Value, String)`.

## Test plan

Run tests in this order:

```sh
cargo test -p kangnam-harness-llm-bridge --features test-util --test loop_with_mock
cargo test -p ai-consult
cargo check -p ai-consult
```

If broader workspace confidence is required after the crate is green:

```sh
cargo test --workspace
```

Do not run live LLM tests by default. No API keys or LM Studio server are required for this implementation.
