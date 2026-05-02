# Per-Call Request Timeout Implementation Plan (v0.3.1)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `LlmRequestOptions.timeout: Option<Duration>`. When set, all 5 HTTP providers and 3 local CLI providers honor it on `chat_with_options_dyn` / `chat_stream_with_options_dyn`. Default `None` preserves v0.3.0 hardcoded behavior. No breaking change.

**Spec:** `docs/specs/2026-04-25-request-timeout.md`

**Tech Stack:** Rust 1.85 (edition 2024). Existing tokio features (`rt`, `macros`, `io-util`, `process`) suffice; verify whether `time` is implicit. reqwest `RequestBuilder::timeout(d)` for HTTP; `tokio::time::timeout(d, future)` for local CLI. No new dependencies.

**Precondition:** Branch `feat/request-timeout-v0.3.1` is checked out from `main` (which has v0.3.0 + docs refresh commit `3ee4046`). Working tree is clean. Verify with `git status && git log --oneline -3`.

---

## Pre-Phase: Setup

### Task 0: Verify baseline + tokio feature audit

**Files:** `Cargo.toml` (possibly modify)

- [ ] **Step 0.1: Confirm clean working tree on feature branch**

```bash
cd /Users/kangnam/projects/llm-router
git branch --show-current  # → feat/request-timeout-v0.3.1
git status                  # → empty (only new spec/plan in docs/ which are git-untracked-but-fine)
```

- [ ] **Step 0.2: Audit tokio time feature**

```bash
grep -E '^tokio = ' Cargo.toml
```

If features don't include `"time"`, add it. Test by writing a temp `tokio::time::timeout(Duration::from_secs(1), async { 1 }).await` snippet — `cargo check` must pass.

Note: tokio's `rt` feature implies a runtime which transitively gates `time` in some configurations; always-on `tokio::time` is gated by feature `"time"`. Confirm explicitly.

- [ ] **Step 0.3: Run baseline test suite**

```bash
cargo test --lib 2>&1 | tail -5
```
Expected: `67 passed`. Record. We must end with `>= 67 + N new tests passed`.

- [ ] **Step 0.4: Commit any Cargo.toml feature change (if needed)**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: enable tokio time feature for request timeout"
```

Skip this commit if no feature change was needed.

---

## Phase 1: Public API surface

### Task 1: Add `timeout` field to `LlmRequestOptions`

**Files:** `src/lib.rs`

- [ ] **Step 1.1: Extend `default_options_are_empty` test**

In `src/lib.rs` `mod request_options_tests`, add to the existing test:

```rust
assert!(opts.timeout.is_none());
```

Run `cargo test request_options_tests::default_options_are_empty 2>&1 | tail -5`.
Expected: FAIL (field doesn't exist yet).

- [ ] **Step 1.2: Add the field**

Open `src/lib.rs`. Find `pub struct LlmRequestOptions { ... reasoning_effort: Option<String> }`. Add after `reasoning_effort`:

```rust
    /// Maximum total time for this call. None = use the provider's built-in
    /// default. On elapse:
    /// - Blocking calls return `LlmError::Network { msg: "timeout" }`.
    /// - Streams emit `LlmStreamEvent::Error { message: "timeout" }` and end.
    /// Local CLI providers (`claude_local`, `codex_local`, `gemini_local`)
    /// rely on `kill_on_drop(true)` to reap the subprocess on stream drop.
    /// Honored only on `chat_with_options_dyn` / `chat_stream_with_options_dyn`;
    /// the no-options paths (`chat_dyn` / `chat_stream_dyn`) keep the v0.3.0
    /// hardcoded behavior.
    pub timeout: Option<std::time::Duration>,
