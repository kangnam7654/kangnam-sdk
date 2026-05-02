# Thinking + Anthropic Prompt Caching + Streaming Usage (v0.5.0)

> **Status: draft**

## Purpose

Add three production LLM patterns to llm-router that v0.4.x is missing:
1. **Thinking/Reasoning block separation** — surface model's internal reasoning as a distinct stream event for UI rendering.
2. **Anthropic Prompt Caching** — opt-in cache markers for 90% cost reduction on long system prompts and RAG context.
3. **Streaming token usage updates** — live cost-display updates during long streams.

**Done when:** (a) Setting `LlmRequestOptions.thinking_budget_tokens: Some(8192)` on `claude` makes the response stream emit `LlmStreamEvent::Thinking { text }` events for reasoning blocks separate from `Delta`. (b) Setting `cache_breakpoints: vec![CacheBreakpoint::System]` on `claude` makes the request body include `cache_control: {type:"ephemeral"}` on the system block, and the response's `cache_read_input_tokens` populates correctly. (c) Long streams from any HTTP provider with usage support emit periodic `LlmStreamEvent::Usage { ... }` events with running token counts and estimated cost.

## Motivation

v0.4.x covers tool calling and multimodal but misses three patterns that 2026-era LLM apps depend on:

- **Thinking blocks**: Claude 4 / Sonnet 4.5+ extended thinking, OpenAI o1/o3/o4 reasoning, Gemini 3.x `thinking_level` all expose internal reasoning. Apps want to display this in a collapsed/grayed UI region distinct from the main answer. Currently llm-router collapses both into `rendered_text` (HTTP) or skips entirely (local CLI). This was deferred from v0.4.0 per user decision; user has now revisited.
- **Anthropic Prompt Caching**: long system prompts (10K+ tokens) and RAG-stuffed contexts at $3/MTok input rate burn budget fast. Anthropic offers a 90% discount on cached read with 5-min TTL via `cache_control: {type: "ephemeral"}` markers. llm-router has no surface for this — every call re-pays full input cost.
- **Streaming Usage**: long streams (multi-minute thinking + multi-thousand-token output) leave the UI without live cost feedback. The user only learns the cost at `End { total }`. Anthropic and Gemini expose progressive usage updates during streams; llm-router doesn't surface them.

## Non-Goals (deferred)

- **Embeddings provider trait** — separate API surface, separate PR.
- **Automatic retry/backoff** — caller responsibility (network resilience is app-level concern).
- **Provider fallback chain** — orchestration concern, not transport concern.
- **Structured output (`response_format` / JSON Schema)** — separate feature, not coupled to thinking/caching.
- **Telemetry hooks** — observability framework concern.
- **Audio/voice (TTS, STT)** — different modality.
- **Tool definition caching** (`CacheBreakpoint::ToolByName` / `ToolIndex`) — narrow use case (only matters for very long tool schemas + dynamic tool sets); use `CacheBreakpoint::System` to cache system+tools prefix in most cases. Add in v0.5.x when concrete demand surfaces.

## Architecture

### 1. New types

```rust
/// Cache marker positions for Anthropic prompt caching. Each variant
/// targets one location in the request that should be cached up to.
/// Anthropic billing: 90% discount on cached read, 5-min TTL, max 4
/// cache breakpoints per request (excess silently truncated).
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum CacheBreakpoint {
    /// Cache the system prompt (and any tool definitions Anthropic
    /// groups with it). Most common case.
    System,
    /// Cache `messages[0..=index]` (inclusive). Useful for caching
    /// long RAG context blocks placed early in the conversation.
    /// Out-of-range indices are silently skipped with `tracing::warn!`.
    MessageIndex(usize),
}
```

### 2. `LlmRequestOptions` extensions

