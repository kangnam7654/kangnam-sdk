# Thinking + Anthropic Prompt Caching + Streaming Usage Implementation Plan (v0.5.0)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Each task's checkboxes (`- [ ]`) are for tracking.

**Goal:** Ship v0.5.0 with: thinking/reasoning block separation across 5 providers (claude HTTP / claude_local / codex / openai_compat / gemini), Anthropic Prompt Caching (claude HTTP only), and streaming usage updates across HTTP providers. Plus `LlmResponse` `#[non_exhaustive]` for permanent future-additivity.

**Spec:** `docs/specs/2026-04-27-thinking-caching-streaming-usage.md`

**Tech Stack:** Rust 1.85 (edition 2024). No new dependencies. Uses existing `reqwest`, `tokio`, `async-stream`, `infer`, `base64`, `serde_json`.

**Branch:** `feat/v0.5.0-thinking-caching-streaming-usage` (already checked out from `main` at `d8d5a85`).

---

## Phase 0: Setup

### Task 0: Verify baseline

**Files:** none (just verification)

- [ ] **Step 0.1:** Confirm clean tree on feature branch.
  ```sh
  cd /Users/kangnam/projects/llm-router
  git branch --show-current  # → feat/v0.5.0-thinking-caching-streaming-usage
  git status                  # → clean (only untracked docs/ which is fine)
  ```

- [ ] **Step 0.2:** Baseline test count.
  ```sh
  cargo test --lib 2>&1 | tail -3
  ```
  Expected: 151 pass + 1 env-fail (`codex_local::full_chat_via_cli_returns_text` — pre-existing).

- [ ] **Step 0.3:** No new dependencies needed. Skip Cargo.toml.

---

## Phase 1: Type scaffolding (no behavior change)

### Task 1: Add `CacheBreakpoint` type

**File:** `src/lib.rs`

- [ ] **Step 1.1:** Add the enum after existing `ToolChoice` definition:
  ```rust
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
      MessageIndex(usize),
  }
  ```

- [ ] **Step 1.2:** Add unit test:
  ```rust
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
  ```

- [ ] **Step 1.3:** Verify `cargo build --lib && cargo test --lib cache_breakpoint_tests` clean.

- [ ] **Step 1.4:** Commit: `feat(types): add CacheBreakpoint enum`.

---

### Task 2: Extend `LlmRequestOptions` + `LlmResponse` (with `#[non_exhaustive]`)

**File:** `src/lib.rs` + all 9 provider files (LlmResponse construction site updates)

This is the breaking-change task: `LlmResponse` becomes `#[non_exhaustive]`.

- [ ] **Step 2.1:** Add fields to `LlmRequestOptions` after `cache_breakpoints: Vec<CacheBreakpoint>` (alphabetical for consistency would be after `tool_choice`, but logical grouping after `tools`/`tool_choice` works):
  ```rust
  /// Anthropic extended thinking budget (input tokens reserved for
  /// reasoning). `None` = thinking off. Honored by `claude` HTTP
  /// and `claude_local` CLI providers; ignored elsewhere.
  ///
  /// When `Some(n)`, the `claude` HTTP provider also auto-adds the
  /// `anthropic-beta: interleaved-thinking-2025-05-14` header.
  pub thinking_budget_tokens: Option<u32>,

  /// Anthropic prompt caching breakpoints. Empty = no caching.
  /// Honored by `claude` HTTP only. Anthropic limit: max 4 — excess
  /// truncated with `tracing::warn!`. Out-of-range `MessageIndex`
  /// silently skipped with `tracing::warn!`.
  pub cache_breakpoints: Vec<CacheBreakpoint>,
  ```

- [ ] **Step 2.2:** Mark `LlmResponse` with `#[non_exhaustive]` and add new fields:
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
      /// Accumulated reasoning/thinking text. None if model produced
      /// no thinking content.
      pub thinking_text: Option<String>,
      /// Anthropic only: tokens written to cache this turn.
      pub cache_creation_input_tokens: Option<u32>,
      /// Anthropic only: tokens read from cache (10% rate).
      pub cache_read_input_tokens: Option<u32>,
  }
  ```

- [ ] **Step 2.3:** Update ALL `LlmResponse { ... }` construction sites across all provider files to use `..Default::default()` pattern, OR add the 3 new fields explicitly. Search:
  ```sh
  grep -rn 'LlmResponse {' /Users/kangnam/projects/llm-router/src
  ```
  Expected ~20 sites (per v0.4.0 audit). For each:
  - If the site builds with `tool_calls: Vec::new()` and 5 other fields → easiest to refactor to `LlmResponse { rendered_text: ..., ..Default::default() }` style.
  - If site already uses `..Default::default()` (rare but possible) → no change needed.

- [ ] **Step 2.4:** Update `default_options_are_empty` test to assert new field defaults.
- [ ] **Step 2.5:** Update `default_response_has_empty_tool_calls` test (or rename to `default_response_has_zero_state`) to also assert `thinking_text.is_none()`, `cache_creation_input_tokens.is_none()`, `cache_read_input_tokens.is_none()`.

- [ ] **Step 2.6:** Add test for `#[non_exhaustive]` behavior:
  ```rust
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
  ```

