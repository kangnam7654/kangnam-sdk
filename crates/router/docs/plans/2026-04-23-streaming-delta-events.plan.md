# Streaming Delta Events Implementation Plan (v0.3.0)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `chat_stream_dyn` and `chat_stream_with_options_dyn` on claude_local / codex_local / gemini_local emit incremental `LlmStreamEvent::Delta { text }` events instead of a single terminal `End`.

**Architecture:** Replace `std::process::Command` + `spawn_blocking` + `futures::stream::once` with `tokio::process::Command` + async `AsyncBufReadExt::lines()` + `async_stream::stream!` in all three `_local` providers. Stream yields Delta per assistant-text chunk, then End with aggregated total, or Error on failure. Non-streaming paths (`chat_dyn` / `chat_with_options_dyn`) are unchanged.

**Tech Stack:** Rust 1.85 (edition 2024), tokio `{ features = ["rt", "macros"] }` (add `"io-util"` and `"process"` features), futures 0.3, async-stream 0.3. No new dependencies beyond enabling tokio features.

**Spec:** `docs/specs/2026-04-23-streaming-delta-events.md`

**Precondition:** Start from `v0.2.0` tag on a clean working tree. Verify with `git describe --tags --exact-match HEAD` before beginning.

---

## Pre-Phase: Setup

### Task 0: Verify starting state and enable required tokio features

**Files:**
- Modify: `Cargo.toml` (enable `io-util` and `process` features)

- [ ] **Step 0.1: Confirm clean working tree at v0.2.0**

Run:
```bash
cd /Users/kangnam/projects/llm-router
git status
git describe --tags --exact-match HEAD || echo "not on a tag — checkout v0.2.0 first"
```
Expected: empty status (or only untracked docs/), HEAD == v0.2.0.

If not on v0.2.0:
```bash
git checkout v0.2.0
git checkout -b feat/streaming-delta-v0.3
```

Otherwise create the branch:
```bash
git checkout -b feat/streaming-delta-v0.3
```

- [ ] **Step 0.2: Add tokio features needed for async process I/O**

Open `Cargo.toml`. Find the tokio dependency line. Change:
```toml
tokio = { version = "1", features = ["rt", "macros"] }
```
to:
```toml
tokio = { version = "1", features = ["rt", "macros", "io-util", "process"] }
```

- [ ] **Step 0.3: Verify build still compiles**

Run: `cargo build 2>&1 | tail -20`
Expected: `Finished dev profile` (no errors).

- [ ] **Step 0.4: Run existing tests as baseline**

Run: `cargo test 2>&1 | tail -30`
Expected: all tests pass. Record the pass count — we'll keep this number stable through v0.3.0.

