# Per-Call Request Timeout (v0.3.1)

> **Status: draft**

## Purpose

Let callers override the hardcoded per-provider timeouts on a single LLM call. **Done when:** `LlmRequestOptions { timeout: Some(Duration::from_secs(180)), ..default() }` makes every provider honor 180s instead of its built-in default, and a fixture-driven test for `openai_compat` proves the elapsed deadline produces an `Error { message: "timeout" }` stream event without leaking the underlying request/process.

## Motivation

Today every HTTP provider's timeout is a `const` baked into the source file (claude 30s/120s, codex 60s, copilot 60s, gemini 60s, openai_compat 60s/300s). Local CLI providers have **no** timeout at all. A consumer hitting a slow `openai_compat` endpoint (self-hosted vLLM, on-prem proxy) cannot extend the 60s ceiling without forking the crate. Per-call configuration is the smallest change that unblocks them.

## Non-Goals

- Per-provider default override (e.g. `create_provider(..., default_timeout: ...)`). Not requested. Per-call control is sufficient.
- Splitting `request_timeout` vs `stream_timeout`. The internal split (claude 30/120, openai_compat 60/300) is a library implementation detail; the user model is "give me up to N seconds for this call."
- Inactivity-based timeouts (token-stalled-for-N-seconds). Total deadline only.
- Retry-on-timeout policy. Caller's responsibility.
- Honoring `timeout` in `chat_dyn` / `chat_stream_dyn` (the no-options paths). Use `chat_with_options_dyn` / `chat_stream_with_options_dyn` to exercise per-call options.

## Architecture

One additive field on `LlmRequestOptions`. Each provider's `chat_with_options_dyn` and `chat_stream_with_options_dyn` either passes the duration into reqwest's per-request `RequestBuilder::timeout(d)` (HTTP) or wraps the future in `tokio::time::timeout(d, ...)` (local CLI). On elapse:

- **HTTP providers**: reqwest already maps the elapsed timeout to an error caught by the existing `e.is_timeout()` branch in each provider — surfaces as `LlmError::Network { provider, msg: "timeout" }` (blocking) / `LlmStreamEvent::Error { message: "timeout" }` (streaming). No new error variant.
- **Local CLI providers**: `tokio::time::timeout` returns `Err(Elapsed)`. We map that to `LlmError::Network { provider, msg: "timeout" }` (blocking) / emit `LlmStreamEvent::Error { message: "timeout" }` (streaming) and drop the stream so `kill_on_drop(true)` reaps the child.

No breaking change. Existing constructors using `..Default::default()` continue to compile (new field defaults to `None`). Default behavior with `timeout: None` is byte-identical to v0.3.0.

## File changes

- `Cargo.toml` — bump `version` `0.3.0` → `0.3.1`. No new deps. Tokio already has `time` available via the default feature flag set; verify and add `"time"` if missing.
- `src/lib.rs` — add `pub timeout: Option<std::time::Duration>` to `LlmRequestOptions` (rustdoc), extend `request_options_tests::default_options_are_empty` to cover `assert!(opts.timeout.is_none())`.
- `src/claude.rs` — override `chat_with_options_dyn` / `chat_stream_with_options_dyn` (currently default-implemented at trait level). When `options.timeout` is `Some(d)`, wrap the inner reqwest call so `RequestBuilder::timeout(d)` replaces the constant. Reuses existing `is_timeout()` branches.
- `src/codex.rs` — same.
- `src/copilot.rs` — same.
- `src/gemini.rs` — same.
- `src/openai_compat.rs` — same. Both blocking and streaming paths.
- `src/claude_local.rs` — override `chat_with_options_dyn` and `chat_stream_with_options_dyn` (existing) to wrap the awaited future / stream in `tokio::time::timeout(d, ...)`. On elapse, emit timeout error per surface and drop the stream/child.
- `src/codex_local.rs` — same.
- `src/gemini_local.rs` — same.
- `tests/fixtures/fake_codex_slow.sh` — new fixture: `sleep 5; echo '{"type":"item.completed","item":{"text":"too late"}}'`. Used to assert local-CLI timeout fires before output arrives.
- `CHANGELOG.md` — new `[0.3.1] - 2026-04-25` entry.

## Implementation order