```

- [ ] **Step 1.3: Add a struct-update test**

In `mod request_options_tests`, add:

```rust
#[test]
fn options_timeout_can_be_set() {
    use std::time::Duration;
    let opts = LlmRequestOptions {
        timeout: Some(Duration::from_millis(500)),
        ..Default::default()
    };
    assert_eq!(opts.timeout, Some(Duration::from_millis(500)));
}
```

- [ ] **Step 1.4: Verify**

```bash
cargo test --lib request_options_tests 2>&1 | tail -10
cargo build --lib 2>&1 | tail -5
```
Expected: tests pass; build clean (existing struct literal sites unaffected because all use `..Default::default()`).

- [ ] **Step 1.5: Commit**

```bash
git add src/lib.rs
git commit -m "feat(options): add LlmRequestOptions.timeout field"
```

---

## Phase 2: HTTP providers

For each HTTP provider, override `chat_with_options_dyn` and `chat_stream_with_options_dyn` so they honor `options.timeout`. Use a per-request `RequestBuilder::timeout(d)` that overrides the constant baked into the `Client`. The pattern:

- Trait default forwards to `chat_dyn` / `chat_stream_dyn` (no-options) — that keeps the v0.3.0 default constants for callers who don't pass options.
- Override on the provider type calls a new private helper (e.g. `do_chat_with_timeout(...)`, `do_stream_with_timeout(...)`) that mirrors the existing helper but accepts a `request_timeout: Option<Duration>` and applies it to the `RequestBuilder`.

We do these provider-by-provider, smallest first.

### Task 2: `gemini` (HTTP, simplest)

**Files:** `src/gemini.rs`

- [ ] **Step 2.1: Add `do_chat_with_timeout` private helper**

Mirror the existing `do_chat` body. Change the request-builder line from:

```rust
let resp = req.send().await...
```

…to apply `request_timeout` only when `Some`:

```rust
let req = match request_timeout {
    Some(d) => req.timeout(d),
    None => req,
};
let resp = req.send().await...
```

(`reqwest::RequestBuilder::timeout(d)` exists since 0.11; we're on 0.12.)

- [ ] **Step 2.2: Add `do_stream_with_timeout` private helper**

Same pattern for the streaming path.

- [ ] **Step 2.3: Override `chat_with_options_dyn`**

```rust
fn chat_with_options_dyn<'a>(
    &'a self,
    system_prompt: &'a str,
    messages: &'a [ChatMessage],
    options: &'a crate::LlmRequestOptions,
    _result_json: &'a Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + 'a>> {
    let timeout = options.timeout;
    let system = system_prompt.to_string();
    let msgs = messages.to_vec();
    Box::pin(async move { self.do_chat_with_timeout(&system, &msgs, timeout).await })
}
```

- [ ] **Step 2.4: Override `chat_stream_with_options_dyn`** — analogous.

- [ ] **Step 2.5: Verify**

```bash
cargo build --lib 2>&1 | tail -5
cargo test --lib gemini 2>&1 | tail -10
cargo clippy --lib -- -D warnings 2>&1 | tail -10
```

- [ ] **Step 2.6: Commit**

```bash
git add src/gemini.rs
git commit -m "feat(gemini): honor LlmRequestOptions.timeout per-call"
```

### Task 3: `codex` (HTTP)

Same pattern as Task 2 but on `src/codex.rs`. No streaming path on this provider's blocking-only `do_chat` — apply timeout to blocking only and rely on the trait default for streaming if no streaming impl exists.

(If `chat_stream_dyn` is implemented, mirror the streaming override too.)

- [ ] **Step 3.1**: Add `do_chat_with_timeout`.
- [ ] **Step 3.2**: Override `chat_with_options_dyn`. Override `chat_stream_with_options_dyn` only if a streaming path exists.
- [ ] **Step 3.3**: Verify (`cargo test --lib codex 2>&1 | tail -10`, clippy clean).
- [ ] **Step 3.4**: Commit `feat(codex): honor LlmRequestOptions.timeout per-call`.

### Task 4: `copilot` (HTTP)

Same pattern on `src/copilot.rs`.

- [ ] **Step 4.1**: Add helpers as needed.
- [ ] **Step 4.2**: Override `_with_options_dyn` methods.
- [ ] **Step 4.3**: Verify.
- [ ] **Step 4.4**: Commit `feat(copilot): honor LlmRequestOptions.timeout per-call`.

### Task 5: `claude` (HTTP, has separate stream timeout)

Same pattern on `src/claude.rs`. This provider has split request (30s) / stream (120s) constants. Override behavior: when `options.timeout` is `Some(d)`, `d` replaces BOTH (single-knob mental model per spec § Decisions).

- [ ] **Step 5.1**: Helpers.
- [ ] **Step 5.2**: Overrides.
- [ ] **Step 5.3**: Verify.
- [ ] **Step 5.4**: Commit `feat(claude): honor LlmRequestOptions.timeout per-call`.

### Task 6: `openai_compat` (HTTP — primary user case)

Same pattern on `src/openai_compat.rs`. This is the user's reported pain point.

- [ ] **Step 6.1**: Add `do_chat_with_timeout` and a `do_stream_with_timeout` (taking `Option<Duration>`).
- [ ] **Step 6.2**: Override `chat_with_options_dyn`.
- [ ] **Step 6.3**: Override `chat_stream_with_options_dyn`. Note: streaming wraps `do_stream` in an `async_stream::stream!` that re-yields; ensure the override's stream is still `'a`-lifetime-correct (use the same `Arc<Self>` clone pattern as `chat_stream_dyn`).
- [ ] **Step 6.4**: Add wiremock-based test (TDD-friendly):

