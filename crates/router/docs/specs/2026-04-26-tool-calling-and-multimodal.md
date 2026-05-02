# Tool Calling + Multimodal Input + ChatMessage Refactor (v0.4.0)

> **Status: draft**

## Purpose

Add native tool calling and HTTP provider image input to llm-router. Both require the existing `ChatMessage { role: String, content: String }` to evolve into a multipart-content shape. **Done when:** (a) `LlmRequestOptions { tools: vec![ToolDef { ... }], ..default() }` makes Anthropic / OpenAI / Gemini / openai_compat emit `tool_calls` on the response or `LlmStreamEvent::ToolCall` events on the stream; (b) `ChatMessage::user_with_image(text, image)` round-trips through `claude` / `gemini` / `openai_compat` HTTP requests; (c) all 4 existing downstream projects (kangnam-client, travel-planner, Lunawave, dear-jeongbin) continue to compile after applying the documented migration steps.

## Motivation

Modern LLM apps treat tool calling as table-stakes. Anthropic Messages API, OpenAI Chat Completions, Gemini Generative Language all support it natively — llm-router does not expose it. Without this, downstream projects either work around llm-router (using lower-level HTTP clients directly) or wait. Same for HTTP image input: today `LlmRequestOptions.image_paths` is `_local` CLI only; HTTP providers can't accept images at all.

Both features need messages that carry more than a single string per turn. `tool_use` blocks (model output), `tool_result` blocks (caller feedback for the next turn), and `image` blocks all require a multipart structure. So `ChatMessage` itself must change.

## Non-Goals

Per handoff doc (out of scope, NEVER add):
- **Tool execution** — caller receives `ToolCall`, executes itself, sends back `ToolResult` in next turn.
- **Permission dialogs** — caller's UI concern.
- **Multi-turn agent loop** — caller orchestrates the `tool_use → execute → tool_result → next turn` loop.
- **Subagents, hooks** — agent-framework concerns, not LLM-client concerns.

Also out of scope for v0.4.0 (deferred):
- **Thinking/reasoning block separation** (Claude 4 extended thinking, o1/o3) — defer to v0.4.1+ when downstream UI actually distinguishes.
- **Streaming `ToolCall` partial JSON deltas** — initial implementation accumulates the full `tool_use` block server-side and emits one `LlmStreamEvent::ToolCall` event after the block completes. Per-token JSON streaming via `input_json_delta` deferred to v0.5+.
- **Provider-specific tool features** — Anthropic computer use tools, OpenAI parallel tool calls flags, Gemini code execution. Generic schema only for now.
- **Tool definition validation** — JSON Schema is passed through verbatim. Caller responsible for valid schema.

## Architecture

### New types (additions)

```rust
/// Tool / function definition the model can call.
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,  // JSON Schema (Draft 2020-12)
}

/// A tool invocation produced by the model.
pub struct ToolCall {
    pub id: String,                      // Provider-assigned call id, used to pair with ToolResult
    pub name: String,
    pub arguments: serde_json::Value,    // Parsed from model output
}

/// Strategy for forcing tool use.
pub enum ToolChoice {
    Auto,                  // Model decides
    Required,              // Model must call at least one tool
    None,                  // Disable tool calling for this call
    Specific(String),      // Force a specific tool by name
}

/// One block of a multipart message.
#[non_exhaustive]
pub enum ChatContent {
    Text(String),
    Image { source: ImageSource, mime_type: String },
    ToolResult { tool_use_id: String, content: String, is_error: bool },
}

/// Source of an image attachment.
#[non_exhaustive]
pub enum ImageSource {
    /// Pre-encoded base64 (caller responsible for encoding).
    Base64(String),
    /// Public URL the provider fetches.
    Url(String),
}
```

### `ChatMessage` refactor (the breaking change)

**Before** (v0.3.x):
```rust
#[derive(Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}
```