```rust
pub struct LlmRequestOptions {
    // ... existing v0.4.1 fields ...

    /// Anthropic extended thinking budget (input tokens reserved for
    /// reasoning). `None` = thinking off. Honored by `claude` HTTP and
    /// `claude_local` CLI providers; ignored elsewhere.
    ///
    /// When `Some(n)`, the `claude` HTTP provider also auto-adds the
    /// `anthropic-beta: interleaved-thinking-2025-05-14` header (required
    /// for Claude 4 / Sonnet 4.5; safely ignored on Claude 4.6+).
    pub thinking_budget_tokens: Option<u32>,

    /// Anthropic prompt caching breakpoints. Empty = no caching.
    /// Honored by `claude` HTTP only; silently ignored elsewhere.
    /// Anthropic limit: 4 breakpoints max per request — excess truncated
    /// with `tracing::warn!`. Invalid `MessageIndex` (out of range)
    /// silently skipped with `tracing::warn!`.
    pub cache_breakpoints: Vec<CacheBreakpoint>,
}
```

### 3. `LlmResponse` extensions + `#[non_exhaustive]`

```rust
#[non_exhaustive]
#[derive(Debug, Default)]
pub struct LlmResponse {
    pub rendered_text: String,
    pub model: String,
    pub estimated_cost_usd: f64,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub tool_calls: Vec<ToolCall>,

    // NEW in v0.5.0:
    /// Accumulated reasoning/thinking text (separate from `rendered_text`).
    /// `None` if the model produced no thinking content. Populated by
    /// `claude` HTTP / `claude_local` (extended thinking blocks),
    /// `codex` / `openai_compat` (o-series reasoning_content), `gemini`
    /// (3.x thinking parts).
    pub thinking_text: Option<String>,

    /// Anthropic only: tokens written to cache this turn (uncached input
    /// for cache breakpoints that produced a new cache entry).
    pub cache_creation_input_tokens: Option<u32>,

    /// Anthropic only: tokens read from cache this turn — billed at 10%
    /// of the standard input rate.
    pub cache_read_input_tokens: Option<u32>,
}
```

**Adding `#[non_exhaustive]` to `LlmResponse`** is a breaking change in semver but minimal real impact: external `LlmResponse { ... }` literal sites become illegal (must use `..Default::default()`). Almost no caller constructs `LlmResponse` directly — only test fixtures and mock providers do. The 4 downstream projects' last migration (v0.4.0) didn't surface this site type; expected zero external sites.

### 4. `LlmStreamEvent` extensions

```rust
#[non_exhaustive]
pub enum LlmStreamEvent {
    Delta { text: String },
    End { total: LlmResponse },
    Error { message: String },
    ToolCall { call: ToolCall },

    // NEW in v0.5.0:
    /// Incremental thinking/reasoning text. Emitted as model produces
    /// reasoning before/between answer text.
    Thinking { text: String },

    /// Periodic usage snapshot. Emit cadence varies per provider:
    /// - Anthropic: per `message_delta` (real progressive)
    /// - Gemini: per SSE chunk (most granular)
    /// - OpenAI / OpenAI-compat: only the final chunk before `End`
    ///   (effectively redundant with `End`, but fires earlier so caller
    ///   can update UI before terminal event)
    /// - Local CLI: only on `result` message before `End`
    ///
    /// Throttling: providers SHOULD throttle to one event per 50 output
    /// token delta to avoid stream noise. Caller-side throttling not
    /// required.
    Usage {
        input_tokens: Option<u32>,
        output_tokens: u32,
        estimated_cost_usd: f64,
    },
}
```

### 5. Provider behavior matrix

| Provider | Thinking | Caching | Streaming Usage |
| :--- | :---: | :---: | :---: |
| `claude` (HTTP) | ✅ extended thinking blocks | ✅ cache_control markers | ✅ progressive (message_delta) |
| `claude_local` | ✅ stream-json thinking blocks | ❌ CLI doesn't expose | ✅ result usage only |
| `codex` | ✅ Responses API reasoning | ❌ N/A | ✅ final chunk only |
| `openai_compat` | ✅ reasoning_content (o-series) | ❌ N/A | ✅ final chunk only (`include_usage`) |
| `gemini` | ✅ Gemini 3.x thinking parts | ❌ N/A | ✅ progressive (per-chunk usageMetadata) |
| `copilot` | ⚠️ if endpoint exposes (untested) | ❌ N/A | ⚠️ best-effort |
| `dummy` | ❌ no-op | ❌ no-op | ❌ no-op |

