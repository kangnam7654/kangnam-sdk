# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [0.5.1] - 2026-04-27

### Fixed
- `claude` cache_breakpoint `MessageIndex` now logs `tracing::warn!`
  when the target message's last content block is a `Thinking` block
  (cache_control inapplicable). Previously silent — created a footgun
  for callers who expected confirmation that a breakpoint was applied.
- `claude` OAT 400 → Haiku fallback path now correctly populates
  `LlmResponse.cache_creation_input_tokens` and `cache_read_input_tokens`
  from the fallback response's `usage` block. Previously these fields
  remained `None` in the fallback path even when usage was present.

### Documentation
- `CacheBreakpoint::MessageIndex` rustdoc now mentions the Thinking-block
  edge case.
- **Migration guide correction (`docs/migration/v0.5.0.md`)**: the v0.5.0
  guide incorrectly suggested `..Default::default()` for migrating
  external `LlmResponse { ... }` literal sites. Rust's `#[non_exhaustive]`
  rejects ALL struct-literal expressions from external crates (E0639),
  including FRU. The correct external pattern is `LlmResponse::default()`
  + field mutation. Two downstream projects (Lunawave, dear-jeongbin)
  hit this during their v0.5.0 migration. Guide now explains the
  limitation and shows the correct pattern.

### Fixed (also)
- `LlmResponse` for `..Default::default()` works inside the `llm-router`
  crate but NOT external — clarification added to migration guide and
  `LlmResponse` rustdoc.

### Migration
None. Strictly additive (additional warn + correctly populated fields +
documentation correction).

## [0.5.0] - 2026-04-27

### Breaking
- `LlmResponse` is now `#[non_exhaustive]`. External struct literal
  syntax `LlmResponse { ... }` no longer compiles. Use
  `..Default::default()`. Internal callers and downstream code unaffected
  unless mock providers / test fixtures construct `LlmResponse` directly.
  See `docs/migration/v0.5.0.md`.

### Added
- **Thinking / reasoning block separation**:
  - `LlmStreamEvent::Thinking { text }` variant for progressive reasoning text.
  - `LlmResponse.thinking_text: Option<String>` accumulated total.
  - `LlmRequestOptions.thinking_budget_tokens: Option<u32>` (Anthropic budget).
  - `claude` HTTP: extended thinking blocks (`thinking: { type: "enabled", budget_tokens }` request param + `interleaved-thinking-2025-05-14` beta header auto-added; `content[].type == "thinking"` blocks parsed; SSE `content_block_delta` with `thinking_delta` accumulated per index via BTreeMap).
  - `claude_local`: stream-json `thinking` content blocks parsed; `--thinking-budget` flag passed when set.
  - `codex`: Responses API `output[].type == "reasoning"` items extracted (with `reasoning_content` string field or `summary[].text` array fallback). `thinking_budget_tokens.is_some() && reasoning_effort.is_none()` → infers `"high"` reasoning effort.
  - `openai_compat`: `choices[0].message.reasoning_content` (blocking) + `choices[0].delta.reasoning_content` (streaming SSE) → progressive `Thinking` events.
  - `gemini`: Gemini 3.x `parts[]` with `thought: true` flag detected; `generationConfig.thinkingConfig.thinkingLevel` set from `reasoning_effort`.
- **Anthropic prompt caching** (`claude` HTTP only):
  - `CacheBreakpoint` enum (`#[non_exhaustive]`) with `System` and `MessageIndex(usize)` variants.
  - `LlmRequestOptions.cache_breakpoints: Vec<CacheBreakpoint>`.
  - `cache_control: { type: "ephemeral" }` markers placed on `system` blocks and `messages[i].content[last]` per breakpoint.
  - 4-breakpoint limit enforced via `tracing::warn!` + truncate to first 4.
  - Out-of-range `MessageIndex` silently skipped with `tracing::warn!`.
  - `LlmResponse.cache_creation_input_tokens` and `cache_read_input_tokens` populated from response `usage`.
- **Streaming usage updates**:
  - `LlmStreamEvent::Usage { input_tokens, output_tokens, estimated_cost_usd }` variant.
  - `claude` HTTP: progressive emission from `message_delta` events with 50-output-token throttle. Cost via existing `estimate_cost` helper.
  - `gemini`: emitted before `End` (single event; gemini uses Delta+End wrap, no real per-chunk SSE).
  - `codex` + `openai_compat`: single `Usage` event before `End` from final chunk's usage block.
  - `copilot`: experimental — same pattern as openai_compat.