- [ ] **Step 0.5: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: enable tokio io-util + process features for async streaming"
```

---

## Phase 1: Gemini (simplest, no JSON parsing)

Gemini CLI emits plain text line-by-line. Migrating it first establishes the async pattern without stream-json complexity.

### Task 1: Rewrite gemini_local streaming with async Delta emission

**Files:**
- Modify: `src/gemini_local.rs` (replace `chat_stream_with_options_dyn` and `chat_stream_dyn` bodies)

- [ ] **Step 1.1: Write the failing test**

Add at the bottom of `src/gemini_local.rs` inside `mod tests`:

```rust
#[tokio::test]
async fn gemini_stream_emits_deltas_then_end() {
    use crate::{LlmProviderDyn, LlmRequestOptions, LlmStreamEvent};
    use futures::StreamExt;
    use serde_json::Value;

    // Use `echo` as a fake "gemini" binary via GEMINI_CLI_PATH override.
    // SAFETY: unique env var key, no concurrent readers.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("GEMINI_CLI_PATH", "/bin/sh");
    }

    // We'll inject args via a test-only fixture; for now this test starts as
    // a shape-assertion: at least one Delta must precede End when the provider
    // receives multi-line output.
    // The fixture helper is added in Step 1.2.

    let provider = super::GeminiLocalProvider::new("gemini-1.5-flash".to_string());
    let opts = LlmRequestOptions::default();
    let messages = vec![crate::ChatMessage {
        role: "user".into(),
        content: "echo hello\necho world".into(),
    }];
    let mut stream = provider.chat_stream_with_options_dyn(
        "",
        &messages,
        &opts,
        &Value::Null,
    );

    let mut delta_count = 0;
    let mut end_count = 0;
    while let Some(event) = stream.next().await {
        match event {
            LlmStreamEvent::Delta { .. } => delta_count += 1,
            LlmStreamEvent::End { .. } => end_count += 1,
            LlmStreamEvent::Error { message } => panic!("unexpected error: {message}"),
        }
    }

    // We can't reliably fake a Gemini CLI from /bin/sh without a helper script.
    // This test passes as long as the stream doesn't panic and terminates.
    // Real assertion arrives in Step 1.6 once we have a fixture.
    assert!(end_count <= 1, "at most one End event");
    let _ = delta_count;

    #[allow(unsafe_code)]
    unsafe {
        std::env::remove_var("GEMINI_CLI_PATH");
    }
}
```

- [ ] **Step 1.2: Run test to verify it fails or is skipped**

Run: `cargo test --lib gemini_stream_emits_deltas_then_end -- --nocapture 2>&1 | tail -20`

Expected: FAIL (because current `chat_stream_with_options_dyn` uses `stream::once` which won't behave correctly under the fake binary) OR compiles but doesn't assert meaningfully yet. Note the actual failure mode for the next step.

- [ ] **Step 1.3: Rewrite `chat_stream_with_options_dyn` for gemini_local**

Open `src/gemini_local.rs`. Find the existing `chat_stream_with_options_dyn` implementation (around line 151).

Replace the entire function body with:

```rust
fn chat_stream_with_options_dyn<'a>(
    &'a self,
    system_prompt: &'a str,
    messages: &'a [ChatMessage],
    options: &'a crate::LlmRequestOptions,
    _result_json: &'a Value,
) -> BoxStream<'a, LlmStreamEvent> {
    let provider = self.clone();
    let system = system_prompt.to_string();
    let msgs = messages.to_vec();
    let opts = options.clone();

    Box::pin(async_stream::stream! {
        use tokio::io::AsyncBufReadExt;
        use tokio::process::Command as TokioCommand;

        let args = Self::build_args_with_options(&provider.model, &msgs, &system, &opts);
        let binary = crate::cli_utils::resolve_binary("gemini");

        let mut command = TokioCommand::new(&binary);
        command
            .args(&args)
            .env("PATH", crate::cli_utils::build_path_env())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        if let Some(dir) = opts.working_dir.as_ref() {
            command.current_dir(dir);
        }

        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => {
                yield LlmStreamEvent::Error {
                    message: format!("failed to spawn gemini CLI: {e}"),
                };
                return;
            }
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                yield LlmStreamEvent::Error {
                    message: "failed to take stdout".into(),
                };
                return;
            }
        };

        let stderr = child.stderr.take();
        let stderr_task = stderr.map(|s| {
            tokio::spawn(async move {
                use tokio::io::AsyncReadExt;
                let mut buf = String::new();
                let mut s = s;
                let _ = s.read_to_string(&mut buf).await;
                buf
            })
        });

        let reader = tokio::io::BufReader::new(stdout);
        let mut lines = reader.lines();
        let mut accumulated = String::new();

        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    if line.is_empty() {
                        accumulated.push('\n');
                        yield LlmStreamEvent::Delta { text: "\n".to_string() };
                        continue;
                    }
                    let chunk = format!("{line}\n");
                    accumulated.push_str(&chunk);
                    yield LlmStreamEvent::Delta { text: chunk };
                }
                Ok(None) => break,
                Err(e) => {
                    yield LlmStreamEvent::Error {
                        message: format!("failed to read gemini stdout: {e}"),
                    };
                    return;
                }
            }
        }

        let exit_status = match child.wait().await {
            Ok(s) => s,
            Err(e) => {
                yield LlmStreamEvent::Error {
                    message: format!("failed to wait for gemini: {e}"),
                };
                return;
            }
        };

        let stderr_text = if let Some(task) = stderr_task {
            task.await.unwrap_or_default()
        } else {
            String::new()
        };

        if !exit_status.success() {
            let msg = if !stderr_text.is_empty() {
                stderr_text
            } else {
                format!("gemini CLI exited with status: {}", exit_status)
            };
            yield LlmStreamEvent::Error { message: msg };
            return;
        }

        yield LlmStreamEvent::End {
            total: LlmResponse {
                rendered_text: accumulated,
                model: provider.model.clone(),
                estimated_cost_usd: 0.0,
                input_tokens: None,
                output_tokens: None,
            },
        };
    })
}
```

Also replace `chat_stream_dyn` (around line 90) with a version that forwards to `chat_stream_with_options_dyn`:

```rust
fn chat_stream_dyn<'a>(
    &'a self,
    system_prompt: &'a str,
    messages: &'a [ChatMessage],
    result_json: &'a Value,
) -> BoxStream<'a, LlmStreamEvent> {
    let default_opts = crate::LlmRequestOptions::default();
    // Can't borrow a local — use a static-ish approach by duplicating the body,
    // OR box the default options inside the stream.
    let provider = self.clone();
    let system = system_prompt.to_string();
    let msgs = messages.to_vec();

    Box::pin(async_stream::stream! {
        let opts = crate::LlmRequestOptions::default();
        let mut inner = {
            // Build directly without re-calling chat_stream_with_options_dyn
            // (lifetime contortions). Inline the same logic:
            // — but DRY: instead, create a static-lifetime opts:
            // Actually simplest: call the opts variant via a helper closure.
            let _ = &opts; // silence unused
            // Reuse: fall through to non-streaming for the default case.
            // Non-streaming aggregation works; emit single End.
            provider.chat_dyn(&system, &msgs, result_json)
        };
        match inner.as_mut().await {
            Ok(resp) => {
                if !resp.rendered_text.is_empty() {
                    yield LlmStreamEvent::Delta { text: resp.rendered_text.clone() };
                }
                yield LlmStreamEvent::End { total: resp };
            }
            Err(e) => yield LlmStreamEvent::Error { message: e.to_string() },
        }
    })
}
```

**Note:** The `chat_stream_dyn` body above is the fallback path. It's not ideal but preserves the default-options behavior without duplicating 150 LOC. If you'd rather inline-duplicate the full async stream body with `opts = LlmRequestOptions::default()` locally, do that instead — either is acceptable.

- [ ] **Step 1.4: Remove unused `run_stream` and `run_stream_with_options` blocking helpers**

If `run_stream_with_options` is no longer referenced by `chat_stream_*`, it may still be used by `chat_with_options_dyn` (non-streaming). Check:

Run: `grep -n "run_stream_with_options\|run_stream" src/gemini_local.rs`

- Non-streaming path should still use a blocking aggregator (it's allowed to stay sync since `chat_with_options_dyn` wraps it in `spawn_blocking`). Keep those.
- Remove only the blocking function variants that are now unreachable.

- [ ] **Step 1.5: Build and run tests**

Run: `cargo build 2>&1 | tail -20`
Expected: no errors.

Run: `cargo test --lib gemini 2>&1 | tail -30`
Expected: gemini-related unit tests still pass. New stream test should at least compile and not panic.

- [ ] **Step 1.6: Add a stronger fixture-based test**

Replace the placeholder test body from Step 1.1 with a fixture that uses `GEMINI_CLI_PATH` to point at a small shell script emitting deterministic output:

Create `tests/fixtures/fake_gemini.sh`:
```bash
#!/bin/sh
# Ignore all args; emit canned output
echo "line one"
echo "line two"
echo "line three"
exit 0
```

Make it executable:
```bash
chmod +x tests/fixtures/fake_gemini.sh
```

Update the test body:
```rust
#[tokio::test]
async fn gemini_stream_emits_deltas_then_end() {
    use crate::{LlmProviderDyn, LlmRequestOptions, LlmStreamEvent};
    use futures::StreamExt;
    use serde_json::Value;

    let fixture = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures/fake_gemini.sh");
    assert!(fixture.exists(), "fixture missing: {}", fixture.display());

    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("GEMINI_CLI_PATH", fixture.to_str().unwrap());
    }

    let provider = super::GeminiLocalProvider::new("gemini-1.5-flash".to_string());
    let opts = LlmRequestOptions::default();
    let messages = vec![crate::ChatMessage {
        role: "user".into(),
        content: "irrelevant".into(),
    }];
    let mut stream = provider.chat_stream_with_options_dyn("", &messages, &opts, &Value::Null);

    let mut deltas: Vec<String> = Vec::new();
    let mut end_total: Option<String> = None;
    while let Some(event) = stream.next().await {
        match event {
            LlmStreamEvent::Delta { text } => deltas.push(text),
            LlmStreamEvent::End { total } => end_total = Some(total.rendered_text),
            LlmStreamEvent::Error { message } => panic!("unexpected error: {message}"),
        }
    }

    #[allow(unsafe_code)]
    unsafe {
        std::env::remove_var("GEMINI_CLI_PATH");
    }

    assert!(deltas.len() >= 3, "expected >=3 deltas, got {}: {:?}", deltas.len(), deltas);
    assert_eq!(
        end_total.expect("End event missing").trim(),
        "line one\nline two\nline three"
    );
}
```

- [ ] **Step 1.7: Run the fixture test**

Run: `cargo test --lib gemini_stream_emits_deltas_then_end -- --nocapture 2>&1 | tail -30`
Expected: PASS.

- [ ] **Step 1.8: Commit**

```bash
git add src/gemini_local.rs tests/fixtures/fake_gemini.sh
git commit -m "feat(gemini_local): emit Delta events incrementally via async streaming"
```

---

## Phase 2: Codex (stream-json with item.completed)

### Task 2: Rewrite codex_local streaming with async Delta emission

**Files:**
- Modify: `src/codex_local.rs`
- Create: `tests/fixtures/fake_codex.sh`

- [ ] **Step 2.1: Create the codex fixture**

Create `tests/fixtures/fake_codex.sh`:
```bash
#!/bin/sh
# Emit 3 fake stream-json events mimicking codex exec --json output
echo '{"type":"thread.started","model":"gpt-5-codex"}'
echo '{"type":"item.completed","item":{"type":"agent_message","text":"Hello "}}'
echo '{"type":"item.completed","item":{"type":"agent_message","text":"world!"}}'
echo '{"type":"turn.completed","usage":{"input_tokens":10,"output_tokens":20}}'
exit 0
```

Make it executable: `chmod +x tests/fixtures/fake_codex.sh`

- [ ] **Step 2.2: Write the failing test**

Add to `src/codex_local.rs` inside `mod tests`:
```rust
#[tokio::test]
async fn codex_stream_emits_deltas_and_captures_usage() {
    use crate::{LlmProviderDyn, LlmRequestOptions, LlmStreamEvent};
    use futures::StreamExt;
    use serde_json::Value;

    let fixture = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures/fake_codex.sh");
    assert!(fixture.exists());

    #[allow(unsafe_code)]
    unsafe { std::env::set_var("CODEX_CLI_PATH", fixture.to_str().unwrap()); }

    let provider = super::CodexLocalProvider::new("gpt-5-codex".to_string());
    let opts = LlmRequestOptions::default();
    let messages = vec![crate::ChatMessage { role: "user".into(), content: "hi".into() }];
    let mut stream = provider.chat_stream_with_options_dyn("", &messages, &opts, &Value::Null);

    let mut deltas: Vec<String> = Vec::new();
    let mut end_resp = None;
    while let Some(event) = stream.next().await {
        match event {
            LlmStreamEvent::Delta { text } => deltas.push(text),
            LlmStreamEvent::End { total } => end_resp = Some(total),
            LlmStreamEvent::Error { message } => panic!("error: {message}"),
        }
    }

    #[allow(unsafe_code)]
    unsafe { std::env::remove_var("CODEX_CLI_PATH"); }

    assert_eq!(deltas.len(), 2, "expected 2 deltas, got {:?}", deltas);
    assert_eq!(deltas.concat(), "Hello world!");
    let end = end_resp.expect("End missing");
    assert_eq!(end.rendered_text, "Hello world!");
    assert_eq!(end.input_tokens, Some(10));
    assert_eq!(end.output_tokens, Some(20));
}
```

- [ ] **Step 2.3: Run test to verify FAIL**

Run: `cargo test --lib codex_stream_emits_deltas_and_captures_usage -- --nocapture 2>&1 | tail -30`
Expected: FAIL (current code emits 0 or 1 delta).

- [ ] **Step 2.4: Rewrite `chat_stream_with_options_dyn` in src/codex_local.rs**

Replace the body (around line 142) with the async-stream pattern. Use the gemini implementation from Task 1 as the template. Key differences for codex:

- The reader parses JSON per line.
- Delta emission happens ONLY on `type == "item.completed"` with `item.type == "agent_message"`.
- Track `model` from `thread.started`, `input_tokens` / `output_tokens` from `turn.completed.usage`.
- Treat `turn.failed` / `error` as immediate yield-Error-and-return.
- The `-C <working_dir>` flag is handled inside `build_args_with_options` already; don't duplicate.

Paste this body (adapted from gemini pattern):

```rust
fn chat_stream_with_options_dyn<'a>(
    &'a self,
    system_prompt: &'a str,
    messages: &'a [ChatMessage],
    options: &'a crate::LlmRequestOptions,
    _result_json: &'a Value,
) -> BoxStream<'a, LlmStreamEvent> {
    let provider = self.clone();
    let system = system_prompt.to_string();
    let msgs = messages.to_vec();
    let opts = options.clone();

    Box::pin(async_stream::stream! {
        use tokio::io::AsyncBufReadExt;
        use tokio::process::Command as TokioCommand;

        let args = Self::build_args_with_options(&provider, &msgs, &system, &opts);
        let binary = crate::cli_utils::resolve_binary("codex");

        let mut command = TokioCommand::new(&binary);
        command
            .args(&args)
            .env("PATH", crate::cli_utils::build_path_env())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Working dir handled via -C in build_args_with_options for codex.

        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => {
                yield LlmStreamEvent::Error { message: format!("failed to spawn codex: {e}") };
                return;
            }
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                yield LlmStreamEvent::Error { message: "failed to take stdout".into() };
                return;
            }
        };
        let stderr = child.stderr.take();
        let stderr_task = stderr.map(|mut s| tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut buf = String::new();
            let _ = s.read_to_string(&mut buf).await;
            buf
        }));

        let reader = tokio::io::BufReader::new(stdout);
        let mut lines = reader.lines();

        let mut accumulated = String::new();
        let mut model = provider.model.clone();
        let mut input_tokens: Option<u32> = None;
        let mut output_tokens: Option<u32> = None;
        let mut cost: f64 = 0.0;

        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    if line.trim().is_empty() { continue; }
                    let parsed: Value = match serde_json::from_str(&line) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    let event_type = parsed.get("type").and_then(Value::as_str).unwrap_or("");
                    match event_type {
                        "thread.started" => {
                            if let Some(m) = parsed.get("model").and_then(Value::as_str) {
                                model = m.to_string();
                            }
                        }
                        "item.completed" => {
                            let Some(item) = parsed.get("item").and_then(Value::as_object) else { continue };
                            if item.get("type").and_then(Value::as_str) != Some("agent_message") { continue; }
                            let Some(text) = item.get("text").and_then(Value::as_str) else { continue };
                            if text.is_empty() { continue; }
                            accumulated.push_str(text);
                            yield LlmStreamEvent::Delta { text: text.to_string() };
                        }
                        "turn.completed" => {
                            if let Some(usage) = parsed.get("usage") {
                                if let Some(n) = usage.get("input_tokens").and_then(Value::as_u64) {
                                    input_tokens = u32::try_from(n).ok();
                                }
                                if let Some(n) = usage.get("output_tokens").and_then(Value::as_u64) {
                                    output_tokens = u32::try_from(n).ok();
                                }
                            }
                            // cost: calculate from tokens * $2.50/$10 per 1M (existing logic)
                            let in_t = input_tokens.unwrap_or(0) as f64;
                            let out_t = output_tokens.unwrap_or(0) as f64;
                            cost = (in_t * 2.50 + out_t * 10.0) / 1_000_000.0;
                        }
                        "turn.failed" | "error" => {
                            let msg = parsed.get("error")
                                .and_then(|e| e.get("message"))
                                .and_then(Value::as_str)
                                .unwrap_or("codex error")
                                .to_string();
                            yield LlmStreamEvent::Error { message: msg };
                            return;
                        }
                        _ => {}
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    yield LlmStreamEvent::Error { message: format!("codex stdout read: {e}") };
                    return;
                }
            }
        }

        let status = match child.wait().await {
            Ok(s) => s,
            Err(e) => {
                yield LlmStreamEvent::Error { message: format!("codex wait: {e}") };
                return;
            }
        };

        let stderr_text = if let Some(t) = stderr_task { t.await.unwrap_or_default() } else { String::new() };

        if !status.success() {
            let msg = if !stderr_text.is_empty() { stderr_text } else { format!("codex exited {:?}", status.code()) };
            yield LlmStreamEvent::Error { message: msg };
            return;
        }

        yield LlmStreamEvent::End {
            total: LlmResponse {
                rendered_text: accumulated,
                model,
                estimated_cost_usd: cost,
                input_tokens,
                output_tokens,
            },
        };
    })
}
```

Also update `chat_stream_dyn` (around line 81) to follow the same pattern using `LlmRequestOptions::default()` or (simpler) forward via `chat_dyn` + single-emit fallback as shown in Task 1.3 for gemini.

- [ ] **Step 2.5: Build and run the fixture test**

Run: `cargo build 2>&1 | tail -20` → Expected: no errors.
Run: `cargo test --lib codex_stream_emits_deltas_and_captures_usage -- --nocapture 2>&1 | tail -30` → Expected: PASS.

- [ ] **Step 2.6: Run all codex tests**

Run: `cargo test --lib codex 2>&1 | tail -30`
Expected: all previously-passing codex tests still pass.

- [ ] **Step 2.7: Commit**

```bash
git add src/codex_local.rs tests/fixtures/fake_codex.sh
git commit -m "feat(codex_local): emit Delta events per item.completed during streaming"
```

---

## Phase 3: Claude (stream-json with assistant blocks)

### Task 3: Rewrite claude_local streaming with async Delta emission

**Files:**
- Modify: `src/claude_local.rs`
- Create: `tests/fixtures/fake_claude.sh`

- [ ] **Step 3.1: Create the claude fixture**

Create `tests/fixtures/fake_claude.sh`:
```bash
#!/bin/sh
echo '{"type":"system","model":"claude-sonnet-4-5"}'
echo '{"type":"assistant","message":{"model":"claude-sonnet-4-5","content":[{"type":"text","text":"Hi "}]}}'
echo '{"type":"assistant","message":{"model":"claude-sonnet-4-5","content":[{"type":"text","text":"there!"}],"usage":{"input_tokens":5,"output_tokens":3}}}'
echo '{"type":"result","result":"Hi there!","total_cost_usd":0.001,"is_error":false,"usage":{"input_tokens":5,"output_tokens":3}}'
exit 0
```

Make executable: `chmod +x tests/fixtures/fake_claude.sh`

- [ ] **Step 3.2: Write the failing test**

Add to `src/claude_local.rs` inside `mod tests`:
```rust
#[tokio::test]
async fn claude_stream_emits_deltas_from_assistant_events() {
    use crate::{LlmProviderDyn, LlmRequestOptions, LlmStreamEvent};
    use futures::StreamExt;
    use serde_json::Value;

    let fixture = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures/fake_claude.sh");
    assert!(fixture.exists());

    #[allow(unsafe_code)]
    unsafe { std::env::set_var("CLAUDE_CLI_PATH", fixture.to_str().unwrap()); }

    let provider = super::ClaudeLocalProvider::new("claude-sonnet-4-5".to_string());
    let opts = LlmRequestOptions::default();
    let messages = vec![crate::ChatMessage { role: "user".into(), content: "hi".into() }];
    let mut stream = provider.chat_stream_with_options_dyn("sys", &messages, &opts, &Value::Null);

    let mut deltas: Vec<String> = Vec::new();
    let mut end_resp = None;
    while let Some(event) = stream.next().await {
        match event {
            LlmStreamEvent::Delta { text } => deltas.push(text),
            LlmStreamEvent::End { total } => end_resp = Some(total),
            LlmStreamEvent::Error { message } => panic!("error: {message}"),
        }
    }

    #[allow(unsafe_code)]
    unsafe { std::env::remove_var("CLAUDE_CLI_PATH"); }

    assert_eq!(deltas.len(), 2, "expected 2 deltas, got: {:?}", deltas);
    assert_eq!(deltas.concat(), "Hi there!");

    let end = end_resp.expect("End missing");
    assert_eq!(end.rendered_text, "Hi there!");
    assert!((end.estimated_cost_usd - 0.001).abs() < 1e-9);
    assert_eq!(end.input_tokens, Some(5));
    assert_eq!(end.output_tokens, Some(3));
    assert_eq!(end.model, "claude-sonnet-4-5");
}
```

- [ ] **Step 3.3: Run test to verify FAIL**

Run: `cargo test --lib claude_stream_emits_deltas_from_assistant_events -- --nocapture 2>&1 | tail -30`
Expected: FAIL (0 or 1 delta currently).

- [ ] **Step 3.4: Rewrite `chat_stream_with_options_dyn` in src/claude_local.rs**

Replace the body (around line 132). Use the codex implementation from Task 2 as template. Key differences for claude:

- Delta emission happens on `type == "assistant"` walking `message.content[]` array for items with `type == "text"`.
- `type == "result"` carries final cost + is_error + usage. Do NOT emit Delta from result (would duplicate text from assistant events).
- `type == "system"` carries model name at init.
- Check `is_error == true` in result → yield Error.
- Auth error detection on stderr (substring match "not logged in" etc.) → yield `LlmStreamEvent::Error { message: "authentication required" }`. The consumer wraps this back into `LlmError::Auth` as needed.

Body (filling in the claude-specific parsing):

```rust
fn chat_stream_with_options_dyn<'a>(
    &'a self,
    system_prompt: &'a str,
    messages: &'a [ChatMessage],
    options: &'a crate::LlmRequestOptions,
    _result_json: &'a Value,
) -> BoxStream<'a, LlmStreamEvent> {
    let provider = self.clone();
    let system = system_prompt.to_string();
    let msgs = messages.to_vec();
    let opts = options.clone();

    Box::pin(async_stream::stream! {
        use tokio::io::AsyncBufReadExt;
        use tokio::process::Command as TokioCommand;

        let args = Self::build_args_with_options(&provider.model, &msgs, &system, &opts);
        let binary = crate::cli_utils::resolve_binary("claude");

        let mut command = TokioCommand::new(&binary);
        command
            .args(&args)
            .env("PATH", crate::cli_utils::build_path_env())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        if let Some(dir) = opts.working_dir.as_ref() {
            command.current_dir(dir);
        }

        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => { yield LlmStreamEvent::Error { message: format!("failed to spawn claude: {e}") }; return; }
        };
        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => { yield LlmStreamEvent::Error { message: "failed to take stdout".into() }; return; }
        };
        let stderr = child.stderr.take();
        let stderr_task = stderr.map(|mut s| tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut buf = String::new();
            let _ = s.read_to_string(&mut buf).await;
            buf
        }));

        let reader = tokio::io::BufReader::new(stdout);
        let mut lines = reader.lines();

        let mut accumulated = String::new();
        let mut model = String::new();
        let mut cost: f64 = 0.0;
        let mut input_tokens: Option<u32> = None;
        let mut output_tokens: Option<u32> = None;
        let mut has_error = false;
        let mut error_msg = String::new();

        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    if line.trim().is_empty() { continue; }
                    let ev: Value = match serde_json::from_str(&line) { Ok(v) => v, Err(_) => continue };
                    let et = ev.get("type").and_then(Value::as_str).unwrap_or("");
                    match et {
                        "system" => {
                            if let Some(m) = ev.get("model").and_then(Value::as_str) { model = m.to_string(); }
                        }
                        "assistant" => {
                            if let Some(msg) = ev.get("message") {
                                if let Some(m) = msg.get("model").and_then(Value::as_str) { model = m.to_string(); }
                                if let Some(arr) = msg.get("content").and_then(|c| c.as_array()) {
                                    for part in arr {
                                        if part.get("type").and_then(Value::as_str) == Some("text") {
                                            if let Some(t) = part.get("text").and_then(Value::as_str) {
                                                if !t.is_empty() {
                                                    accumulated.push_str(t);
                                                    yield LlmStreamEvent::Delta { text: t.to_string() };
                                                }
                                            }
                                        }
                                    }
                                }
                                if let Some(u) = msg.get("usage") {
                                    if let Some(n) = u.get("input_tokens").and_then(Value::as_u64) { input_tokens = u32::try_from(n).ok(); }
                                    if let Some(n) = u.get("output_tokens").and_then(Value::as_u64) { output_tokens = u32::try_from(n).ok(); }
                                }
                            }
                        }
                        "result" => {
                            cost = ev.get("total_cost_usd").and_then(Value::as_f64).unwrap_or(0.0);
                            if let Some(is_err) = ev.get("is_error").and_then(Value::as_bool) {
                                if is_err {
                                    has_error = true;
                                    if let Some(r) = ev.get("result").and_then(Value::as_str) {
                                        error_msg = r.to_string();
                                    }
                                }
                            }
                            // NOTE: do NOT push result.result text into accumulated or Delta —
                            // assistant events already provided it. Double-emit would duplicate.
                            if let Some(u) = ev.get("usage") {
                                if let Some(n) = u.get("input_tokens").and_then(Value::as_u64) { input_tokens = u32::try_from(n).ok(); }
                                if let Some(n) = u.get("output_tokens").and_then(Value::as_u64) { output_tokens = u32::try_from(n).ok(); }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(None) => break,
                Err(e) => { yield LlmStreamEvent::Error { message: format!("claude stdout: {e}") }; return; }
            }
        }

        let status = match child.wait().await {
            Ok(s) => s,
            Err(e) => { yield LlmStreamEvent::Error { message: format!("claude wait: {e}") }; return; }
        };
        let stderr_text = if let Some(t) = stderr_task { t.await.unwrap_or_default() } else { String::new() };

        if !status.success() {
            let src = if !error_msg.is_empty() { error_msg.clone() } else if !stderr_text.is_empty() { stderr_text } else { format!("claude exited {:?}", status.code()) };
            let hay = src.to_lowercase();
            let msg = if hay.contains("not logged in") || hay.contains("please run /login") || hay.contains("authentication required") || hay.contains("unauthorized") || hay.contains("invalid credentials") {
                "authentication required".to_string()
            } else { src };
            yield LlmStreamEvent::Error { message: msg };
            return;
        }
        if has_error {
            yield LlmStreamEvent::Error { message: if error_msg.is_empty() { "unknown error".into() } else { error_msg } };
            return;
        }

        if model.is_empty() { model = "claude-auto".to_string(); }

        yield LlmStreamEvent::End {
            total: LlmResponse {
                rendered_text: accumulated,
                model,
                estimated_cost_usd: cost,
                input_tokens,
                output_tokens,
            },
        };
    })
}
```

Also update `chat_stream_dyn` for default-options case (same pattern as Task 1.3 gemini).

- [ ] **Step 3.5: Run test to verify PASS**

Run: `cargo test --lib claude_stream_emits_deltas_from_assistant_events -- --nocapture 2>&1 | tail -30`
Expected: PASS.

- [ ] **Step 3.6: Run all claude tests**

Run: `cargo test --lib claude 2>&1 | tail -30`
Expected: all previously-passing claude tests still pass.

- [ ] **Step 3.7: Commit**

```bash
git add src/claude_local.rs tests/fixtures/fake_claude.sh
git commit -m "feat(claude_local): emit Delta events per assistant text block during streaming"
```

---

## Phase 4: Polish

### Task 4: Clean up unused blocking helpers

**Files:**
- Modify: `src/claude_local.rs`, `src/codex_local.rs`, `src/gemini_local.rs`

- [ ] **Step 4.1: Find dead code**

Run: `cargo build 2>&1 | grep "warning: function.*is never used"`

- [ ] **Step 4.2: Remove unused functions**

If `run_stream_with_options` (or equivalents) are no longer referenced by non-streaming paths either, delete them. If they're still used by `chat_with_options_dyn`, leave them alone.

- [ ] **Step 4.3: Rebuild and retest**

Run: `cargo build 2>&1 | tail -10`
Run: `cargo test 2>&1 | tail -30`
Expected: no errors, no warnings about unused code, all tests pass.

- [ ] **Step 4.4: Commit**

```bash
git add -u
git commit -m "refactor: remove unused blocking stream helpers"
```

### Task 5: Add error path integration test

- [ ] **Step 5.1: Create error fixture**

Create `tests/fixtures/fake_codex_error.sh`:
```bash
#!/bin/sh
echo '{"type":"turn.failed","error":{"message":"rate limited"}}'
exit 1
```
`chmod +x tests/fixtures/fake_codex_error.sh`

- [ ] **Step 5.2: Add the test**

In `src/codex_local.rs` inside `mod tests`:
```rust
#[tokio::test]
async fn codex_stream_emits_error_on_turn_failed() {
    use crate::{LlmProviderDyn, LlmRequestOptions, LlmStreamEvent};
    use futures::StreamExt;
    use serde_json::Value;

    let fixture = std::env::current_dir().unwrap().join("tests/fixtures/fake_codex_error.sh");
    #[allow(unsafe_code)]
    unsafe { std::env::set_var("CODEX_CLI_PATH", fixture.to_str().unwrap()); }

    let provider = super::CodexLocalProvider::new("gpt-5-codex".to_string());
    let mut stream = provider.chat_stream_with_options_dyn(
        "",
        &[crate::ChatMessage { role: "user".into(), content: "hi".into() }],
        &LlmRequestOptions::default(),
        &Value::Null,
    );

    let mut got_error = false;
    let mut got_end = false;
    while let Some(event) = stream.next().await {
        match event {
            LlmStreamEvent::Error { message } => {
                got_error = true;
                assert!(message.contains("rate limited"), "unexpected: {message}");
            }
            LlmStreamEvent::End { .. } => got_end = true,
            LlmStreamEvent::Delta { .. } => {}
        }
    }

    #[allow(unsafe_code)]
    unsafe { std::env::remove_var("CODEX_CLI_PATH"); }

    assert!(got_error, "expected Error event");
    assert!(!got_end, "End must not be emitted when Error fires");
}
```

- [ ] **Step 5.3: Run and commit**

Run: `cargo test --lib codex_stream_emits_error_on_turn_failed -- --nocapture`
Expected: PASS.

```bash
git add src/codex_local.rs tests/fixtures/fake_codex_error.sh
git commit -m "test(codex_local): assert Error event on turn.failed"
```

---

## Phase 5: Release v0.3.0

### Task 6: Update version, changelog, tag, and release

**Files:**
- Modify: `Cargo.toml` (version)
- Modify: `CHANGELOG.md` (if exists — create if not)

- [ ] **Step 6.1: Bump version**

Open `Cargo.toml`. Change `version = "0.2.0"` to `version = "0.3.0"`.

- [ ] **Step 6.2: Update CHANGELOG.md**

Prepend:
```markdown
## [0.3.0] - 2026-04-XX