## File changes

- `src/lib.rs`:
  - New types: `CacheBreakpoint`.
  - `LlmRequestOptions`: add `thinking_budget_tokens`, `cache_breakpoints`.
  - `LlmResponse`: add `thinking_text`, `cache_creation_input_tokens`, `cache_read_input_tokens`. Add `#[non_exhaustive]` + `#[derive(Default)]`. (Default already added in v0.4.0; verify.)
  - `LlmStreamEvent`: add `Thinking` and `Usage` variants. Already `#[non_exhaustive]` from v0.4.0.
  - Tests in `request_options_tests`, `response_token_tests`, `stream_event_tests` modules.

- `src/claude.rs` (HTTP, biggest changes):
  - Request body: add `thinking: { type: "enabled", budget_tokens: N }` when `thinking_budget_tokens.is_some()`.
  - Auto-add `anthropic-beta: interleaved-thinking-2025-05-14` header when thinking enabled.
  - Apply `cache_control` markers per `cache_breakpoints` per Anthropic spec (system block, message content blocks). Truncate to 4 max with warn. Skip invalid MessageIndex with warn.
  - Response parsing (blocking): extract `content[].type == "thinking"` blocks → `thinking_text`. Extract `usage.cache_creation_input_tokens`, `usage.cache_read_input_tokens` → response fields.
  - Response parsing (streaming): emit `LlmStreamEvent::Thinking` on `content_block_start type=thinking` + `content_block_delta type=thinking_delta`. Emit `LlmStreamEvent::Usage` on `message_delta` (with throttle). Final cache token counts on terminal `message_stop`.

