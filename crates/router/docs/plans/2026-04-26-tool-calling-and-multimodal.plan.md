# Tool Calling + Multimodal Implementation Plan (v0.4.0)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Each task's checkboxes (`- [ ]`) are for tracking.

**Goal:** Ship v0.4.0 with: native tool calling on 4 HTTP providers (claude / codex / gemini / openai_compat / copilot-experimental), HTTP image input on 4 providers, `ChatMessage` multipart refactor with constructor methods.

**Spec:** `docs/specs/2026-04-26-tool-calling-and-multimodal.md`

**Tech Stack:** Rust 1.85 (edition 2024). Adds `infer = "0.16"` dep for MIME magic-byte detection. No other new deps.

**Branch:** `feat/tool-calling-v0.4.0` (already checked out from `main` after v0.3.2 / commit `94e95b0`).

---

## Phase 0: Setup

### Task 0: Verify baseline + add `infer` dependency

**Files:** `Cargo.toml`, `Cargo.lock`

- [ ] **Step 0.1:** Confirm clean tree on `feat/tool-calling-v0.4.0` branch from main HEAD `94e95b0`.
- [ ] **Step 0.2:** Add `infer = "0.16"` to `[dependencies]` in `Cargo.toml`.
- [ ] **Step 0.3:** `cargo build --lib` — confirm compilation.
- [ ] **Step 0.4:** `cargo test --lib 2>&1 | tail -3` — record baseline (85 pass + 1 env-fail expected from v0.3.2).
- [ ] **Step 0.5:** Commit: `chore: add infer dependency for image MIME detection`.

---

## Phase 1: Type scaffolding (no behavior change)

This phase introduces all new types and refactors `ChatMessage` to multipart. Provider impls adapt to new shape using `text_content()` helper, but no new feature behavior yet. After Phase 1, `cargo test` should still pass with byte-identical observable behavior.

### Task 1: Add new tool / multimodal types

**File:** `src/lib.rs`

- [ ] **Step 1.1:** Add `ToolDef`, `ToolCall`, `ToolChoice` types (per spec § Function/API signatures). Derive `Debug, Clone`. `ToolCall` and `ToolDef` get `serde::Serialize, Deserialize` for fixture round-tripping in tests.
- [ ] **Step 1.2:** Add `ImageSource` enum with `#[non_exhaustive]`, `Base64(String)` and `Url(String)` variants.
- [ ] **Step 1.3:** Add `ChatContent` enum with `#[non_exhaustive]`, `Text(String)`, `Image { source: ImageSource, mime_type: String }`, `ToolResult { tool_use_id: String, content: String, is_error: bool }` variants.
- [ ] **Step 1.4:** Add `From<&str>` and `From<String>` impls for `ChatContent` (both → `Text` variant).
- [ ] **Step 1.5:** Add unit tests for each type's basic construction. `cargo test --lib` clean.
- [ ] **Step 1.6:** Commit: `feat(types): add ToolDef / ToolCall / ToolChoice / ChatContent / ImageSource`.

### Task 2: Refactor `ChatMessage` to multipart + constructors

**File:** `src/lib.rs` + 9 provider files (mechanical adaptation)

This is THE breaking change. After this commit, `ChatMessage { role, content }` literal syntax doesn't compile. Provider impls switch to `msg.text_content()`.

- [ ] **Step 2.1:** Change `ChatMessage` struct definition:
  ```rust
  #[derive(Clone)]
  #[non_exhaustive]
  pub struct ChatMessage {
      pub role: String,
      pub content: Vec<ChatContent>,
  }
  ```
- [ ] **Step 2.2:** Add constructor methods:
  - `pub fn user(text: impl Into<String>) -> Self`
  - `pub fn assistant(text: impl Into<String>) -> Self`
  - `pub fn user_with_image(text: impl Into<String>, image: ChatContent) -> Self`
  - `pub fn user_multipart(blocks: Vec<ChatContent>) -> Self`
  - `pub fn tool_result(tool_use_id: impl Into<String>, content: impl Into<String>, is_error: bool) -> Self`
  - `pub fn text_content(&self) -> String` — concatenates Text blocks; skips Image / ToolResult.