In `src/openai_compat.rs::tests` (create the module if absent):

```rust
#[tokio::test(flavor = "multi_thread")]
async fn timeout_option_aborts_slow_blocking_call() {
    use crate::{ChatMessage, LlmError, LlmProviderDyn, LlmRequestOptions};
    use serde_json::Value;
    use std::time::Duration;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(2))
                .set_body_json(serde_json::json!({"choices":[{"message":{"content":"late"}}]})),
        )
        .mount(&server)
        .await;

    let provider = super::make("k", "test-model", &server.uri()).expect("make");
    let messages = vec![ChatMessage { role: "user".into(), content: "x".into() }];
    let opts = LlmRequestOptions {
        timeout: Some(Duration::from_millis(500)),
        ..Default::default()
    };

    let started = std::time::Instant::now();
    let res = provider
        .chat_with_options_dyn("", &messages, &opts, &Value::Null)
        .await;
    let elapsed = started.elapsed();

    assert!(matches!(
        res,
        Err(LlmError::Network { ref provider, ref msg, .. })
            if provider == "openai_compat" && msg.contains("timeout")
    ), "expected Network/timeout error, got {res:?}");
    assert!(
        elapsed < Duration::from_millis(1500),
        "should have aborted before 1.5s, took {elapsed:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn timeout_option_aborts_slow_stream() {
    use crate::{ChatMessage, LlmProviderDyn, LlmRequestOptions, LlmStreamEvent};
    use futures::StreamExt;
    use serde_json::Value;
    use std::time::Duration;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
        .mount(&server)
        .await;

    let provider = super::make("k", "test-model", &server.uri()).expect("make");
    let messages = vec![ChatMessage { role: "user".into(), content: "x".into() }];
    let opts = LlmRequestOptions {
        timeout: Some(Duration::from_millis(500)),
        ..Default::default()
    };

    let mut stream = provider.chat_stream_with_options_dyn("", &messages, &opts, &Value::Null);
    let started = std::time::Instant::now();
    let mut got_error = false;
    while let Some(ev) = stream.next().await {
        if let LlmStreamEvent::Error { message } = ev {
            assert!(message.contains("timeout"), "error not timeout: {message}");
            got_error = true;
            break;
        }
    }
    assert!(got_error, "expected timeout Error event");
    assert!(started.elapsed() < Duration::from_millis(1500));
}
```

- [ ] **Step 6.5**: Verify both tests pass.
- [ ] **Step 6.6**: Commit `feat(openai_compat): honor LlmRequestOptions.timeout per-call`.

---

## Phase 3: Local CLI providers

For each local provider, override `chat_with_options_dyn` and `chat_stream_with_options_dyn` to wrap the inner work with `tokio::time::timeout`. On elapse:
- Blocking: `match tokio::time::timeout(d, inner_future).await { Ok(r) => r, Err(_) => Err(LlmError::Network { provider, msg: "timeout".into() }) }`
- Streaming: inside `async_stream::stream!`, use `tokio::select!` between the inner stream and `tokio::time::sleep(d)`. On sleep wins: `yield LlmStreamEvent::Error { message: "timeout".into() }`, then `return` (drops stream → `kill_on_drop` reaps child).