- 3 new examples under `examples/`: `thinking.rs`, `prompt_caching.rs`, `streaming_usage.rs`.

### Internal
- `LlmResponse` got `#[derive(Default)]` (already in v0.4.0 — verified preserved).
- All ~21 in-crate `LlmResponse { ... }` construction sites converted to `..Default::default()` pattern for future-additivity.
- BTreeMap-based thinking accumulator on claude HTTP streaming (deterministic iteration order, mirrors v0.4.0 tool_call accumulator pattern).
- Cross-task collateral cleanup of pre-existing clippy warnings during Wave A/B.

### Migration
See `docs/migration/v0.5.0.md`. Changes are mostly additive; the single breaking item is `LlmResponse` non_exhaustive (rare construction sites only).

## [0.4.1] - 2026-04-26

### Changed
- `ToolChoice` enum is now `#[non_exhaustive]`. Future tool-choice modes
  (e.g. parallel-call control) can be added without a breaking change.
  Practically: external `match` arms on `ToolChoice` should now include
  a `_ => {}` arm. Internal crate code is unaffected.

### Documentation
- Migration guide (`docs/migration/v0.4.0.md`) tool calling round-trip
  example now correctly preserves the assistant's text turn before
  appending the `tool_result` message. Empty `assistant("")` placeholder
  removed (it silently dropped the model's response and broke
  conversation history with Anthropic/OpenAI).

## [0.4.0] - 2026-04-26

### Breaking
- `ChatMessage` is now `#[non_exhaustive]` and uses
  `content: Vec<ChatContent>` instead of `content: String`. The struct
  literal syntax `ChatMessage { role, content }` no longer compiles
  outside the crate. Use the new constructors: `ChatMessage::user(text)`,
  `ChatMessage::assistant(text)`, `ChatMessage::user_with_image(text, img)`,
  `ChatMessage::user_multipart(blocks)`, `ChatMessage::tool_result(id, content, is_error)`.
  See `docs/migration/v0.4.0.md` for full migration patterns.
- `LlmStreamEvent` is now `#[non_exhaustive]` and gained a `ToolCall { call: ToolCall }`
  variant. Exhaustive `match` blocks on this enum must add a `_ => {}` arm.
- `ChatContent`, `ImageSource` are `#[non_exhaustive]` enums (future-additivity).

### Added
- **Tool calling** on 4 HTTP providers (`claude`, `codex`, `gemini`,
  `openai_compat`) plus `copilot` (experimental):
  - `LlmRequestOptions { tools: Vec<ToolDef>, tool_choice: Option<ToolChoice>, ..default() }`
  - Models emit tool calls via `LlmResponse.tool_calls` (blocking) or
    `LlmStreamEvent::ToolCall { call }` (streaming).
  - `ChatContent::ToolResult { tool_use_id, content, is_error }` for
    feedback to the model in subsequent turns.
- **HTTP image input** on the same 4 HTTP providers:
  - `ChatContent::Image { source: Base64(...) | Url(...), mime_type }` blocks.
  - Per-provider encoding handled internally (Anthropic image blocks,
    OpenAI image_url, Codex Responses input_image, Gemini inlineData).
  - Gemini does NOT support arbitrary URLs; pass `ImageSource::Base64`
    or pre-upload via Files API.
- **Image helpers**:
  - `ChatContent::image_from_path(path)` — reads file, base64-encodes,
    infers MIME from extension.
  - `ChatContent::image_from_base64_auto(data)` — magic-byte MIME detection.
- New types: `ToolDef`, `ToolCall`, `ToolChoice`, `ChatContent`, `ImageSource`.
- Local CLI providers (`claude_local`, `codex_local`, `gemini_local`)
  gracefully consume multipart `ChatMessage`:
  - `ChatContent::Image` with Base64 → written to temp file + routed through
    existing `image_paths` flow, RAII cleanup.
  - `ChatContent::Image` with URL → tracing::warn! + skip.
  - `ChatContent::ToolResult` → tracing::debug! + drop (CLI handles its
    own internal tools, not user-defined ones).

