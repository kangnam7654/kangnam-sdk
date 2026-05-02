# Streaming Delta Events for `_local` Providers Design

> **Status: draft**

## Goal

Make `chat_stream_with_options_dyn` (and `chat_stream_dyn`) on the `_local` providers emit `LlmStreamEvent::Delta { text }` **incrementally** as tokens arrive from the CLI subprocess, instead of collapsing to a single terminal `End` event.

## Motivation

Today all three `_local` providers (claude_local, codex_local, gemini_local) implement streaming as:

```rust
fn chat_stream_with_options_dyn<'a>(...) -> BoxStream<'a, LlmStreamEvent> {
    futures::stream::once(async move {
        let result = tokio::task::spawn_blocking(move || {
            Provider::run_stream_with_options(...)
        }).await...;
        // single End event with full text
        LlmStreamEvent::End { total: LlmResponse { rendered_text: full_text, ... } }
    }).boxed()
}
```

`run_stream_with_options` iterates `reader.lines()` **synchronously**, parses each stream-json line, and accumulates the text into a `String` — but nothing escapes the blocking thread. The consumer only sees the final `End` event. That defeats the purpose of streaming.

Downstream apps (dear-jeongbin in particular) rely on incremental text to power realtime SSE / JSON-RPC streaming to the UI. Without Delta events, they must either (a) keep their own hand-rolled CLI subprocess code, or (b) degrade UX to "spinner → full text at once".

## Non-Goals

- Changing the non-streaming methods (`chat_dyn` / `chat_with_options_dyn`). They stay blocking-aggregated.
- Adding new `LlmStreamEvent` variants. Delta/End/Error are sufficient.
- Guaranteeing Delta granularity matches exactly what the CLI emits. The CLI's chunking (line-based or block-based) is the floor.
- Structured block/thinking/tool-use events. Delta is plain text only.
- Cloud providers (claude, codex, gemini, copilot, openai_compat). Their HTTP streaming is separate scope; this spec targets `_local` only because that's what regressed against the v0.1 baseline.

## Architecture

Replace the `spawn_blocking → stream::once` pattern with fully-async line reading via `tokio::io::AsyncBufReadExt::lines()` wrapped in `async_stream::stream!`.

### Key shift

| v0.2.0 (current) | v0.3.0 (target) |
|---|---|
| `std::process::Command` | `tokio::process::Command` |
| `std::io::BufReader::lines()` | `tokio::io::BufReader::lines()` |
| `spawn_blocking` wraps the whole parse | Async `while let Some(line) = lines.next_line().await?` |
| `futures::stream::once(...)` | `async_stream::stream! { yield Delta ... yield End }` |
| Single End event at completion | Delta per assistant text chunk + single End with totals |

### Flow per provider

```
spawn child process (tokio::process::Command)
├── stdout → tokio BufReader → lines()
│     for each line:
│       parse JSON
│       match event type:
│         text delta (assistant/content_block_delta/item.completed) → yield Delta { text }
│         usage/model/cost → update accumulator
│         error → yield Error { message } and return
├── stderr → concurrent drain task (buffer into String, join at end)
├── after EOF: wait for exit status
│     if non-zero → yield Error
│     else → yield End { total: LlmResponse { accumulated_text, model, cost, tokens } }
```

`accumulated_text` is still tracked because End carries the full canonical text for consumers that only want the terminal result (e.g., calling `chat_with_options_dyn` continues to aggregate via the non-streaming path, and streaming consumers who need the total get it in End).

### Backward compatibility

`chat_dyn` / `chat_with_options_dyn` (non-streaming) keep returning the aggregated full text. No behavior change.

`chat_stream_dyn` / `chat_stream_with_options_dyn` **change observable behavior**: they now yield multiple Delta events before End. Any consumer that only reads the terminal End event continues to work unchanged. Consumers that treated the stream as "one delta + one end" (the literal current shape) will see many deltas instead of one — functionally an improvement, but a behavior change.

This is a minor-version bump at most (v0.3.0); no type signatures change.

## Per-Provider Implementation Detail

### claude_local (stream-json format)

Delta trigger: `type: "assistant"` with `message.content[].type == "text"`.

```rust
"assistant" => {
    if let Some(content_arr) = event.get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
    {
        for part in content_arr {
            if part.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    if !text.is_empty() {
                        accumulated_text.push_str(text);
                        yield LlmStreamEvent::Delta { text: text.to_string() };
                    }
                }
            }
        }
    }
    // capture model + usage as before
}
"result" => {
    // final totals: cost, is_error, usage. Don't emit Delta — End event carries these.
}
```

Note: Claude CLI emits BOTH `assistant` events (per block) AND a final `result` event that contains the full text again. Only emit Delta from `assistant` events to avoid duplicate text.