- [ ] **Step 2.7:** Verify:
  ```sh
  cargo build --lib
  cargo test --lib
  cargo clippy --all-targets -- -D warnings
  ```
  Expected: 151 + ~3 new tests = ~154 pass + 1 env-fail. No regression.

- [ ] **Step 2.8:** Commit: `feat(types)!: LlmResponse non_exhaustive + thinking_text/cache fields + thinking_budget_tokens/cache_breakpoints options`. The `!` indicates breaking change.

---

### Task 3: Extend `LlmStreamEvent` with `Thinking` + `Usage` variants

**File:** `src/lib.rs`

- [ ] **Step 3.1:** Add the two new variants:
  ```rust
  #[non_exhaustive]
  pub enum LlmStreamEvent {
      Delta { text: String },
      End { total: LlmResponse },
      Error { message: String },
      ToolCall { call: ToolCall },

      /// Incremental reasoning/thinking text. Emitted as the model
      /// produces internal reasoning before/between answer text.
      /// Accumulated total appears in `End { total: { thinking_text } }`.
      Thinking { text: String },

      /// Periodic usage snapshot. Emit cadence varies per provider:
      /// - Anthropic: per `message_delta` (real progressive)
      /// - Gemini: per SSE chunk
      /// - OpenAI / OpenAI-compat: only the final chunk before End
      /// - Local CLI: only on `result` message
      ///
      /// Throttle: providers SHOULD throttle to ≥50 output token
      /// delta to avoid stream noise.
      Usage {
          input_tokens: Option<u32>,
          output_tokens: u32,
          estimated_cost_usd: f64,
      },
  }
  ```

- [ ] **Step 3.2:** Find existing `match` on `LlmStreamEvent` in production code (not tests) and confirm `_` arms exist (added in v0.4.0 Task 3). If new exhaustive matches landed in v0.4.x, add `_` arm minimally.

- [ ] **Step 3.3:** Add tests in `stream_event_tests` mod:
  ```rust
  #[test]
  fn thinking_variant_constructs() {
      let e = LlmStreamEvent::Thinking { text: "step 1".into() };
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
          LlmStreamEvent::Usage { output_tokens, .. } => assert_eq!(output_tokens, 50),
          _ => panic!("expected Usage"),
      }
  }
  ```

- [ ] **Step 3.4:** Verify `cargo build --lib && cargo test --lib && cargo clippy --all-targets -- -D warnings`. Expected: ~156 pass + 1 env-fail.

- [ ] **Step 3.5:** Commit: `feat(stream): add Thinking and Usage variants to LlmStreamEvent`.

---

## Phase 2: Thinking implementation per provider

### Task 4: claude HTTP — extended thinking

**Files:** `src/claude.rs`, `tests/fixtures/claude_thinking_response.json` (new)

Anthropic Messages API extended thinking:
- Request param: `thinking: { type: "enabled", budget_tokens: N }`
- Beta header: `anthropic-beta: interleaved-thinking-2025-05-14`
- Response content block: `{type: "thinking", thinking: "..."}`
- Streaming SSE: `content_block_start` with `type: "thinking"`, then `content_block_delta` with `delta: {type: "thinking_delta", thinking: "..."}`, then `content_block_stop`.

- [ ] **Step 4.1:** Create fixture (blocking response):
  ```json
  {
    "id": "msg_01",
    "type": "message",
    "role": "assistant",
    "model": "claude-sonnet-4-5",
    "stop_reason": "end_turn",
    "content": [
      {"type": "thinking", "thinking": "Let me reason about this..."},
      {"type": "text", "text": "Final answer."}
    ],
    "usage": {"input_tokens": 50, "output_tokens": 30}
  }
  ```

- [ ] **Step 4.2:** Add `thinking` field to `ClaudeRequest` struct:
  ```rust
  #[derive(Serialize)]
  struct ClaudeThinkingConfig {
      #[serde(rename = "type")]
      kind: String,  // "enabled"
      budget_tokens: u32,
  }

  #[derive(Serialize)]
  struct ClaudeRequest {
      // ... existing ...
      #[serde(skip_serializing_if = "Option::is_none")]
      thinking: Option<ClaudeThinkingConfig>,
  }
  ```