### Internal
- `infer = "0.16"` and `base64 = "0.22"` direct dependencies.
- `BTreeMap`-based streaming tool call accumulator (claude / openai_compat)
  ensures parallel tool calls emit in deterministic index order.
- `cli_utils::TempFile` newtype for image temp file RAII.
- ~16 new tests across all providers (151 total lib tests pass).

### Migration
See `docs/migration/v0.4.0.md` for complete migration guide.

## [0.3.2] - 2026-04-25

### Added
- `LlmRequestOptions.temperature: Option<f32>` — sampling temperature.
- `LlmRequestOptions.max_tokens: Option<u32>` — max tokens to generate.
- `LlmRequestOptions.top_p: Option<f32>` — nucleus sampling top-p.
- `LlmRequestOptions.stop_sequences: Vec<String>` — stop sequences.

All 4 honored by HTTP providers (`claude`, `codex`, `copilot`, `gemini`,
`openai_compat`) on `chat_with_options_dyn` /
`chat_stream_with_options_dyn`. Local CLI providers silently ignore.

For `claude`, when `max_tokens` is `None` the existing `MAX_TOKENS = 1024`
default applies (Anthropic API requires the field).
For `gemini`, when `temperature` and `max_tokens` are `None` the existing
hardcoded defaults apply (back-compat).

### Migration
None. Adding fields to `LlmRequestOptions` is non-breaking; existing
`..Default::default()` constructions compile unchanged.

## [0.3.1] - 2026-04-25

### Added
- `LlmRequestOptions.timeout: Option<std::time::Duration>` — per-call total
  deadline. Honored on `chat_with_options_dyn` and
  `chat_stream_with_options_dyn`; the no-options paths (`chat_dyn` /
  `chat_stream_dyn`) keep the v0.3.0 hardcoded behavior.
- HTTP providers (`claude`, `codex`, `copilot`, `gemini`, `openai_compat`):
  applied via `RequestBuilder::timeout(d)`, overriding the per-provider
  hardcoded constants on a per-request basis.
- Local CLI providers (`claude_local`, `codex_local`, `gemini_local`):
  when `options.timeout.is_some()`, the call is routed through the
  streaming code path which uses `tokio::process::Command` with
  `kill_on_drop(true)`. On timeout elapse, the dropped stream reaps the
  subprocess. Blocking path with `timeout: None` continues to use
  `spawn_blocking` + `std::process::Command` exactly as v0.3.0.
- New timeout fixtures: `tests/fixtures/fake_{claude,codex,gemini}_slow.sh`.
- Wiremock-driven HTTP timeout tests; fake-CLI timeout tests for the
  three `_local` providers. Total test count: 80 pass.

### Changed
- HTTP timeout error messages normalized: `is_timeout()` branches
  consistently emit `LlmError::Network { provider, msg: "timeout" }`
  (replacing prior provider-specific phrasing like "request timed out"
  or "Copilot API timed out"). The `Network` variant itself is unchanged.
- Per-test `tokio::sync::Mutex` env locks added/extended to cover
  `CLAUDE_CLI_PATH`, `CODEX_CLI_PATH` (existing), and `GEMINI_CLI_PATH`
  so parallel-thread test runs don't race on env-var mutation.

### Migration
- None. The new `timeout` field defaults to `None`; existing
  `LlmRequestOptions { ... }` struct literals using `..Default::default()`
  compile unchanged. No new `LlmError` variant. No factory signature
  change.