If `options.timeout` is `None`, don't wrap — preserve current behavior verbatim.

### Task 7: `claude_local`

**Files:** `src/claude_local.rs`, `tests/fixtures/fake_claude_slow.sh` (new)

- [ ] **Step 7.1: Add fixture**

Create `tests/fixtures/fake_claude_slow.sh`:
```sh
#!/bin/sh
sleep 5
echo '{"type":"assistant","message":{"content":[{"type":"text","text":"too late"}]}}'
echo '{"type":"result","subtype":"success","total_cost_usd":0,"usage":{"input_tokens":1,"output_tokens":1}}'
```
Then `chmod +x tests/fixtures/fake_claude_slow.sh`.

- [ ] **Step 7.2: Add timeout test (blocking + streaming)**

Use the same `CLAUDE_CLI_PATH` env-mutation pattern already used by `claude_local::tests`. Wrap with `tokio::sync::Mutex` lock if existing tests already do (mirror `CODEX_ENV_LOCK` from codex_local). Skip the test if no equivalent lock exists; add one. Test asserts:
- blocking: `LlmError::Network { msg: "timeout" }` within < 2s
- streaming: `LlmStreamEvent::Error { message: "timeout" }` first non-Delta event, total wall time < 2s

- [ ] **Step 7.3: Implement override on `chat_with_options_dyn`**

Wrap the existing inner-call with `tokio::time::timeout(d, ...)` only when `options.timeout.is_some()`.

- [ ] **Step 7.4: Implement override on `chat_stream_with_options_dyn`**

Inside the `async_stream::stream!`, use the `tokio::select!` pattern. On timeout branch, yield `Error { message: "timeout".into() }` and return. The dropped inner stream / spawned child task triggers `kill_on_drop`.

- [ ] **Step 7.5: Verify all `claude_local` tests pass + clippy clean**

- [ ] **Step 7.6: Commit `feat(claude_local): honor LlmRequestOptions.timeout per-call`.**

### Task 8: `codex_local`

**Files:** `src/codex_local.rs`, `tests/fixtures/fake_codex_slow.sh` (new)

- [ ] **Step 8.1: Add fixture** `fake_codex_slow.sh`:

```sh
#!/bin/sh
sleep 5
echo '{"type":"item.completed","item":{"item_type":"assistant_message","text":"too late"}}'
echo '{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1}}'
```
Then `chmod +x`.

- [ ] **Step 8.2: Add timeout test** — reuse `CODEX_ENV_LOCK` (already exists per v0.3.0). Mirror Task 7 assertions.

- [ ] **Step 8.3: Implement overrides** as in Task 7.

- [ ] **Step 8.4: Verify + commit `feat(codex_local): honor LlmRequestOptions.timeout per-call`.**

### Task 9: `gemini_local`

**Files:** `src/gemini_local.rs`, `tests/fixtures/fake_gemini_slow.sh` (new)

- [ ] **Step 9.1: Add fixture** `fake_gemini_slow.sh`:
```sh
#!/bin/sh
sleep 5
echo "too late"
```
Then `chmod +x`.

- [ ] **Step 9.2: Add timeout test** — add a `GEMINI_ENV_LOCK` modeled on `CODEX_ENV_LOCK` if absent.

- [ ] **Step 9.3: Implement overrides**.

- [ ] **Step 9.4: Verify + commit `feat(gemini_local): honor LlmRequestOptions.timeout per-call`.**

---

## Phase 4: Release prep

### Task 10: CHANGELOG + README + version bump

**Files:** `CHANGELOG.md`, `README.md`, `Cargo.toml`

- [ ] **Step 10.1: CHANGELOG entry**

Insert under `## [Unreleased]` (or above `## [0.3.0]`):