- [ ] **Step 4.3:** Populate `thinking` from `options.thinking_budget_tokens` in both blocking (`chat_impl_with_timeout`) and streaming (`stream_impl`) request builders.

- [ ] **Step 4.4:** Auto-add header when thinking enabled:
  ```rust
  if options.thinking_budget_tokens.is_some() {
      req = req.header("anthropic-beta", "interleaved-thinking-2025-05-14");
  }
  ```
  Apply to BOTH the primary request builders AND the OAT haiku-fallback builders.

- [ ] **Step 4.5:** Extend `ContentBlock` enum to handle `Thinking`:
  ```rust
  #[derive(Serialize, Deserialize)]
  #[serde(tag = "type", rename_all = "snake_case")]
  enum ContentBlock {
      Text { text: String },
      ToolResult { ... },
      Image { source: ClaudeImageSource },
      Thinking { thinking: String },  // NEW
  }
  ```

- [ ] **Step 4.6:** Blocking response parser: collect `Thinking` blocks into `thinking_text: Option<String>` (concat with newlines if multiple, `None` if zero).

- [ ] **Step 4.7:** Streaming SSE parser: maintain a per-block-index thinking accumulator (similar to existing tool_use accumulator but `BTreeMap<usize, String>` for thinking text). On `content_block_delta` with `type: "thinking_delta"`, append `delta.thinking` to accumulator. On `content_block_stop`, emit `LlmStreamEvent::Thinking { text: <accumulated> }` and append to local `accumulated_thinking: String` for the terminal `End`.

- [ ] **Step 4.8:** Tests:
  - `thinking_block_in_blocking_response_populates_thinking_text` — fixture-based, asserts `LlmResponse.thinking_text == Some("Let me reason about this...")`.
  - `thinking_budget_appears_in_request_body` — wiremock + `body_partial_json`, asserts `{"thinking": {"type": "enabled", "budget_tokens": 8192}}`.
  - `thinking_budget_adds_beta_header` — wiremock asserts request had `anthropic-beta: interleaved-thinking-2025-05-14`.
  - `thinking_streaming_emits_thinking_event` — wiremock SSE chunked: `content_block_start type=thinking`, `content_block_delta type=thinking_delta`, `content_block_stop`. Assert at least one `LlmStreamEvent::Thinking` event followed by `End { total }` with `thinking_text.is_some()`.

- [ ] **Step 4.9:** Verify `cargo test --lib claude` and `cargo clippy --all-targets -- -D warnings` clean.

- [ ] **Step 4.10:** Commit: `feat(claude): extended thinking support (request + response + streaming)`.

---

### Task 5: claude_local — thinking from stream-json

**Files:** `src/claude_local.rs`, `tests/fixtures/fake_claude_thinking.sh` (new)