- [ ] **Step 2.3:** Update existing in-crate construction sites (`ChatMessage { role, content }` literals) to use constructors:
  - `src/dummy.rs`, `src/claude.rs`, `src/codex.rs`, `src/copilot.rs`, `src/gemini.rs`, `src/openai_compat.rs`, `src/claude_local.rs`, `src/codex_local.rs`, `src/gemini_local.rs`
  - All test modules in those files
  - Provider request body builders that currently read `msg.content` as `&str` → switch to `msg.text_content()`.
- [ ] **Step 2.4:** Update existing tests in `request_options_tests` and provider modules to use new constructors. Verify `cargo test --lib` still passes (85 + 1 env-fail).
- [ ] **Step 2.5:** Commit: `feat(types)!: refactor ChatMessage to multipart content`. The `!` indicates breaking change per Conventional Commits.

### Task 3: Extend `LlmStreamEvent` with `ToolCall` variant

**File:** `src/lib.rs`

- [ ] **Step 3.1:** Add `#[non_exhaustive]` to `LlmStreamEvent`.
- [ ] **Step 3.2:** Add new variant: `ToolCall { call: ToolCall }`.
- [ ] **Step 3.3:** Update existing in-crate matches (search `match.*LlmStreamEvent` across `src/`) to add `_ => {}` arms or handle the new variant explicitly. Most live inside `async_stream::stream! { ... }` blocks in providers — these don't match exhaustively, just yield specific variants.
- [ ] **Step 3.4:** Update test code in provider modules with exhaustive matches. Add `_ => unreachable!()` arms.
- [ ] **Step 3.5:** `cargo test --lib` clean.
- [ ] **Step 3.6:** Commit: `feat(stream)!: add ToolCall variant + #[non_exhaustive] on LlmStreamEvent`.

### Task 4: Extend `LlmRequestOptions` and `LlmResponse`

**File:** `src/lib.rs`

- [ ] **Step 4.1:** Add fields to `LlmRequestOptions`:
  - `pub tools: Vec<ToolDef>` (default empty Vec)
  - `pub tool_choice: Option<ToolChoice>` (default None)
- [ ] **Step 4.2:** Add field to `LlmResponse`:
  - `pub tool_calls: Vec<ToolCall>` (default empty Vec)
- [ ] **Step 4.3:** Update `LlmResponse` construction sites in all 9 providers to add `tool_calls: vec![]` (or implement `Default` for `LlmResponse` and use `..Default::default()`). Recommended: add `#[derive(Default)]` and use `..Default::default()`.

  Note: `String` already implements `Default` (returns `""`), `Option<u32>` defaults to `None`, `f64` defaults to `0.0`, `Vec<ToolCall>` defaults to `vec![]`. `#[derive(Default)]` works.
- [ ] **Step 4.4:** Update existing `request_options_tests::default_options_are_empty` and `response_token_tests` to assert new fields default correctly.
- [ ] **Step 4.5:** `cargo test --lib` clean.
- [ ] **Step 4.6:** Commit: `feat(types): add tools/tool_choice options + tool_calls response field`.

---

## Phase 2: Tool calling per HTTP provider

Each task adds tool encoding (request) + parsing (response, blocking + streaming) for one provider. Tests use wiremock with recorded JSON fixtures.

### Task 5: claude tool calling

**Files:** `src/claude.rs`, `tests/fixtures/claude_tool_use_response.json` (new)

- [ ] **Step 5.1:** Add fixture `tests/fixtures/claude_tool_use_response.json` — minimal Anthropic Messages response with `content: [{type: "tool_use", id: "toolu_01", name: "get_weather", input: {city: "seoul"}}]`.
- [ ] **Step 5.2:** Encode `tools` array in request body. Anthropic format:
  ```json
  "tools": [{"name": "...", "description": "...", "input_schema": {...}}]
  ```
  Skip if `options.tools.is_empty()`.
- [ ] **Step 5.3:** Encode `tool_choice` in request body. Anthropic format:
  ```json
  "tool_choice": {"type": "auto" | "any" | "tool", "name"?: "..."}
  ```
  Map: `Auto` → `{type: "auto"}`, `Required` → `{type: "any"}`, `Specific(s)` → `{type: "tool", name: s}`, `None` → omit field (Anthropic default is auto unless tools provided).