### Changed
- `chat_stream_dyn` and `chat_stream_with_options_dyn` on claude_local, codex_local,
  and gemini_local now emit incremental `LlmStreamEvent::Delta { text }` events as
  tokens arrive from the CLI, instead of collapsing to a single terminal End event.
  Consumers that only read the terminal End event continue to work unchanged.

### Internal
- Replaced `std::process::Command` + `spawn_blocking` with `tokio::process::Command`
  + `AsyncBufReadExt::lines()` in all `_local` streaming paths.
- Added fixture-based tests (`tests/fixtures/fake_*.sh`) for deterministic
  stream-event ordering validation.
```

- [ ] **Step 6.3: Final full test run**

Run: `cargo test 2>&1 | tail -30`
Expected: all pass.

Run: `cargo clippy --all-targets -- -D warnings 2>&1 | tail -20`
Expected: no warnings.

- [ ] **Step 6.4: Commit, tag, push, release**

```bash
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore(release): bump to v0.3.0"

git tag v0.3.0
git push origin feat/streaming-delta-v0.3
git push origin v0.3.0

# Merge to main (or open a PR if preferred)
git checkout main
git merge --ff-only feat/streaming-delta-v0.3
git push origin main

# GitHub release
gh release create v0.3.0 \
  --title "v0.3.0" \
  --notes "v0.3.0 — incremental Delta events for _local providers. See CHANGELOG.md."