1. **`LlmRequestOptions.timeout` field + default test** — `src/lib.rs`. Field + rustdoc + extend default test. `cargo build` clean.
2. **HTTP providers (5 files)** — implement `chat_with_options_dyn` and `chat_stream_with_options_dyn` overrides on `claude`, `codex`, `copilot`, `gemini`, `openai_compat`. Pattern: copy existing blocking/streaming bodies, swap the constant `Duration::from_secs(REQUEST_TIMEOUT_SECS)` baked into the `Client` for a per-request `RequestBuilder::timeout(options.timeout.unwrap_or(<existing const>))`. Keep `Client` builder timeout as outer ceiling; `RequestBuilder::timeout` is allowed to be larger or smaller. Validate per file with `cargo test --lib <module>`.
3. **Local CLI providers (3 files)** — wrap entry points with `tokio::time::timeout`. Pattern: in `chat_with_options_dyn` (blocking), `match tokio::time::timeout(d, run_blocking_async(...)).await`; on `Err(_)` return `LlmError::Network { provider: "claude_local", msg: "timeout".into() }`. In `chat_stream_with_options_dyn`, push the timeout into the `async_stream::stream!` body via `tokio::select!` between `stream.next()` and `tokio::time::sleep(d)`. On sleep wins: yield `LlmStreamEvent::Error { message: "timeout".into() }`, then break (dropping the inner stream → `kill_on_drop` reaps child).
4. **Test: HTTP timeout via wiremock** — already a dev-dep. New test in `openai_compat.rs::tests` uses `wiremock` to delay response by 2s; calls with `timeout: Some(500ms)`; asserts blocking path returns `LlmError::Network { msg: "timeout" }` and streaming path yields exactly one `Error` event.
5. **Test: local CLI timeout via fake script** — `fake_codex_slow.sh` sleeps 5s. Test sets `CODEX_CLI_PATH` (under existing `CODEX_ENV_LOCK`), calls `chat_stream_with_options_dyn` with `timeout: Some(500ms)`, asserts first event is `Error { "timeout" }` within < 2s wallclock. Also covers blocking path.
6. **CHANGELOG `[0.3.1]` entry** — Added section with `LlmRequestOptions.timeout`. Changed section noting per-provider override behavior. Migration: none.
7. **Version bump** — `Cargo.toml` `0.3.0` → `0.3.1`. Update `[Unreleased]` link footer + add `[0.3.1]` link.
8. **Validation** — `cargo test --lib` (all 67 + new tests pass), `cargo clippy --all-targets -- -D warnings`, `cargo doc --no-deps`. No new warnings.

## Function/API signatures

```rust
// src/lib.rs — additive field on existing struct
#[derive(Debug, Clone, Default)]
pub struct LlmRequestOptions {
    pub image_paths: Vec<PathBuf>,
    pub working_dir: Option<PathBuf>,
    pub allow_web_search: bool,
    pub allow_local_read: bool,
    pub max_turns: Option<u32>,
    pub reasoning_effort: Option<String>,
    /// Maximum total time for this call. None = use the provider's built-in
    /// default. On elapse, blocking calls return
    /// `LlmError::Network { msg: "timeout" }`; streams emit
    /// `LlmStreamEvent::Error { message: "timeout" }` and end.
    /// Local CLI providers (`claude_local`, `codex_local`, `gemini_local`)
    /// kill the underlying subprocess on elapse via `kill_on_drop`.
    pub timeout: Option<std::time::Duration>,
}
```

No new public functions. No trait method signature changes — `chat_with_options_dyn` / `chat_stream_with_options_dyn` are already on `LlmProviderDyn` (trait default forwards to non-options path; per-provider overrides honor the new field).

## Constraints

- **Naming**: field is `timeout`, not `deadline` or `max_duration`. Matches reqwest / tokio conventions.
- **Error mapping**: only `LlmError::Network { msg: "timeout" }`. No new `LlmError::Timeout` variant — would be a breaking change to consumers who exhaustively match. The existing tests in `error.rs` already exercise the `msg: "timeout"` shape.
- **Streaming termination**: when timeout fires mid-stream, emit exactly one `LlmStreamEvent::Error` and end. No `End` event. Partial Deltas already delivered to consumer remain valid; aggregation is consumer's responsibility.
- **`kill_on_drop` reliance**: local CLI timeout MUST drop the stream so the child is reaped. No explicit `child.kill()` call — that's the existing v0.3.0 contract (commit `b5ec2a9`).
- **Floor**: `timeout: Some(Duration::ZERO)` is honored as "elapse immediately." Library does not impose a minimum. Caller's choice.
- **No-options paths unaffected**: `chat_dyn` / `chat_stream_dyn` keep the v0.3.0 hardcoded constants. Documented in rustdoc.
- **Test isolation**: local CLI tests reuse `CODEX_ENV_LOCK` (and equivalent for claude_local / gemini_local — add similar `tokio::sync::Mutex` if missing) to serialize env-var mutation.
- **Clippy**: no `await_holding_lock`. All locks across `.await` use `tokio::sync::Mutex`.

## Decisions

- **Adopted**: single `timeout: Option<Duration>` field on `LlmRequestOptions`, applied to total call elapse.
- **Rejected**: split `request_timeout` / `stream_timeout` — internal implementation detail; user mental model is "this call, N seconds." 1 field beats 2.
- **Rejected**: `LlmError::Timeout` new variant — exhaustive-match break. Reuse `Network { msg: "timeout" }` already established by HTTP `is_timeout()` branches.
- **Rejected**: per-provider default override at construction time — not the reported pain point; can be added later if needed without breaking this API.
- **Rejected**: inactivity-based stall detection — adds state machine + heartbeat config; out of scope. Total deadline is sufficient for the openai_compat slow-server case.
- **Rejected**: honoring `timeout` in `chat_dyn` / `chat_stream_dyn` (no-options paths) — would silently change v0.3.0 default behavior. Options paths are the explicit opt-in.