- [ ] **Step 5.4:** Encode `ChatContent::ToolResult` in messages: as a content block of `{type: "tool_result", tool_use_id, content, is_error}` inside a user-role message.
- [ ] **Step 5.5:** Parse `tool_use` blocks in non-streaming response. Each block → `ToolCall { id, name, arguments }`. Append to `LlmResponse.tool_calls`.
- [ ] **Step 5.6:** Parse streaming SSE: `content_block_start` with `type: "tool_use"` opens an accumulator keyed by `index`; `content_block_delta` with `input_json_delta` accumulates partial JSON; `content_block_stop` finalizes — parse accumulated JSON into `arguments` and emit `LlmStreamEvent::ToolCall { call }`.
- [ ] **Step 5.7:** Tests:
  - `tool_use_in_blocking_response_returns_tool_calls` (wiremock with fixture)
  - `tool_use_in_streaming_response_emits_toolcall_event` (wiremock SSE chunked)
  - `tool_choice_required_appears_in_request_body`
  - `tool_result_block_serializes_correctly`
- [ ] **Step 5.8:** Commit: `feat(claude): tool calling (request encoding + response parsing)`.

### Task 6: openai_compat tool calling

**Files:** `src/openai_compat.rs`, `tests/fixtures/openai_tool_call_response.json`

- [ ] **Step 6.1:** Add fixture with OpenAI-style `tool_calls` in `choices[0].message.tool_calls`:
  ```json
  {"choices": [{"message": {"role": "assistant", "tool_calls": [{"id": "call_1", "type": "function", "function": {"name": "get_weather", "arguments": "{\"city\":\"seoul\"}"}}]}}]}
  ```
- [ ] **Step 6.2:** Encode `tools` and `tool_choice` in request body (OpenAI format):
  ```json
  "tools": [{"type": "function", "function": {"name": "...", "description": "...", "parameters": {...}}}]
  "tool_choice": "auto" | "required" | "none" | {"type": "function", "function": {"name": "..."}}
  ```
- [ ] **Step 6.3:** Encode `ChatContent::ToolResult` as a SEPARATE message (not a content block) with `role: "tool"` and `tool_call_id`. This is OpenAI's wire format quirk vs Anthropic.
- [ ] **Step 6.4:** Parse `tool_calls` array in `choices[0].message`. `function.arguments` is a JSON-encoded string → parse to `serde_json::Value`. → `LlmResponse.tool_calls`.
- [ ] **Step 6.5:** Streaming: SSE `delta.tool_calls` deltas with `index`, partial `function.arguments`. Accumulate per-index, emit `ToolCall` event when `finish_reason: "tool_calls"` arrives or stream ends with non-empty accumulator.
- [ ] **Step 6.6:** Tests:
  - `tool_call_in_blocking_response_returns_tool_calls`
  - `tool_call_in_streaming_response_emits_toolcall_event`
  - `tool_choice_required_appears_in_request_body`
  - `tool_result_message_role_serializes_correctly`
- [ ] **Step 6.7:** Commit: `feat(openai_compat): tool calling support`.

### Task 7: codex tool calling

**Files:** `src/codex.rs`

- [ ] **Step 7.1:** Confirm whether `chatgpt.com/backend-api/codex/responses` endpoint accepts standard OpenAI `tools` schema. If yes (likely), reuse openai_compat encoding logic. If divergent, document deviation in spec doc + adapt.
- [ ] **Step 7.2:** Apply same encoding/parsing pattern as openai_compat. Tests parallel.
- [ ] **Step 7.3:** Commit: `feat(codex): tool calling support`.

### Task 8: gemini tool calling

**Files:** `src/gemini.rs`, `tests/fixtures/gemini_function_call_response.json`

- [ ] **Step 8.1:** Add fixture with Gemini's `functionCall` part:
  ```json
  {"candidates": [{"content": {"parts": [{"functionCall": {"name": "get_weather", "args": {"city": "seoul"}}}]}}]}
  ```
- [ ] **Step 8.2:** Encode `tools` in request body (Gemini format):
  ```json
  "tools": [{"functionDeclarations": [{"name": "...", "description": "...", "parameters": {...}}]}]
  ```
  Note: single-element outer array, all functions in one `functionDeclarations` array inside it.