**After** (v0.4.0):
```rust
#[derive(Clone)]
#[non_exhaustive]
pub struct ChatMessage {
    pub role: String,
    pub content: Vec<ChatContent>,
}

impl ChatMessage {
    /// User turn with plain text. Equivalent to v0.3.x `ChatMessage { role: "user", content: text }`.
    pub fn user(text: impl Into<String>) -> Self;

    /// Assistant turn with plain text.
    pub fn assistant(text: impl Into<String>) -> Self;

    /// User turn with text and one image.
    pub fn user_with_image(text: impl Into<String>, image: ChatContent) -> Self;

    /// Multi-block user turn (text + N images, etc.).
    pub fn user_multipart(blocks: Vec<ChatContent>) -> Self;

    /// Tool result feedback (from caller back to model).
    /// `role` is set to "user" — Anthropic and OpenAI both expect tool_result blocks
    /// inside a user message.
    pub fn tool_result(tool_use_id: impl Into<String>, content: impl Into<String>, is_error: bool) -> Self;

    /// Best-effort extract concatenated text content. Convenience for legacy code paths.
    /// Skips Image and ToolResult blocks.
    pub fn text_content(&self) -> String;
}

/// Convenience for back-compat: `&str` → `ChatContent::Text(s.to_string())`.
impl From<&str> for ChatContent { fn from(s: &str) -> Self { ChatContent::Text(s.into()) } }
impl From<String> for ChatContent { fn from(s: String) -> Self { ChatContent::Text(s) } }
```

**Why `#[non_exhaustive]` on `ChatMessage`?** Future fields (e.g. message-level metadata) become non-breaking. Cost: external crates can no longer use struct literal syntax — they MUST use constructors. This forces the migration to constructor-based code.

**Why `Vec<ChatContent>` not `Either<String, Vec<ChatContent>>`?** Single source of truth. Callers using `ChatMessage::user("hi")` get `vec![Text("hi")]` internally — same shape as multipart. No type-level branching in providers.

### Migration: v0.3.x → v0.4.0 caller code

| Before | After |
| :--- | :--- |
| `ChatMessage { role: "user".into(), content: "hi".into() }` | `ChatMessage::user("hi")` |
| `ChatMessage { role: "assistant".into(), content: text }` | `ChatMessage::assistant(text)` |
| `msg.content` (read as String) | `msg.text_content()` |
| `match event { Delta { text } => …, End { .. } => …, Error { .. } => … }` | Add `_ => unreachable!()` arm or handle new variants |

Mechanical, sed-able. Estimated 30 min - 1 hr per downstream project (5-20 sites).

### `LlmStreamEvent` extension

```rust
#[non_exhaustive]
pub enum LlmStreamEvent {
    Delta { text: String },
    End { total: LlmResponse },
    Error { message: String },
    /// Model produced a tool call. Emitted after the tool_use block fully accumulates
    /// (no partial-JSON streaming in v0.4.0). Multiple `ToolCall` events per stream
    /// are possible (Anthropic supports parallel tool use).
    ToolCall { call: ToolCall },
}
```

`#[non_exhaustive]` is the breaking part — downstream `match` arms without `_` or all-variants coverage break.

### `LlmRequestOptions` extensions

```rust
pub struct LlmRequestOptions {
    // ... existing v0.3.2 fields ...

    /// Tool / function definitions the model may call. Empty = no tools.
    /// Honored by HTTP providers (claude, codex, gemini, openai_compat).
    /// Local CLI providers ignore.
    pub tools: Vec<ToolDef>,

    /// Strategy for forcing tool use. None = provider default (Auto).
    pub tool_choice: Option<ToolChoice>,
}
```

### `LlmResponse` extension

```rust
pub struct LlmResponse {
    // ... existing fields ...
    /// Tool calls produced by the model on this response. Empty if model produced
    /// only text. For streaming, these are accumulated and exposed on the terminal
    /// `End { total }` event.
    pub tool_calls: Vec<ToolCall>,
}
```

`Default::default()` returns empty Vec — back-compat for tests / construction sites that didn't previously build with these fields.

## File changes

- `Cargo.toml` — bump `version` 0.3.2 → 0.4.0. Add dependency `infer = "0.16"` for MIME magic-byte detection in `ChatContent::image_from_base64_auto` helper.
- `src/lib.rs`:
  - New types: `ToolDef`, `ToolCall`, `ToolChoice`, `ChatContent`, `ImageSource`.
  - `ChatMessage`: refactor to `Vec<ChatContent>` + constructor methods + `From<&str>` / `From<String>` for `ChatContent`.
  - `LlmStreamEvent`: add `#[non_exhaustive]` + `ToolCall` variant.
  - `LlmRequestOptions`: add `tools`, `tool_choice` fields.
  - `LlmResponse`: add `tool_calls` field.
  - Update existing test modules to use new constructors.