```
## [0.3.1] - 2026-04-25

### Added
- `LlmRequestOptions.timeout: Option<std::time::Duration>` — per-call total
  deadline. Honored on `chat_with_options_dyn` and
  `chat_stream_with_options_dyn`.
  - HTTP providers (claude, codex, copilot, gemini, openai_compat): applied
    via `RequestBuilder::timeout(d)`, overriding the per-provider
    hardcoded constants on a per-request basis.
  - Local CLI providers (claude_local, codex_local, gemini_local):
    inner future / stream wrapped in `tokio::time::timeout(d, ...)`. On
    elapse, the dropped stream triggers `kill_on_drop(true)` to reap the
    subprocess.
- Timeout fixtures: `tests/fixtures/fake_{claude,codex,gemini}_slow.sh`.
- Tests: wiremock-driven HTTP timeout assertions on `openai_compat`;
  fake-CLI timeout assertions on the three `_local` providers.

### Changed
- No-options paths (`chat_dyn`, `chat_stream_dyn`) unchanged.
- No new `LlmError` variant; timeout surfaces as
  `LlmError::Network { msg: "timeout" }` (blocking) /
  `LlmStreamEvent::Error { message: "timeout" }` (streaming) consistent
  with v0.3.0 behavior.

### Migration
- None. Adding a field with `Default = None` is non-breaking; existing
  `..Default::default()` constructions compile unchanged.
```

Also update the link footer:

```
[Unreleased]: https://github.com/kangnam7654/llm-router/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/kangnam7654/llm-router/compare/v0.3.0...v0.3.1
```

- [ ] **Step 10.2: README options matrix**

Find the "Per-Request Options" table and add a `timeout` row:

```
| `timeout`          | wraps reqwest    | wraps reqwest                        | wraps reqwest         |
```

(Or simpler: a one-line note above the matrix: "`timeout: Option<Duration>` is honored by all providers; HTTP via `RequestBuilder::timeout`, local CLI via `tokio::time::timeout`.")

Also add a brief snippet showing `timeout: Some(Duration::from_secs(180))` use.

- [ ] **Step 10.3: Cargo.toml version**

```toml
version = "0.3.1"
```

- [ ] **Step 10.4: Verify final**

```bash
cargo test --lib 2>&1 | tail -5
cargo clippy --all-targets -- -D warnings 2>&1 | tail -10
cargo doc --no-deps 2>&1 | grep -iE 'warn|error' | head -5
```
All clean. Test count must be `>= 67 + (HTTP timeout tests + 3 local timeout tests)`.

- [ ] **Step 10.5: Commit + tag**

```bash
git add CHANGELOG.md README.md Cargo.toml Cargo.lock
git commit -m "chore(release): bump to v0.3.1"
```

DO NOT tag yet. The tag/release is the controller's job (Task 11), not the implementer's.

---

## Phase 5: Final review + release

### Task 11: Final code review + tag + release (controller-driven)

- [ ] **Step 11.1**: Dispatch a final code-reviewer subagent to audit the whole branch diff (`git diff main...HEAD`) for:
  - All 8 providers honor timeout consistently.
  - Spec-compliance: `timeout: None` is byte-identical to v0.3.0 default behavior (no constants changed).
  - No `LlmError::Timeout` variant introduced.
  - kill_on_drop coverage on all 3 local providers.
  - clippy + rustdoc clean.

- [ ] **Step 11.2**: After reviewer approval, push branch + open PR (only on user request) OR fast-forward main + tag (only on user request):

```bash
# After user approval:
git checkout main
git merge --ff-only feat/request-timeout-v0.3.1
git tag -a v0.3.1 -m "v0.3.1: per-call request timeout"
# Push only on explicit user "push해" / "release해":
git push origin main v0.3.1
gh release create v0.3.1 --notes-from-tag
```

NEVER `git push` without explicit user request (CLAUDE.md NEVER #2).

---

## Quality gates

After every task:
1. `cargo build --lib` clean.
2. `cargo test --lib` — no regression vs baseline (67 + new tests).
3. `cargo clippy --all-targets -- -D warnings` — zero warnings.

After Phase 4:
4. `cargo doc --no-deps` — zero warnings.
5. Spec compliance review (subagent).
6. Code quality review (subagent).