- [ ] **Step 8.3:** Encode `tool_choice`: Gemini uses `toolConfig.functionCallingConfig.mode` (`AUTO` | `ANY` | `NONE`) and optional `allowedFunctionNames`.
- [ ] **Step 8.4:** Encode `ChatContent::ToolResult` as a part with `functionResponse: { name, response: { content } }`. Note: Gemini uses `name` rather than `tool_use_id` to pair, so the encoder needs to look up the call name from the original `ToolCall` — for v0.4.0, store `name` alongside `tool_use_id` or pass through. Actually inspect: `ChatContent::ToolResult { tool_use_id, content, is_error }` — `tool_use_id` is the call id, but Gemini wants the `name`. Decision: caller must pass `name` for Gemini. Easiest: extend `ChatContent::ToolResult` to optionally include `name` for cross-provider portability, OR caller chooses provider-appropriate construction. Default to using `tool_use_id` as `name` for Gemini, document that for Gemini callers should construct `tool_use_id` = function name. Document this gotcha.

  **Alternative (preferred):** Generate a synthetic name by parsing it from the `tool_use_id` if it has form `<name>:<id>`. Document this convention.

  Actually best path: keep `ToolResult { tool_use_id, content, is_error }` simple; for Gemini, the caller passes the function name as `tool_use_id`. Gemini itself doesn't generate stable IDs anyway. Document.
- [ ] **Step 8.5:** Parse `functionCall` parts → `ToolCall { id: name, name, arguments: args }`. ID == name for Gemini (no separate id concept).
- [ ] **Step 8.6:** Streaming: Gemini SSE chunks contain partial `parts`. Accumulate `functionCall.args` JSON across chunks if they arrive partial. Emit `ToolCall` when `finishReason: "STOP"` with completed function call.
- [ ] **Step 8.7:** Tests parallel to claude/openai_compat. Plus a test asserting the Gemini-specific `ToolResult` encoding (using `tool_use_id` as `name`).
- [ ] **Step 8.8:** Commit: `feat(gemini): tool calling support`.

### Task 9: copilot tool calling (experimental)

**Files:** `src/copilot.rs`

- [ ] **Step 9.1:** Add module-level rustdoc note: `//! **EXPERIMENTAL**: tool calling support is implemented against the OpenAI-compatible schema, but real-world Copilot Chat tool support varies by tier and is not publicly documented. Behavior with the live Copilot endpoint is not guaranteed; use at your own risk.`
- [ ] **Step 9.2:** Apply same encoding/parsing pattern as openai_compat (likely identical schema, copy logic).
- [ ] **Step 9.3:** Tests with mock only (no real-endpoint validation).
- [ ] **Step 9.4:** Commit: `feat(copilot): experimental tool calling support`.

---

## Phase 3: HTTP image input + helpers

### Task 10: claude HTTP image input

**File:** `src/claude.rs`

- [ ] **Step 10.1:** Encode `ChatContent::Image` as Anthropic content block:
  ```json
  {"type": "image", "source": {"type": "base64" | "url", "media_type": "image/png", "data" | "url": "..."}}
  ```
  Map `ImageSource::Base64(data)` → `source.type: "base64"`, `source.data: data`. `ImageSource::Url(url)` → `source.type: "url"`, `source.url: url` (Anthropic added URL support in 2024).
- [ ] **Step 10.2:** Test: send `ChatMessage::user_with_image("describe", ChatContent::Image { source: Base64(...), mime_type: "image/png" })`, assert request body contains expected image block.
- [ ] **Step 10.3:** Commit: `feat(claude): HTTP image input via ChatContent::Image`.

### Task 11: openai_compat HTTP image input

**File:** `src/openai_compat.rs`

- [ ] **Step 11.1:** Encode `ChatContent::Image` as OpenAI content block:
  ```json
  {"type": "image_url", "image_url": {"url": "data:image/png;base64,<DATA>" | "<URL>"}}
  ```
  For `Base64(data)`, build data URL: `format!("data:{};base64,{}", mime_type, data)`. For `Url(url)`, use as-is.
- [ ] **Step 11.2:** Test mirrors claude test pattern.
- [ ] **Step 11.3:** Commit: `feat(openai_compat): HTTP image input`.

### Task 12: gemini HTTP image input