- `src/claude.rs` — Anthropic Messages API:
  - Encode `ChatContent::Image` as `{type: "image", source: {type: "base64"|"url", media_type, data|url}}`.
  - Encode `ChatContent::ToolResult` as `{type: "tool_result", tool_use_id, content, is_error}`.
  - Add `tools: [...]` and `tool_choice: {type: ...}` to request body when present.
  - Parse `tool_use` content blocks in non-streaming response → `LlmResponse.tool_calls`.
  - Parse `content_block_start` (type=tool_use) + `input_json_delta` accumulation in streaming SSE → emit `LlmStreamEvent::ToolCall` on `content_block_stop`.
- `src/codex.rs` — same as claude.rs but for `chatgpt.com/backend-api/codex/responses` endpoint. May share encoding logic with `openai_compat.rs` if schema matches.
- `src/gemini.rs` — Google Generative Language API:
  - Encode `ChatContent::Image` as `inlineData: { mimeType, data }` (base64) or `fileData: { fileUri }` (URL — note: Gemini URL support requires Files API upload first; v0.4.0 emits warning if URL provided, treats as Base64-only for safety).
  - Encode `ChatContent::ToolResult` as `functionResponse: { name, response: { content } }`.
  - Add `tools: [{ functionDeclarations: [...] }]` to request body.
  - Parse `functionCall` parts in response → `LlmResponse.tool_calls`.