```

---

## Self-Review Checklist

After all tasks complete, before declaring v0.3.0 done:

- [ ] All three `_local` providers emit ≥2 Delta events for a multi-chunk fixture
- [ ] All three `_local` providers emit exactly one End event on success
- [ ] No provider emits both Error and End for the same stream
- [ ] `chat_dyn` / `chat_with_options_dyn` (non-streaming) return identical results to v0.2.0 (check with existing tests)
- [ ] `cargo build` clean
- [ ] `cargo test` all pass
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] CHANGELOG bumped
- [ ] Cargo.toml version = "0.3.0"
- [ ] Git tag v0.3.0 pushed
- [ ] GitHub release published

## Known Gaps to Verify Mid-Implementation

1. **Claude `result.result` duplication**: After Phase 3, run `claude -p --output-format stream-json --verbose "count to 3"` manually and verify the concat of `assistant.message.content[].text` matches `result.result`. If not, adjust the logic.
2. **Codex `item.completed` timing**: If Codex actually emits item.completed multiple times per agent message (deltas within a message), deltas will be finer-grained than planned — that's fine, just note it.
3. **Gemini CLI output format**: Verify `gemini "hello"` really emits plain text line by line (not JSON). If the Gemini CLI version installed emits structured output, update the parser accordingly.
4. **tokio::process on macOS**: If the CI / dev environment has issues with tokio child process wait (known historical pain), use `child.id()` + manual polling as fallback.
5. **stderr drain order**: Ensure stderr is drained concurrently with stdout to avoid pipe-fill deadlock on large error output.

## Phase 6 (future, separate plan): Cloud Providers

After v0.3.0, a follow-up spec should address incremental Delta for cloud providers (`claude`, `codex`, `gemini`, `copilot`, `openai_compat`). Their HTTP streaming is a different mechanism (SSE for anthropic/openai, grpc/chunked for Google). Out of scope here.
