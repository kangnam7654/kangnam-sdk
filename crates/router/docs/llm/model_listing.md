# Model Listing Feature

## Purpose

Add ability to list available LLM models for each provider (Gemini HTTP API, Gemini CLI, Claude, Codex, Copilot).

## Completion Criteria

1. `ListModel` struct exported from crate root with `name`, `display_name`, `description`, `input_token_limit`, `output_token_limit` fields
2. `list_models(provider, api_key) -> Result<Vec<ListModel>, LlmError>` public function in `lib.rs`
3. `GeminiProvider::list_models(api_key)` — calls `GET https://generativelanguage.googleapis.com/v1beta/models?key={api_key}`
4. `GeminiLocalProvider::list_models()` — runs `gemini --list-models`, parses JSON output
5. `LlmProviderDyn::list_models_dyn` trait method with default `Err(Other { "not supported" })`
6. Tests: wiremock for gemini HTTP, unit test for gemini_local arg building
7. `registered_models()` returns all providers that support model listing

## File Changes

### `src/lib.rs`
- Add `ListModel` struct (pub) with serde Serialize+Deserialize
- Add `list_models(provider, api_key) -> Result<Vec<ListModel>, LlmError>` function
- Match on provider name, delegate to appropriate module
- Add `SUPPORTED_MODELS` constant listing providers that support listing

### `src/gemini.rs`
- Add `GeminiModel` response struct (internal) matching API response
- Add `pub async fn list_models(api_key: &str) -> Result<Vec<ListModel>, LlmError>`
- HTTP GET to `https://generativelanguage.googleapis.com/v1beta/models?key={api_key}`
- Parse `models[]` array, extract `name`, `display_name`, `description`, `input_token_limit`, `output_token_limit`

### `src/gemini_local.rs`
- Add `pub fn list_models(&self) -> Result<Vec<ListModel>, LlmError>`
- Spawn `gemini --list-models --output-format json`
- Parse JSON output (array of `{name, displayName, description, ...}`)
- Handle CLI not installed, auth not configured

### `src/claude.rs`, `src/codex.rs`, `src/copilot.rs`
- Each adds `pub async fn list_models(api_key: &str) -> Result<Vec<ListModel>, LlmError>`
- Claude: `GET https://api.anthropic.com/v1/models` (requires `x-api-key` + `anthropic-version`)
- Codex/Copilot: return `Err(Other { "model listing not yet supported for {provider}" })`
- Dummy: return empty vec

### `src/error.rs`
- No changes needed (reuse existing variants)

## Implementation Order

1. `ListModel` struct in `lib.rs` + export
2. `list_models` in `gemini.rs` (HTTP)
3. `list_models` in `gemini_local.rs` (CLI)
4. `list_models` in `claude.rs` (HTTP)
5. `list_models` stubs in `codex.rs`, `copilot.rs`, `dummy.rs`
6. `list_models` public function in `lib.rs`
7. Tests

## Function Signatures

```rust
// lib.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListModel {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub input_token_limit: Option<i32>,
    pub output_token_limit: Option<i32>,
}

pub fn list_models(provider: &str, api_key: &str) -> Result<Vec<ListModel>, LlmError>;
```

```rust
// gemini.rs
pub async fn list_models(api_key: &str) -> Result<Vec<ListModel>, LlmError>;

// gemini_local.rs
pub fn list_models(&self) -> Result<Vec<ListModel>, LlmError>;

// claude.rs
pub async fn list_models(api_key: &str) -> Result<Vec<ListModel>, LlmError>;
```

## Constraints

- Follow existing `reqwest` usage pattern (same Client builder, timeout, error handling)
- Reuse `LlmError` variants (Network, Upstream, Auth, Parse, Other)
- No new dependencies
- gemini_local `list_models` is sync (CLI), others are async (HTTP)
- Default to `generativelanguage.googleapis.com` base URL for Gemini

## Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Separate async fn per module vs trait method | Separate async fn per module | `list_models` doesn't need dynamic dispatch; simpler, no trait bloat |
| gemini_local sync vs async | Sync (spawn_blocking) | CLI is blocking I/O; callers can spawn if needed |
| Claude model listing | Implement (API exists) | Anthropic has `/v1/models` endpoint, well-documented |
| Codex/Copilot | Stub with unsupported error | No public model listing API documented |