- `src/openai_compat.rs` — OpenAI Chat Completions standard:
  - Encode `ChatContent::Image` as `{type: "image_url", image_url: {url: "data:<mime>;base64,<data>"|<url>}}`.
  - Encode `ChatContent::ToolResult` as a separate message with `role: "tool"` and `tool_call_id` (OpenAI's wire format differs from Anthropic — handle in encoder).
  - Add `tools` and `tool_choice` to request body.
  - Parse `tool_calls` in `choices[0].message` → `LlmResponse.tool_calls`.
  - Streaming: accumulate `tool_calls` deltas across SSE chunks → emit `LlmStreamEvent::ToolCall` on completion.
- `src/copilot.rs` — same as openai_compat (OpenAI-style schema). Add tool calling marked **experimental** in module-level rustdoc: real-world Copilot Chat tool support varies by tier and isn't publicly documented. Tests use mock; production behavior with real Copilot endpoint not guaranteed.
- `src/dummy.rs` — accept new `ChatContent` blocks but ignore them; emit empty `tool_calls` always. Confirms compile-only contract.
- `src/claude_local.rs`, `src/codex_local.rs`, `src/gemini_local.rs`:
  - Adapt to new `ChatMessage` structure: extract text via `text_content()` for CLI prompt construction.
  - Image attachments in `ChatContent::Image` blocks → use existing `image_paths` flow if `ImageSource::Base64` is unsupported by the CLI (write to temp file and pass path), or skip with warning. Document behavior precisely.
  - `ToolResult` blocks → ignored (CLI doesn't expose explicit tool feedback). Caller using local CLI provider is implicitly using CLI's own agent loop, not the API tool flow.
  - No tool calling exposure on `_local` (CLI's tool use is internal to the CLI). Document.
- `tests/fixtures/`: add JSON fixtures for tool_use response shapes per provider.
- `CHANGELOG.md` — `[0.4.0]` entry with Added/Changed/Migration/Breaking sections.
- `README.md` — major rewrite of relevant sections (Quick Start uses new constructors; Per-Request Options shows tools; new Multimodal section).
- `docs/migration/v0.4.0.md` — new migration guide for downstream projects.

## Implementation order

Each step is an independent commit. Each step has its own task in the implementation plan.

1. **Type scaffolding** — new types + `ChatMessage` refactor + tests (compile-only checks). All 5 provider modules' `chat_*_dyn` impls update to consume the new `ChatMessage` shape (using `text_content()` initially) — no new feature behavior yet, just shape change. **Cargo.toml stays at 0.3.x for this commit** to keep partial-build snapshots clean.
2. **`LlmStreamEvent::ToolCall` variant + `#[non_exhaustive]`** — additive enum work. Dummy provider passes through. Tests confirm compile and that existing match arms in tests now have `_` arms.
3. **`LlmRequestOptions.tools` / `tool_choice` + `LlmResponse.tool_calls` field plumbing** — type additions, no provider behavior yet. Tests confirm Defaults are empty.
4. **claude tool calling** — request encoding, response parsing (blocking + streaming). Tests with wiremock + recorded fixture.
5. **openai_compat tool calling** — request encoding, response parsing (blocking + streaming). Tests.
6. **codex tool calling** — same pattern as openai_compat (likely shares encoder).
7. **gemini tool calling** — different schema (`functionDeclarations`, `functionCall`/`functionResponse`). Tests.
8. **copilot tool calling** — copy from openai_compat. Tests.
9. **claude HTTP image input** — `ChatContent::Image` encoding. Tests.
10. **openai_compat HTTP image input** — `ChatContent::Image` encoding (data URL form). Tests.
11. **gemini HTTP image input** — `inlineData` part. Tests + warn-on-URL behavior documented.
12. **codex / copilot HTTP image input** — same as openai_compat.
13. **Local provider adaptation** — `claude_local`, `codex_local`, `gemini_local` consume new `ChatMessage` shape via `text_content()`. Image blocks → existing CLI image flow if path-based. ToolResult → ignored with debug log. Tests confirm.
14. **CHANGELOG + README + migration guide + version bump 0.4.0** — final commit.

Total commits: ~14, similar shape to v0.3.1 (13 commits).

## Function/API signatures

```rust
// src/lib.rs

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum ChatContent {
    Text(String),
    Image { source: ImageSource, mime_type: String },
    ToolResult { tool_use_id: String, content: String, is_error: bool },
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum ImageSource {
    Base64(String),
    Url(String),
}

#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolChoice {
    Auto,
    Required,
    None,
    Specific(String),
}

#[derive(Clone)]
#[non_exhaustive]
pub struct ChatMessage {
    pub role: String,
    pub content: Vec<ChatContent>,
}

impl ChatMessage {
    pub fn user(text: impl Into<String>) -> Self { /* role: "user", content: vec![Text(...)] */ }
    pub fn assistant(text: impl Into<String>) -> Self;
    pub fn user_with_image(text: impl Into<String>, image: ChatContent) -> Self;
    pub fn user_multipart(blocks: Vec<ChatContent>) -> Self;
    pub fn tool_result(tool_use_id: impl Into<String>, content: impl Into<String>, is_error: bool) -> Self;
    pub fn text_content(&self) -> String;  // concatenates Text blocks; skips Image / ToolResult
}

impl From<&str> for ChatContent { /* Text variant */ }
impl From<String> for ChatContent { /* Text variant */ }

#[non_exhaustive]
pub enum LlmStreamEvent {
    Delta { text: String },
    End { total: LlmResponse },
    Error { message: String },
    ToolCall { call: ToolCall },
}

pub struct LlmRequestOptions {
    // ... existing v0.3.2 fields ...
    pub tools: Vec<ToolDef>,
    pub tool_choice: Option<ToolChoice>,
}

pub struct LlmResponse {
    // ... existing v0.3.x fields ...
    pub tool_calls: Vec<ToolCall>,
}
```

Trait `LlmProviderDyn` itself: NO method signature changes. The new options/response fields flow through existing methods.

## Constraints

- **`#[non_exhaustive]` is breaking but bounded**: applied to `ChatMessage`, `LlmStreamEvent`, `ChatContent`, `ImageSource`. After v0.4.0 lands, future enum-variant additions are non-breaking. This is the explicit one-time cost.
- **No tool execution loop**: `LlmProviderDyn` impls receive `ToolCall` and return them via `LlmResponse.tool_calls` / `LlmStreamEvent::ToolCall`. Caller decides what to do. Library never executes anything.
- **Tool encoding fidelity**: provider-specific schema is faithful. Fields mapped 1:1 where possible. Where schemas diverge significantly (e.g. OpenAI's separate `role: "tool"` message vs Anthropic's `tool_result` block inside user message), encoders translate transparently.
- **Image base64 responsibility**: caller encodes. Library transports. No file-path → base64 auto-encoding (would conflate with `image_paths` local-CLI flow).
- **`max_tokens` requirement intact**: claude still requires `max_tokens` at protocol level — `LlmRequestOptions.max_tokens` (added in v0.3.2) is the override, default is `MAX_TOKENS = 1024`.
- **Streaming `ToolCall` accumulation**: server-side complete block, then emit. Caller sees complete `ToolCall` with parsed arguments. No half-baked JSON.
- **`Default` impls preserved**: `LlmResponse::default()` works (or struct literal with `..Default::default()`); `LlmRequestOptions::default()` likewise. New fields default to empty Vec / None.
- **Provider feature degradation**: provider that doesn't support tools (dummy, _local) emits no `ToolCall` and silently ignores `options.tools`. NO error.
- **Test coverage**: each new feature gets at least one wiremock-backed integration test per provider that supports it. Dummy provider gets compile-only tests.
- **No new `LlmError` variants**: existing taxonomy (`Network`, `Auth`, `Parse`, etc.) covers all failure modes including malformed `tool_use` JSON (parse error → `LlmError::Parse`).

## Decisions

- **Adopted: `Vec<ChatContent>` for `ChatMessage.content`** — single shape for text-only and multipart. Simpler than `Either<String, Vec<...>>`.
- **Adopted: `#[non_exhaustive]` on enums + `ChatMessage` struct** — one-time breaking cost for permanent future-additivity.
- **Adopted: `From<&str>` / `From<String>` for `ChatContent`** — minimizes call-site verbosity. `vec![ChatContent::from("hi")]` works; constructor methods further reduce boilerplate.
- **Adopted: complete-block streaming for tool_use** — simpler caller code, one `ToolCall` event = one fully-formed call. Defer partial-JSON streaming to v0.5+ when caller demand surfaces.
- **Adopted: caller-side base64 encoding for images, with optional MIME helpers** — three call paths provided:
  1. Explicit: `ChatContent::Image { source: Base64(...), mime_type: "image/png" }` — always works.
  2. Path-based helper: `ChatContent::image_from_path(path)` — reads file, base64-encodes, infers MIME from extension. Returns `Result<ChatContent, std::io::Error>`.
  3. Magic-byte sniffing helper: `ChatContent::image_from_base64_auto(data)` — uses `infer` crate to detect MIME from the first bytes. Returns `Result<ChatContent, &'static str>` (error if format unrecognized).

  Adds dependency: `infer = "0.16"` (~3KB, MIT, no transitive bloat). Default path stays explicit; helpers are convenience.
- **Adopted: deferring thinking-block separation to v0.4.1+** — current `reasoning_effort` already triggers model behavior; UI distinction is the only missing piece, and downstream consumers haven't surfaced this need yet.
- **Rejected: parallel `ChatMessageRich` type alongside legacy `ChatMessage`** — doubled API surface, ugly long-term, perverse incentive to never migrate. Bite the breaking-change bullet once.
- **Rejected: `content: Either<String, Vec<ChatContent>>`** — type-level branching pollutes every provider impl. Worse ergonomics than constructor-based migration.
- **Rejected: tool execution loop in library** — out of scope per handoff doc. Library provides primitives; caller orchestrates.
- **Rejected: new `LlmError::Tool` variant** — exhaustive-match break for marginal benefit. Existing `Parse` covers malformed tool_use; existing `Other` covers unexpected provider errors.
- **Rejected: per-token streaming of `tool_use` JSON (`input_json_delta`)** — complex caller code (partial JSON parse not always valid mid-stream), deferred.
- **Rejected: auto-encoding `image_paths` as base64 for HTTP providers** — silently mixing local-CLI semantics into HTTP transport invites confusion. Caller explicitly chooses `Base64(...)` or `Url(...)` via `ChatContent::Image` for HTTP; `image_paths` stays `_local`-only.
- **Rejected: `ToolDef.input_schema: jsonschema::JSONSchema`** — pinning a JSON Schema crate version is a downstream burden. Verbatim `serde_json::Value` passthrough; caller validates.
- **Rejected: bumping to 1.0** — public API still maturing (tool calling is the largest single addition since extraction). Hold for one or two more releases.

## Resolved decisions (from user review 2026-04-26)

1. **Constructor names**: `ChatMessage::user()` / `assistant()` / `user_with_image()` / `tool_result()`. Rust convention (cf. `String::from`, `Vec::with_capacity`, `Command::new`).

2. **Migration**: hard cutover at v0.4.0. No v0.3.99 deprecation overlap. Migration guide in `docs/migration/v0.4.0.md` covers the 4 downstream projects.

3. **Copilot tool calling**: implement, mark **experimental** in module-level rustdoc. Production behavior with real Copilot endpoint not guaranteed; tests use mock.

4. **Image MIME**: 3 call paths.
   - Explicit `ChatContent::Image { mime_type, ... }` (always works, default).
   - `ChatContent::image_from_path(path)` (extension-based inference + IO).
   - `ChatContent::image_from_base64_auto(data)` (magic-byte sniffing via `infer` crate).
   Adds `infer = "0.16"` dep (~3KB, MIT).

5. **Local CLI `tool_result` blocks**: silent drop + `tracing::debug!` log. Caller using local-CLI is implicitly outside the API tool flow (CLI handles its own internal tools; user-defined tool definitions don't reach it).

6. **`LlmStreamEvent` ordering**: `Delta` for text in arrival order; `ToolCall` after each `tool_use` block completes. Multiple `ToolCall` events possible per stream.

Spec is ready for plan-stage breakdown.