### Internal
- Tokio `time` feature explicitly enabled in `Cargo.toml` (was implicit
  via reqwest's transitive deps).

## [0.3.0] - 2026-04-23

### Changed
- `chat_stream_dyn` / `chat_stream_with_options_dyn` on `claude_local`,
  `codex_local`, `gemini_local` now emit incremental `LlmStreamEvent::Delta
  { text }` events as tokens arrive from the CLI, instead of collapsing to
  a single terminal `End`. The `End` event still carries the full accumulated
  text, so consumers that only read `End` continue to work unchanged.
- Subprocess spawning for the three `_local` stream paths switched from
  `std::process::Command` + `spawn_blocking` + `futures::stream::once` to
  `tokio::process::Command` + `tokio::io::AsyncBufReadExt::lines()` +
  `async_stream::stream!`. Stderr is drained on a `tokio::spawn` task
  started before the stdout read-loop and awaited after `child.wait().await`
  to avoid pipe-fill deadlocks. `kill_on_drop(true)` is set so an orphaned
  stream drops its CLI child.

### Internal
- Extracted Codex pricing constants (`CODEX_INPUT_USD_PER_1M`,
  `CODEX_OUTPUT_USD_PER_1M`, `PER_1M`) and a shared `estimate_codex_cost`
  helper, replacing three duplicated cost-estimation sites.
- Added fixture-driven tests: `claude_stream_emits_deltas_from_assistant_events`,
  `codex_stream_emits_deltas_and_captures_usage`,
  `codex_stream_emits_error_on_turn_failed`,
  `gemini_stream_emits_deltas_then_end` exercised via
  `tests/fixtures/fake_{claude,codex,codex_error,gemini}.sh`.
- Enabled tokio features `io-util` and `process`.

## [0.2.0] - 2026-04-23

### Added
- `LlmRequestOptions` struct for typed per-request options: `image_paths`,
  `working_dir`, `allow_web_search`, `allow_local_read`, `max_turns`,
  `reasoning_effort`. Unsupported options are silently ignored per provider.
- `LlmResponse.input_tokens` and `LlmResponse.output_tokens` (`Option<u32>`)
  reporting per-request token usage when the provider exposes it.
- `LlmProviderDyn::chat_with_options_dyn` and `chat_stream_with_options_dyn`
  trait methods. Default implementations forward to `chat_dyn` /
  `chat_stream_dyn`, so existing custom providers compile unchanged.
- `cli_utils` module: `sanitize_prompt` (strips `\0` + C0 controls except
  TAB/LF/CR + DEL/C1), `resolve_binary` (env var → common dirs → shell PATH
  → bare-name fallback with shell-injection guard), `build_path_env`
  (dedup-merged PATH suited for GUI launch contexts).
- `openai_compat` provider for OpenAI-compatible HTTP endpoints, including
  streaming with `stream_options.include_usage` → token usage parsing on
  both blocking and streaming paths.
- `list_models` support for `claude_local`, `codex_local`, `gemini_local`
  (config-based where CLI has no list command).

### Changed
- `ProviderFactory` signature: `(api_key, model)` → `(api_key, model, base_url)`.
  Affects custom providers that implement the factory trait.
- `claude_local`, `codex_local`, `gemini_local` now honor
  `LlmRequestOptions` when called via `chat_with_options_dyn` /
  `chat_stream_with_options_dyn`. Existing `chat_dyn` / `chat_stream_dyn`
  calls are unchanged. Subprocess stderr is drained concurrently to avoid
  pipe-fill deadlocks; binary resolution goes through
  `cli_utils::resolve_binary` with PATH via `cli_utils::build_path_env`.
- Token conversion in `codex_local` switched from `as u32` to
  `u32::try_from(n).ok()` to avoid silent truncation.

### Migration
- Custom `ProviderFactory` implementations: add an unused `_base_url: &str`
  parameter.
- Code constructing `LlmResponse` with struct literals: add
  `input_tokens: None, output_tokens: None`.
- Callers using `create_provider(...)` + `chat_dyn(...)` need no changes.

## [0.1.0] - 2026-04-21

### Added
- Initial import from `lunawave`.
- `LlmProviderDyn` object-safe trait with `render_dyn` / `chat_dyn` / `chat_stream_dyn` methods.
- Built-in providers: `claude`, `codex`, `copilot`, `gemini` (plus `antigravity` alias), `dummy`.
- Factory `create_provider(provider, api_key, model)` with registry keyed by lowercase provider name.

[Unreleased]: https://github.com/kangnam7654/llm-router/compare/v0.5.1...HEAD
[0.5.1]: https://github.com/kangnam7654/llm-router/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/kangnam7654/llm-router/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/kangnam7654/llm-router/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/kangnam7654/llm-router/compare/v0.3.2...v0.4.0
[0.3.2]: https://github.com/kangnam7654/llm-router/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/kangnam7654/llm-router/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/kangnam7654/llm-router/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/kangnam7654/llm-router/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/kangnam7654/llm-router/releases/tag/v0.1.0