- [ ] **Step 5.1:** Inspect Claude Code CLI's stream-json format for thinking blocks. Likely shape:
  ```json
  {"type":"assistant","message":{"content":[{"type":"thinking","thinking":"..."}]}}
  ```
  (Same as HTTP Messages API — Claude CLI proxies the API.)

  If unsure, run `claude --help` or check existing repo notes. Worst case, log warn + skip (no behavior regression — old test fixtures don't have thinking blocks).

- [ ] **Step 5.2:** Add fixture:
  ```sh
  #!/bin/sh
  echo '{"type":"system","subtype":"init","model":"claude-sonnet-4-5"}'
  echo '{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"Reasoning step..."}]}}'
  echo '{"type":"assistant","message":{"content":[{"type":"text","text":"Final answer."}]}}'
  echo '{"type":"result","subtype":"success","total_cost_usd":0.001,"usage":{"input_tokens":50,"output_tokens":30}}'
  ```

- [ ] **Step 5.3:** Extend `run_stream_async` to handle thinking blocks: when an assistant content block has `type: "thinking"`, emit `LlmStreamEvent::Thinking { text }` and accumulate into a local `thinking_accumulator: String`. On End, populate `total.thinking_text = Some(...)` if accumulator is non-empty.

- [ ] **Step 5.4:** For the `--thinking-budget` CLI flag: check if Claude Code CLI accepts it. If yes, pass when `options.thinking_budget_tokens.is_some()`. If not, log warn ("claude_local: thinking_budget_tokens not supported by current CLI; ignored") and skip. This is verifiable by running `claude --help` once during implementation; if unclear, default to skipping with warn (safest).

- [ ] **Step 5.5:** Test using fixture: assert `LlmStreamEvent::Thinking` event emitted before final Delta/End, and `total.thinking_text == Some("Reasoning step...")`.

- [ ] **Step 5.6:** Verify, commit: `feat(claude_local): thinking block parsing from stream-json`.

---

### Task 6: codex — Responses API reasoning

**File:** `src/codex.rs`

OpenAI Responses API exposes reasoning in the `output` array as `{type: "reasoning", reasoning_content: "..."}` items (verify exact shape — may be `output_reasoning` or similar).

- [ ] **Step 6.1:** Inspect existing codex.rs response parser. The Responses API output array iteration already exists for tool calls. Add a branch for `type: "reasoning"`.

- [ ] **Step 6.2:** Map `LlmRequestOptions.thinking_budget_tokens` to existing `reasoning_effort`: if `thinking_budget_tokens.is_some()` and `reasoning_effort.is_none()`, set effort to `"high"`. Don't override an explicit `reasoning_effort`.

- [ ] **Step 6.3:** Blocking response: collect reasoning items → `thinking_text`. Text items continue → `rendered_text`.

- [ ] **Step 6.4:** Streaming: codex has no real SSE, uses Delta+End wrap. Pre-emit `Thinking { text }` (single event with full reasoning) before `Delta` (single event with full text) when reasoning present.

- [ ] **Step 6.5:** Add fixture `tests/fixtures/codex_reasoning_response.json`:
  ```json
  {
    "id": "resp_1",
    "model": "gpt-o4",
    "output": [
      {"type": "reasoning", "reasoning_content": "Step 1..."},
      {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "Final."}]}
    ],
    "usage": {"input_tokens": 50, "output_tokens": 30}
  }
  ```

- [ ] **Step 6.6:** Tests:
  - `reasoning_in_blocking_response_populates_thinking_text`
  - `thinking_budget_implies_high_reasoning_effort_when_unset`

- [ ] **Step 6.7:** Verify, commit: `feat(codex): reasoning_content extraction (thinking)`.

---

### Task 7: openai_compat — reasoning_content

**File:** `src/openai_compat.rs`

OpenAI Chat Completions API exposes reasoning on `choices[0].message.reasoning_content` (string) and in streaming via `choices[0].delta.reasoning_content` (string deltas).

- [ ] **Step 7.1:** Blocking response parser: extract `choices[0].message.reasoning_content` if present. If non-empty, set `LlmResponse.thinking_text = Some(...)`.

- [ ] **Step 7.2:** Streaming SSE parser: when `choices[0].delta.reasoning_content` is non-empty, emit `LlmStreamEvent::Thinking { text: reasoning_content }` and accumulate. On final chunk / `End`, populate `total.thinking_text`.

- [ ] **Step 7.3:** Add fixture + 2 tests (blocking + streaming SSE).

- [ ] **Step 7.4:** Verify, commit: `feat(openai_compat): reasoning_content extraction (thinking)`.

---

### Task 8: gemini — Gemini 3.x thinking parts

**File:** `src/gemini.rs`

Gemini 3.x exposes thinking via response parts. The implementer MUST consult the Gemini 3 API docs first to confirm exact JSON shape.

- [ ] **Step 8.1:** WebFetch Gemini 3 thinking API docs:
  ```
  WebFetch URL: https://ai.google.dev/gemini-api/docs/gemini-3
  prompt: "What is the JSON shape of thinking/reasoning content in a Gemini 3 response? Include field names and the streaming chunk format."
  ```
  Document findings inline before implementation. As of 2026-04, possible shapes:
  - `candidates[0].content.parts[]` with a `thought: true` flag on the part containing reasoning text.
  - Separate `thoughtSummary` field on candidate.
  - Per-chunk in streaming: same structure.

- [ ] **Step 8.2:** Map `LlmRequestOptions.reasoning_effort` (existing) to Gemini 3 `thinking_level`:
  - `"low"` → `thinking_level: "low"`
  - `"medium"` → `thinking_level: "medium"`
  - `"high"` → `thinking_level: "high"`
  - Otherwise omit (Gemini default).

  Add to `GeminiGenerationConfig` (or wherever generation_config is built).

- [ ] **Step 8.3:** Blocking response: extract thinking parts based on actual schema discovered in Step 8.1. Populate `thinking_text`.

- [ ] **Step 8.4:** Streaming: Gemini SSE chunks contain partial parts. Emit `LlmStreamEvent::Thinking` for thinking part chunks. Accumulate into `End.total.thinking_text`.

- [ ] **Step 8.5:** Tests with fixture (use the JSON shape discovered in Step 8.1).

- [ ] **Step 8.6:** Commit: `feat(gemini): Gemini 3.x thinking_level + thinking part extraction`.

---

## Phase 3: Anthropic Prompt Caching

### Task 9: claude HTTP — `cache_control` placement + cache token parsing

**File:** `src/claude.rs`

- [ ] **Step 9.1:** Define internal helper to apply cache_breakpoints to the request builder's existing `messages` array and `system` field.

  Pseudocode:
  ```rust
  fn apply_cache_breakpoints(
      system_block: &mut Value,        // mutable system block JSON
      messages: &mut Vec<Value>,        // mutable messages array
      breakpoints: &[CacheBreakpoint],
  ) {
      // Step 1: truncate to 4 with warn
      let effective: Vec<&CacheBreakpoint> = if breakpoints.len() > 4 {
          tracing::warn!(
              "claude: {} cache_breakpoints provided, Anthropic limit is 4; truncating",
              breakpoints.len()
          );
          breakpoints.iter().take(4).collect()
      } else {
          breakpoints.iter().collect()
      };

      for bp in effective {
          match bp {
              CacheBreakpoint::System => {
                  // Add cache_control to last system block
                  // Anthropic format: system blocks already are array; mark last one
                  // Or if system is single string, convert to array form first
                  ... insert {"cache_control": {"type": "ephemeral"}} on last system block
              }
              CacheBreakpoint::MessageIndex(idx) => {
                  if *idx >= messages.len() {
                      tracing::warn!(
                          "claude: cache_breakpoint MessageIndex({}) out of range (messages.len = {}); skipping",
                          idx, messages.len()
                      );
                      continue;
                  }
                  // Add cache_control to last content block of messages[idx]
                  // (or to the message itself if content is a string)
                  ... mutate messages[*idx]
              }
          }
      }
  }
  ```

  The exact JSON mutation depends on whether the existing message-builder uses the OLD string form or the NEW multipart array form. Inspect existing code first. Anthropic accepts both: simple string content (no cache_control possible inline) and array of blocks (cache_control per block).

  When we need to add cache_control, ensure the message uses the array-of-blocks form (convert if needed).

- [ ] **Step 9.2:** Apply this helper in BOTH blocking (`chat_impl_with_timeout`) and streaming (`stream_impl` / `send_streaming_request_with_timeout`) request paths. Apply only when `options.cache_breakpoints.is_empty() == false`.

- [ ] **Step 9.3:** Response parsing: extract `usage.cache_creation_input_tokens` and `usage.cache_read_input_tokens` (both optional u32). Populate `LlmResponse.cache_creation_input_tokens` and `cache_read_input_tokens`. Apply on both blocking and streaming end events.

- [ ] **Step 9.4:** Tests in `src/claude.rs::tests`:
  - `cache_breakpoint_system_appears_on_system_block` — wiremock + body_partial_json, send `cache_breakpoints: vec![CacheBreakpoint::System]`, assert request body's system block has `cache_control: {type: "ephemeral"}`.
  - `cache_breakpoint_message_index_appears_on_message` — same pattern, send `MessageIndex(0)`, assert `messages[0]` has cache_control on its last content block.
  - `cache_breakpoint_message_index_out_of_range_skipped` — send `MessageIndex(99)` with 1 message, assert NO cache_control appears in body, assert log contains "out of range".
  - `cache_breakpoints_truncated_at_4` — send 5 breakpoints, assert only 4 cache_controls appear in body, assert log contains "truncating".
  - `cache_token_counts_extracted_from_response` — fixture with `usage: {cache_creation_input_tokens: 100, cache_read_input_tokens: 200}`, assert `LlmResponse` fields populated.
  - `cache_options_ignored_by_other_providers` — sanity test on `gemini.rs` or `openai_compat.rs` that `cache_breakpoints: vec![CacheBreakpoint::System]` doesn't affect their request body.

- [ ] **Step 9.5:** Verify, commit: `feat(claude): Anthropic prompt caching via cache_breakpoints + cache token tracking`.

---

## Phase 4: Streaming Usage events

### Task 10: claude HTTP — `message_delta` Usage emission

**File:** `src/claude.rs`

Anthropic SSE `message_delta` event includes `usage.output_tokens` (cumulative). Emit `LlmStreamEvent::Usage` with throttle.

- [ ] **Step 10.1:** In `stream_impl` SSE parser, when a `message_delta` event arrives with `usage` field:
  - Extract `output_tokens` (always present).
  - Compare to last-emitted `output_tokens` (initialize to 0). If delta ≥ 50 OR this is the first usage event, emit `LlmStreamEvent::Usage`.
  - Compute `estimated_cost_usd` using existing claude pricing helper (`ClaudeProvider::estimate_cost(&model, input_tokens, output_tokens)`).

- [ ] **Step 10.2:** Track input_tokens from `message_start` event (Anthropic gives input_tokens in the initial message_start, doesn't update later).

- [ ] **Step 10.3:** Test: SSE chunked response with multiple `message_delta` events at output_tokens=10, 60, 100. Assert exactly 2 `Usage` events emitted (skipped first delta because <50 from baseline 0; emitted at 60 and 100). Verify costs are positive.

- [ ] **Step 10.4:** Commit: `feat(claude): emit LlmStreamEvent::Usage from message_delta with 50-token throttle`.

---

### Task 11: codex + openai_compat — final-chunk Usage

**Files:** `src/codex.rs`, `src/openai_compat.rs`

OpenAI streams send usage only in the final chunk before `[DONE]` (when `stream_options.include_usage` is set, which openai_compat already does).

- [ ] **Step 11.1:** In `openai_compat.rs::do_stream_with_timeout`: when an SSE chunk contains `usage` (typically the second-to-last chunk before `[DONE]`):
  - Extract `prompt_tokens`, `completion_tokens`, `total_tokens`.
  - Compute estimated cost (use existing pricing or `0.0` for unknown models).
  - Emit single `LlmStreamEvent::Usage` event before the final `End`.

- [ ] **Step 11.2:** In `codex.rs::chat_stream_with_options_dyn`: codex uses Delta+End wrap pattern. After reading the blocking response, emit `Usage` with the response's input_tokens/output_tokens BEFORE the final `End`.

- [ ] **Step 11.3:** Tests:
  - `openai_compat: usage_event_emitted_before_end` — SSE response with usage chunk + [DONE]. Assert exactly one `Usage` event in the stream, and it precedes the `End` event.
  - `codex: usage_event_emitted_before_end` — same shape via blocking response wrapped in stream.

- [ ] **Step 11.4:** Commit: `feat(codex,openai_compat): emit final-chunk LlmStreamEvent::Usage`.

---

### Task 12: gemini — per-chunk Usage

**File:** `src/gemini.rs`

Gemini SSE chunks include cumulative `usageMetadata`. Emit Usage with throttle.

- [ ] **Step 12.1:** In gemini streaming parser: when SSE chunk has `usageMetadata.totalTokenCount` (or per-component fields), update accumulator with `promptTokenCount` (input), `candidatesTokenCount` (output).

- [ ] **Step 12.2:** Apply 50-output-token throttle (same as claude).

- [ ] **Step 12.3:** Test: multi-chunk SSE response with usageMetadata at output=20, 80, 150. Assert 2 Usage events (at 80 and 150).

- [ ] **Step 12.4:** Commit: `feat(gemini): emit progressive LlmStreamEvent::Usage from usageMetadata`.

---

### Task 13: copilot — best-effort Usage (experimental)

**File:** `src/copilot.rs`

- [ ] **Step 13.1:** Copilot uses OpenAI-compatible schema. Apply Task 11's openai_compat pattern: final-chunk usage emission.

- [ ] **Step 13.2:** Document as experimental in module rustdoc (already marked experimental from v0.4.0 tool calling).

- [ ] **Step 13.3:** Test with mock SSE response.

- [ ] **Step 13.4:** Commit: `feat(copilot): experimental Usage event emission`.

---

## Phase 5: Examples + release

### Task 14: Write 3 example files

**Files:** `examples/thinking.rs`, `examples/prompt_caching.rs`, `examples/streaming_usage.rs`

- [ ] **Step 14.1:** `examples/thinking.rs`:
  ```rust
  //! Demonstrates separating Thinking events from Delta events
  //! (Anthropic extended thinking, OpenAI o-series reasoning).
  use llm_router::{ChatMessage, LlmRequestOptions, LlmStreamEvent, create_provider};
  use serde_json::json;
  use futures::StreamExt;

  #[tokio::main]
  async fn main() -> Result<(), Box<dyn std::error::Error>> {
      let provider = create_provider("dummy", "", "", "")?;  // dummy ignores; replace with claude/codex
      let opts = LlmRequestOptions {
          thinking_budget_tokens: Some(8192),
          ..Default::default()
      };
      let messages = vec![ChatMessage::user("Why is the sky blue?")];
      let mut stream = provider.chat_stream_with_options_dyn("", &messages, &opts, &json!({}));
      while let Some(event) = stream.next().await {
          match event {
              LlmStreamEvent::Thinking { text } => print!("[thinking] {text}"),
              LlmStreamEvent::Delta { text } => print!("{text}"),
              LlmStreamEvent::End { total } => println!("\n[done; thinking_text={:?}]", total.thinking_text.is_some()),
              LlmStreamEvent::Error { message } => eprintln!("[error] {message}"),
              _ => {}
          }
      }
      Ok(())
  }
  ```

- [ ] **Step 14.2:** `examples/prompt_caching.rs`:
  ```rust
  //! Demonstrates Anthropic prompt caching.
  use llm_router::{ChatMessage, CacheBreakpoint, LlmRequestOptions, create_provider};
  use serde_json::json;

  #[tokio::main]
  async fn main() -> Result<(), Box<dyn std::error::Error>> {
      let provider = create_provider("dummy", "", "claude-sonnet-4-5", "")?;  // replace with "claude" + key
      let opts = LlmRequestOptions {
          cache_breakpoints: vec![CacheBreakpoint::System, CacheBreakpoint::MessageIndex(0)],
          ..Default::default()
      };
      let long_system = "You are an expert assistant. ".repeat(200);
      let context = "Document context: ".to_string() + &"a".repeat(10_000);
      let messages = vec![
          ChatMessage::user(&context),  // RAG-style large context, cache after this
          ChatMessage::user("Now answer: what color is the sky?"),
      ];
      let resp = provider.chat_with_options_dyn(&long_system, &messages, &opts, &json!({})).await?;
      println!("{}", resp.rendered_text);
      println!("Cache creation tokens: {:?}", resp.cache_creation_input_tokens);
      println!("Cache read tokens: {:?}", resp.cache_read_input_tokens);
      Ok(())
  }
  ```

- [ ] **Step 14.3:** `examples/streaming_usage.rs`:
  ```rust
  //! Demonstrates live cost display during streaming.
  use llm_router::{ChatMessage, LlmStreamEvent, create_provider};
  use serde_json::json;
  use futures::StreamExt;

  #[tokio::main]
  async fn main() -> Result<(), Box<dyn std::error::Error>> {
      let provider = create_provider("dummy", "", "", "")?;  // replace
      let messages = vec![ChatMessage::user("Long answer please.")];
      let mut stream = provider.chat_stream_dyn("", &messages, &json!({}));
      while let Some(event) = stream.next().await {
          match event {
              LlmStreamEvent::Delta { text } => print!("{text}"),
              LlmStreamEvent::Usage { output_tokens, estimated_cost_usd, .. } => {
                  eprint!("\r[live: {} tokens, ${:.4}]", output_tokens, estimated_cost_usd);
              }
              LlmStreamEvent::End { total } => {
                  println!("\n[done: {} tokens total, ${:.4}]", total.output_tokens.unwrap_or(0), total.estimated_cost_usd);
              }
              _ => {}
          }
      }
      Ok(())
  }
  ```

- [ ] **Step 14.4:** Verify each example builds:
  ```sh
  cargo build --example thinking
  cargo build --example prompt_caching
  cargo build --example streaming_usage
  ```

- [ ] **Step 14.5:** Smoke test (uses dummy provider, doesn't crash):
  ```sh
  cargo run --example thinking 2>&1 | head
  cargo run --example prompt_caching 2>&1 | head
  cargo run --example streaming_usage 2>&1 | head
  ```

- [ ] **Step 14.6:** Commit: `docs(examples): add thinking, prompt_caching, streaming_usage examples`.

---

### Task 15: CHANGELOG + migration guide + version bump

**Files:** `CHANGELOG.md`, `docs/migration/v0.5.0.md` (new), `Cargo.toml`, `README.md`

- [ ] **Step 15.1:** Add `[0.5.0] - 2026-04-27` entry to CHANGELOG above `[0.4.1]`. Sections: Added (thinking, caching, streaming usage), Changed (LlmResponse non_exhaustive — single breaking item), Migration (link to guide).

- [ ] **Step 15.2:** Update CHANGELOG link footer:
  ```
  [Unreleased]: ...compare/v0.5.0...HEAD
  [0.5.0]: ...compare/v0.4.1...v0.5.0
  ```

- [ ] **Step 15.3:** Create `docs/migration/v0.5.0.md`:
  ```markdown
  # Migrating to llm-router v0.5.0

  Single breaking change + several additive features. Migration is mechanical
  and small for downstream consumers.

  ## Breaking change

  ### `LlmResponse` is now `#[non_exhaustive]`

  External `LlmResponse { ... }` struct literals stop compiling. Use
  `..Default::default()`:

  **Before**:
  ```rust
  LlmResponse {
      rendered_text: text,
      model: "claude".into(),
      estimated_cost_usd: 0.0,
      input_tokens: None,
      output_tokens: None,
      tool_calls: vec![],
  }
  ```

  **After**:
  ```rust
  LlmResponse {
      rendered_text: text,
      model: "claude".into(),
      ..Default::default()
  }
  ```

  This rarely affects downstream callers — typically only mock providers
  and test fixtures construct `LlmResponse` directly.

  ## Optional new features

  ### Thinking / reasoning blocks

  ```rust
  let opts = LlmRequestOptions {
      thinking_budget_tokens: Some(8192),  // Anthropic
      reasoning_effort: Some("high".into()),  // OpenAI o-series, Gemini 3.x
      ..Default::default()
  };
  let mut stream = provider.chat_stream_with_options_dyn(...);
  while let Some(event) = stream.next().await {
      match event {
          LlmStreamEvent::Thinking { text } => /* render in collapsed UI */,
          LlmStreamEvent::Delta { text } => /* render normally */,
          LlmStreamEvent::End { total } => /* total.thinking_text has full reasoning */,
          _ => {}
      }
  }
  ```

  ### Anthropic prompt caching (claude HTTP only)

  ```rust
  let opts = LlmRequestOptions {
      cache_breakpoints: vec![
          CacheBreakpoint::System,           // cache long system prompt
          CacheBreakpoint::MessageIndex(0),  // cache RAG context message
      ],
      ..Default::default()
  };
  let resp = provider.chat_with_options_dyn(&long_system, &messages, &opts, &json!({})).await?;
  println!("Cache hits: {:?}", resp.cache_read_input_tokens);
  ```

  Anthropic limits: max 4 cache_breakpoints (excess truncated with warn).
  Out-of-range MessageIndex silently skipped with warn.

  ### Streaming usage events

  ```rust
  while let Some(event) = stream.next().await {
      match event {
          LlmStreamEvent::Usage { output_tokens, estimated_cost_usd, .. } => {
              update_live_cost_display(output_tokens, estimated_cost_usd);
          }
          ...
      }
  }
  ```

  Cadence varies per provider:
  - Anthropic: progressive (every ≥50 output tokens)
  - Gemini: progressive (per SSE chunk, ≥50 throttle)
  - OpenAI/Codex/Copilot: single event before `End`
  - Local CLI: single event before `End`

  ## Validation

  ```sh
  # Update Cargo.toml: tag = "v0.5.0"
  cargo build
  cargo test
  ```
  ```

- [ ] **Step 15.4:** Bump `Cargo.toml`: `version = "0.4.1"` → `"0.5.0"`.

- [ ] **Step 15.5:** Update `README.md` installation tag `v0.4.1` → `v0.5.0`. Add brief mentions of thinking/caching/usage in feature list section.

- [ ] **Step 15.6:** Verify everything:
  ```sh
  cargo build --release
  cargo test --lib 2>&1 | tail -3
  cargo clippy --all-targets -- -D warnings
  cargo doc --no-deps 2>&1 | grep -iE 'warn|error'
  cargo build --examples
  ```

  Expected total tests: ~151 baseline + ~30 new = ~181 pass + 1 env-fail.

- [ ] **Step 15.7:** Commit: `chore(release): bump to v0.5.0`.

---

### Task 16: Final whole-branch review

- [ ] **Step 16.1:** Dispatch final code-reviewer subagent on `git diff main..HEAD` covering:
  - 5 providers' thinking implementation matches per-API specs.
  - Anthropic caching applies to system + messages, truncates to 4, skips invalid indices.
  - Streaming Usage events throttle correctly (50-token delta).
  - `LlmResponse` `#[non_exhaustive]` doesn't break any internal code.
  - All examples compile + smoke test runs.
  - Migration guide is actionable.
  - clippy + doc clean.

### Task 17: Tag + release (after user approval)

User authorization required (NEVER #2). Then:

```sh
git checkout main
git merge --ff-only feat/v0.5.0-thinking-caching-streaming-usage
git tag -a v0.5.0 -m "v0.5.0: thinking + Anthropic prompt caching + streaming usage"
git push origin main v0.5.0
gh release create v0.5.0 --notes-from-tag
```

After release: update wiki `Projects/llm-router/Releases/v0.5.0.md` + `History.md` + `Overview.md` "현재 상태" + `MAP.md` Releases list.

---

## Quality gates

After every task:
1. `cargo build --lib` clean.
2. `cargo test --lib` — no regression vs running baseline + new tests pass.
3. `cargo clippy --all-targets -- -D warnings` clean.

After Phase 5:
4. `cargo doc --no-deps` clean.
5. `cargo build --examples` clean.
6. Spec compliance review.
7. Code quality review.

## Estimated effort

- Phase 0: 5 min
- Phase 1: 1 day (type scaffolding + LlmResponse non_exhaustive + ~20 site updates)
- Phase 2: 3 days (thinking on 5 providers, claude HTTP being the most complex due to streaming SSE)
- Phase 3: 1 day (Anthropic caching)
- Phase 4: 1.5 days (Usage on 4 providers)
- Phase 5: 0.5 day (examples + docs + release prep)

**Total: ~7 working days** with TDD + per-task review loops.