### codex_local (thread.started / item.completed / turn.completed)

Delta trigger: `type: "item.completed"` with `item.type == "agent_message"`.

```rust
"item.completed" => {
    if let Some(item) = event.get("item").and_then(|v| v.as_object()) {
        if item.get("type").and_then(|t| t.as_str()) == Some("agent_message") {
            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                if !text.is_empty() {
                    accumulated_text.push_str(text);
                    yield LlmStreamEvent::Delta { text: text.to_string() };
                }
            }
        }
    }
}
"thread.started" => { /* capture model */ }
"turn.completed" => { /* capture usage totals */ }
"turn.failed" | "error" => {
    yield LlmStreamEvent::Error { message };
    return;
}
```

Note: Codex `item.completed` arrives at item boundaries (typically end of agent message), not token-by-token. Delta granularity is coarse — still useful for "I've started typing" signaling, and matches what dear-jeongbin currently produces.

### gemini_local (plain text, line-based)

Gemini CLI outputs plain text on stdout (no JSON framing). Delta is trivially "each line as it arrives".

```rust
while let Some(line) = lines.next_line().await? {
    if !line.is_empty() {
        accumulated_text.push_str(&line);
        accumulated_text.push('\n');
        yield LlmStreamEvent::Delta { text: format!("{line}\n") };
    }
}
```

Token usage isn't available from Gemini CLI output, so End carries `input_tokens: None, output_tokens: None` (unchanged from v0.2.0).

## Error Handling

- **CLI spawn failure** → return `Err(LlmError::Other)` synchronously before the stream starts (consumer sees Err at stream construction).
- **Parse error mid-stream** → skip the malformed line, continue. Do not abort.
- **CLI exits non-zero** → drain stderr, yield `LlmStreamEvent::Error { message }`, then stream ends (no End event).
- **Auth errors** detected in stderr → yield `Error` with `LlmError::Auth`-style message, same substring detection as today.

## Testing Strategy

1. **Unit tests for event ordering** (all 3 providers):
   - Mock the CLI subprocess (use a helper shell script or pre-recorded fixture fed to `tokio::io::DuplexStream`).
   - Assert: N Delta events followed by exactly one End, text concatenation equals total.
2. **Unit test for error path**: CLI exits 1 → consumer sees Error event, no End.
3. **Unit test for empty stream**: CLI outputs nothing → consumer sees End with empty text.
4. **Integration test (ignored, needs real CLI)**: Run a short prompt, assert at least 2 Delta events arrive before End and their concat matches End.total.rendered_text.

## Versioning

Bump to **v0.3.0**. Behavior change in streaming methods is observable. No type-level breaking changes, so struct/trait signatures are identical between v0.2.0 and v0.3.0.

CHANGELOG:
```
## [0.3.0] - 2026-04-XX

### Changed
- `chat_stream_dyn` and `chat_stream_with_options_dyn` on claude_local, codex_local,
  gemini_local now emit incremental `LlmStreamEvent::Delta { text }` events as tokens
  arrive from the CLI, instead of collapsing to a single terminal End event.
  Consumers that only read End continue to work unchanged.

### Internal
- Replaced `std::process::Command` + `spawn_blocking` with `tokio::process::Command`
  + async `AsyncBufReadExt::lines()` in all three `_local` stream paths.
```

## Open Questions

1. **Claude CLI `assistant` event duplication with `result`**: Verify via `claude -p --output-format stream-json --verbose "hello"` that `result.result` == concat of all `assistant.message.content[].text`. If yes, suppress `result` text. If no, decide precedence.
2. **Codex `item.completed` coarseness**: Acceptable? Or should we look into a finer-grained event? Decision: acceptable for v0.3.0 — matches dear-jeongbin's current UX floor.
3. **Should `Delta` text be trimmed?** No. Deliver whatever CLI emits, byte-for-byte. Accumulation uses raw push_str. This preserves whitespace that might matter (markdown list indent, code block newlines).

## Migration Impact for dear-jeongbin

After v0.3.0:
- `generate_streaming_with_sink_for_task` can be replaced with:
  ```rust
  let mut stream = provider.chat_stream_with_options_dyn(&system, &msgs, &opts, &Value::Null);
  while let Some(event) = stream.next().await {
      match event {
          LlmStreamEvent::Delta { text } => { chunk_tx.send(text).await?; }
          LlmStreamEvent::End { total } => { return Ok(AiCompletionResult::from(total)); }
          LlmStreamEvent::Error { message } => { return Err(anyhow!(message)); }
      }
  }
  ```
- ~700 LOC of hand-rolled streaming code removed from `ai.rs`.
- Phase 2 of the dear-jeongbin migration (see `dear-jeongbin/docs/llm/2026-04-23-llm-router-migration.md`) depends on this release.