**File:** `src/gemini.rs`

- [ ] **Step 12.1:** Encode `ChatContent::Image` as Gemini part:
  - `Base64(data)` → `{inlineData: {mimeType, data}}`
  - `Url(url)` → log warning + skip (Gemini URL needs Files API upload first; out of scope for v0.4.0). Caller must use Base64.
- [ ] **Step 12.2:** Document URL limitation in module rustdoc.
- [ ] **Step 12.3:** Test asserts Base64 encoding works; URL emits warning + skips.
- [ ] **Step 12.4:** Commit: `feat(gemini): HTTP image input (base64 only; URL warns)`.

### Task 13: codex / copilot HTTP image input

**Files:** `src/codex.rs`, `src/copilot.rs`

- [ ] **Step 13.1:** Apply openai_compat pattern to both. Likely identical encoding.
- [ ] **Step 13.2:** Tests. Single commit.
- [ ] **Step 13.3:** Commit: `feat(codex,copilot): HTTP image input`.

### Task 14: `ChatContent` image helpers (path + auto)

**File:** `src/lib.rs` (or new `src/image_utils.rs` if cleaner)

- [ ] **Step 14.1:** Implement `ChatContent::image_from_path(path: impl AsRef<Path>) -> Result<ChatContent, std::io::Error>`:
  1. Read file bytes.
  2. Base64 encode (use `base64` crate already in tree? Check; if not, add `base64 = "0.22"` — minor dep). Actually reqwest brings base64 transitively; check `cargo tree`. If not directly accessible, add as dep.
  3. Infer MIME from extension via simple match (png/jpg/jpeg/gif/webp). For unknown extension, default to `application/octet-stream` and let provider reject.
  4. Return `ChatContent::Image { source: Base64(encoded), mime_type }`.
- [ ] **Step 14.2:** Implement `ChatContent::image_from_base64_auto(data: impl Into<String>) -> Result<ChatContent, &'static str>`:
  1. Decode base64 first 32 bytes (enough for any image magic byte).
  2. Use `infer::Infer::new().get(decoded)` to detect MIME.
  3. If detected, return `Image { source: Base64(data), mime_type: detected.mime_type() }`.
  4. If not detected, return `Err("could not detect image MIME from base64 data")`.
- [ ] **Step 14.3:** Tests:
  - `image_from_path_with_png_extension_succeeds` (use a tiny PNG fixture in `tests/fixtures/test_image.png`)
  - `image_from_path_with_unknown_extension_uses_octet_stream`
  - `image_from_base64_auto_detects_png`
  - `image_from_base64_auto_returns_err_for_garbage`
- [ ] **Step 14.4:** Commit: `feat(image): image_from_path + image_from_base64_auto helpers`.

---

## Phase 4: Local CLI provider adaptation

### Task 15: claude_local / codex_local / gemini_local — multipart consumption

**Files:** `src/claude_local.rs`, `src/codex_local.rs`, `src/gemini_local.rs`

This is mechanical adaptation. The local CLI providers built their prompts from `msg.content` (was String). Switch to `msg.text_content()`. For non-text blocks:

- `ChatContent::Image { source, mime_type }`:
  - If `Base64(data)`: write to a temp file (use `tempfile` crate? If not in deps, decide — could use `std::env::temp_dir()` + UUID filename), then prepend path to existing `image_paths` flow. Document temp-file lifecycle.
  - If `Url(url)`: log warning + skip (CLI doesn't fetch URLs; would need pre-download).
- `ChatContent::ToolResult { ... }`: `tracing::debug!` log noting drop, then skip from prompt construction.

- [ ] **Step 15.1:** claude_local — adapt `chat_dyn` / `chat_with_options_dyn` / streaming path's prompt builder to handle multipart.
- [ ] **Step 15.2:** codex_local — same.
- [ ] **Step 15.3:** gemini_local — same.
- [ ] **Step 15.4:** Add `tempfile = "3"` dep if not present; or use simpler approach: write to `std::env::temp_dir().join(uuid)` + best-effort cleanup on drop (a `TempFile` newtype with Drop impl).
- [ ] **Step 15.5:** Tests: each `_local` provider gets a test confirming multipart inputs are handled gracefully (no panic, no error).
- [ ] **Step 15.6:** Commit: `feat(local): adapt to multipart ChatMessage; drop tool_result blocks`.

---

## Phase 5: Release prep

### Task 16: README + migration guide + CHANGELOG + version bump

**Files:** `README.md`, `docs/migration/v0.4.0.md` (new), `CHANGELOG.md`, `Cargo.toml`

- [ ] **Step 16.1:** Write `docs/migration/v0.4.0.md` covering:
  - `ChatMessage` literal → constructor migration with sed-friendly patterns.
  - `LlmStreamEvent` exhaustive match → add `_` arm.
  - New tool calling capability (example).
  - New HTTP image input capability (example).
  - Per-provider notes (Gemini ToolResult name convention, Copilot experimental).
- [ ] **Step 16.2:** Update `README.md`:
  - Quick Start uses new `ChatMessage::user("hello")` constructor.
  - Add "Tool Calling" section with full round-trip example (define tool → call → execute → tool_result → next call).
  - Add "Multimodal Input" section with image example.
  - Update Per-Request Options matrix to include `tools` / `tool_choice`.
  - Bump installation tag to `v0.4.0`.
- [ ] **Step 16.3:** Update `CHANGELOG.md` with `[0.4.0] - 2026-04-26` entry. Sections: Added, Changed, Migration, Breaking. Reference `docs/migration/v0.4.0.md`.
- [ ] **Step 16.4:** `Cargo.toml`: bump `version = "0.3.2"` → `"0.4.0"`.
- [ ] **Step 16.5:** Validation:
  - `cargo build --lib` clean.
  - `cargo test --lib` — total: previous 85 + many new tests (~25-30 new from Phases 1-3). Expect ~110-115 pass + 1 env-fail.
  - `cargo clippy --all-targets -- -D warnings` clean.
  - `cargo doc --no-deps` clean.
  - `cargo run --example minimal` — confirm example still works (will need updating to new `ChatMessage::user` constructor; do that as part of this task).
- [ ] **Step 16.6:** Commit: `chore(release): bump to v0.4.0`.

### Task 17: Final whole-branch review

- [ ] **Step 17.1:** Dispatch final code-reviewer subagent on `git diff main..HEAD` covering:
  - All 9 providers compile + adapt to new ChatMessage.
  - 4 HTTP providers (claude/codex/gemini/openai_compat) plus copilot (experimental) honor `tools` and emit `ToolCall` events.
  - 4 HTTP providers honor `ChatContent::Image`.
  - Local CLI providers gracefully skip non-text blocks with debug logging.
  - `text_content()` helper used consistently for legacy text-only paths.
  - No new `LlmError` variants; all failures route through existing taxonomy.
  - clippy + doc clean.
  - Migration guide accurately covers all breaking changes.

### Task 18: Tag + release (after user approval)

User authorization required (NEVER #2). Then:

```bash
git checkout main
git merge --ff-only feat/tool-calling-v0.4.0
git tag -a v0.4.0 -m "v0.4.0: tool calling + multimodal + ChatMessage refactor"
git push origin main v0.4.0
gh release create v0.4.0 --notes-from-tag
```

After release: update wiki `Projects/llm-router/Overview.md` (현재 상태 섹션), `History.md` (날짜 entry), and create `Releases/v0.4.0.md`.

---

## Quality gates

After every task:
1. `cargo build --lib` clean.
2. `cargo test --lib` — no regression vs current baseline + new tests added by the task pass.
3. `cargo clippy --lib -- -D warnings` clean.

After Phase 5:
4. `cargo doc --no-deps` clean.
5. Spec compliance review (subagent).
6. Code quality review (subagent).
7. Migration guide manually walked-through against one downstream project's expected migration shape.

## Estimated effort

- Phase 0: 15 min (setup)
- Phase 1: 1.5 days (type scaffolding + ChatMessage refactor — biggest single change)
- Phase 2: 3 days (5 providers × tool calling, with claude + gemini being the most complex)
- Phase 3: 1.5 days (4 providers × image input + helpers)
- Phase 4: 0.5 day (local provider adaptation, mechanical)
- Phase 5: 0.5 day (docs + version + final review)

**Total: ~7 working days** with TDD discipline + per-task review loops. Possibly 2 weeks with normal pace including non-implementation overhead.