- `src/claude_local.rs`:
  - Request: pass `thinking_budget_tokens` to CLI as `--thinking-budget <n>` flag (verify CLI flag name; if unsupported, log warn + skip).
  - Stream-json parsing: detect `content_block` with `type: "thinking"` → emit `LlmStreamEvent::Thinking { text }`. Accumulate into `thinking_text` on End.
  - No caching support (CLI doesn't expose).

- `src/codex.rs`:
  - Request: existing `reasoning_effort` already sets the OpenAI Responses API param; ensure it's set when `thinking_budget_tokens.is_some()` (treat any Some(_) as "high" effort if `reasoning_effort` is None, otherwise honor explicit `reasoning_effort`).
  - Response (blocking): extract `output[].reasoning` from Responses API → `thinking_text`.
  - Streaming: codex has no real SSE impl (per Task 7 of v0.4.0), so thinking arrives via the wrapped Delta+End pattern. Pre-emit `Thinking { text }` before Delta/End if reasoning present. `Usage` emitted once before End.

- `src/openai_compat.rs`:
  - Request: add `stream_options: { include_usage: true }` to streaming requests (already done in v0.2.0 — verify).
  - Response (blocking): extract `choices[0].message.reasoning_content` if present (OpenAI o-series via Chat Completions API extension).
  - Response (streaming): emit `Thinking { text }` for `delta.reasoning_content` chunks. Emit `Usage` on the final SSE chunk (single event before End).

- `src/gemini.rs`:
  - Request: map `LlmRequestOptions.reasoning_effort` (existing) to Gemini 3.x `thinking_level: "low"|"medium"|"high"`. Map `thinking_budget_tokens` similarly (any Some → "high" if not otherwise set).
  - Response (blocking): extract thinking parts from `candidates[0].content.parts[]` (Gemini 3.x exposes `thoughtSummary` or similar — verify exact JSON shape during implementation; spec subagent should consult Gemini API docs via WebFetch).
  - Streaming: emit `Thinking` for thinking part chunks. Emit `Usage` per SSE chunk (Gemini provides cumulative `usageMetadata`).

- `src/copilot.rs`: best-effort. Document as experimental. Apply openai_compat pattern.

- `src/dummy.rs`: ignore new options; no thinking/caching/usage events emitted.

- `src/cli_utils.rs`: no changes expected unless `claude_local` thinking flag handling needs new helper.

- `examples/`:
  - `examples/thinking.rs` — show how to consume `Thinking` + `Delta` events separately.
  - `examples/prompt_caching.rs` — show `CacheBreakpoint::System` + `MessageIndex` usage on claude HTTP.
  - `examples/streaming_usage.rs` — show `Usage` event consumption with running cost display.

- `CHANGELOG.md`: `[0.5.0]` entry.

- `docs/migration/v0.5.0.md` (new): migration guide. v0.5.0 changes:
  - `LlmResponse { ... }` literal → use `..Default::default()` (only test fakes affected).
  - `LlmStreamEvent` exhaustive match: add `_` arm for `Thinking` + `Usage` (already have `_` arm if migrated for v0.4.0, no action needed).
  - New optional fields on `LlmRequestOptions`: zero-action (defaults Vec/None).

- `Cargo.toml`: bump `0.4.1` → `0.5.0`.

## Implementation order

Each commit is self-contained. Pattern follows v0.4.0 plan structure.

### Phase 0: Setup (1 task)
- **Task 0**: Verify baseline (151 lib tests pass + 1 env-fail), confirm clean tree on feature branch.

### Phase 1: Type scaffolding (3 tasks, no behavior change)
- **Task 1**: Add `CacheBreakpoint` type + tests.
- **Task 2**: Extend `LlmRequestOptions` with `thinking_budget_tokens` + `cache_breakpoints` + `LlmResponse` with `thinking_text` + cache token fields. Update all `LlmResponse { ... }` construction sites (~20 sites). Mark `LlmResponse` `#[non_exhaustive]`.
- **Task 3**: Extend `LlmStreamEvent` with `Thinking` + `Usage` variants. Update existing `match` blocks to include or `_`-arm.

### Phase 2: Thinking implementation (5 tasks)
- **Task 4**: claude HTTP — request `thinking` param + beta header, response `thinking` content blocks (blocking + streaming).
- **Task 5**: claude_local — stream-json `thinking` block parsing + budget flag.
- **Task 6**: codex — Responses API `reasoning` extraction.
- **Task 7**: openai_compat — `reasoning_content` extraction (blocking + streaming SSE delta).
- **Task 8**: gemini — Gemini 3.x thinking part parsing + `thinking_level` mapping. Spec implementer must consult Gemini 3 API docs first.

### Phase 3: Anthropic Prompt Caching (1 task)
- **Task 9**: claude HTTP only — request body `cache_control` placement per breakpoints + truncation/validation logic + response parsing for cache token counts.

### Phase 4: Streaming Usage (4 tasks)
- **Task 10**: claude HTTP — `message_delta` Usage emission + throttle.
- **Task 11**: codex + openai_compat — final-chunk Usage emission.
- **Task 12**: gemini — per-chunk progressive Usage emission.
- **Task 13**: copilot — best-effort (experimental).

### Phase 5: Examples + release (3 tasks)
- **Task 14**: Write 3 example files + verify they compile + cargo run --example smoke test.
- **Task 15**: CHANGELOG `[0.5.0]` entry + `docs/migration/v0.5.0.md` + Cargo.toml bump.
- **Task 16**: Final whole-branch review + tag (after user approval).

Total: ~17 tasks, similar shape to v0.4.0 (18 tasks).

## Function/API signatures

```rust
// src/lib.rs

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum CacheBreakpoint {
    System,
    MessageIndex(usize),
}

pub struct LlmRequestOptions {
    // ... existing v0.4.1 fields ...
    pub thinking_budget_tokens: Option<u32>,
    pub cache_breakpoints: Vec<CacheBreakpoint>,
}

#[non_exhaustive]  // NEW in v0.5.0
#[derive(Debug, Default)]
pub struct LlmResponse {
    pub rendered_text: String,
    pub model: String,
    pub estimated_cost_usd: f64,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub tool_calls: Vec<ToolCall>,
    pub thinking_text: Option<String>,
    pub cache_creation_input_tokens: Option<u32>,
    pub cache_read_input_tokens: Option<u32>,
}

#[non_exhaustive]
pub enum LlmStreamEvent {
    Delta { text: String },
    End { total: LlmResponse },
    Error { message: String },
    ToolCall { call: ToolCall },
    Thinking { text: String },
    Usage {
        input_tokens: Option<u32>,
        output_tokens: u32,
        estimated_cost_usd: f64,
    },
}
```

No trait method signature changes. Provider impls update their request/response logic only.

## Constraints

- **Backward compat**: `LlmRequestOptions::default()` produces empty `cache_breakpoints` Vec and `None` `thinking_budget_tokens` → request bodies byte-identical to v0.4.1 for callers who don't opt in.
- **`LlmResponse` `#[non_exhaustive]` is the one breaking change**: external struct literals stop compiling. Migration: use `..Default::default()`. Affected sites in 4 downstream: per v0.4.0 migration data, ZERO external `LlmResponse` literal sites surfaced (all internal/test-only). Risk low.
- **Throttle policy for `Usage` emission**: providers throttle to once per ≥50 output token delta. Caller doesn't need to dedupe. Final `End` event is canonical source of truth.
- **Cache breakpoint validation**: warn + skip on invalid `MessageIndex(>=len)`; warn + truncate to 4 on excess breakpoints. Never error/panic — caller's caching intent should not break their request.
- **Anthropic beta header**: auto-added when `thinking_budget_tokens.is_some()`. Always safe (required on Claude 4 ≤ 4.5, deprecated-but-ignored on 4.6+). No model detection.
- **No new `LlmError` variants**: all failure modes route through existing taxonomy (`Network`, `Parse`, `Upstream`, etc.).
- **No `chat_dyn` / `chat_stream_dyn` behavior change**: the no-options paths keep v0.4.x behavior verbatim. Thinking / caching / usage emit only when caller opts in via `chat_with_options_dyn` / `chat_stream_with_options_dyn`.
- **Pricing tables**: each provider's `estimate_cost_usd` function (or equivalent) feeds `Usage.estimated_cost_usd`. Models without entries return `0.0` (matches v0.4.x behavior for unknown models).
- **Test coverage**: each provider's new feature gets at least one wiremock-backed test (HTTP) or fixture-backed test (CLI). Dummy provider gets compile-only verification.
- **Gemini 3.x thinking exact JSON shape**: spec implementer for Task 8 MUST consult Gemini 3 API docs via WebFetch before implementing — published spec may differ from rumored shape (`thoughtSummary` was provisional name).

## Decisions

- **Adopted: `LlmResponse` `#[non_exhaustive]`** — one-time breaking, permanent future-additivity. Same trade-off accepted for `ChatMessage`/`ChatContent`/`LlmStreamEvent` in v0.4.0.
- **Adopted: 2 cache breakpoint variants only (System + MessageIndex)** — covers the common case (long system + RAG context). `ToolByName`/`ToolIndex` deferred until concrete demand surfaces.
- **Adopted: warn + skip for invalid breakpoints, warn + truncate for >4 breakpoints** — library transports caller intent; doesn't enforce business policy by panic.
- **Adopted: auto-add Anthropic interleaved-thinking beta header** — required on older Claude 4, ignored on newer; no caller-facing complexity.
- **Adopted: `Usage` event throttle = ≥50 output token delta** — balances UI freshness vs stream noise.
- **Adopted: include Gemini 3.x in v0.5.0 thinking scope** — current production model (3.1 Pro / 3 Flash); excluding would create a gap.
- **Rejected: separate `thinking_level` enum** — reuse existing `reasoning_effort: Option<String>` (already a String taking "low"/"medium"/"high"). Adding a typed enum doubles the surface for the same semantic.
- **Rejected: `CacheBreakpoint::ToolByName(String)` / `ToolIndex(usize)`** — narrow case, defer.
- **Rejected: per-call observability hooks** — out of scope (Tier 2/3 work).
- **Rejected: tool-result caching, ephemeral vs 1h TTL options** — Anthropic only exposes 5-min ephemeral as a stable feature; 1h is beta/preview. Stick to ephemeral.
- **Rejected: client-side cost tracking accumulator** — provider's `estimate_cost_usd` per response is sufficient; aggregation across calls is caller's responsibility.

## Open questions for user review (none — confirmed before spec)

All decisions confirmed in 2026-04-27 conversation. No open items.
