# codex_local Provider

## Purpose

Add a CLI-based OpenAI Codex provider (`codex_local`) that invokes the `codex` CLI (`codex exec --json`) for non-interactive LLM calls, following the same pattern as `gemini_local`.

## File Changes

| File | Change |
|------|--------|
| `src/codex_local.rs` | **Create** — new CLI provider module |
| `src/lib.rs` | Register `codex_local` in registry, add `list_models` dispatch, add `codex_local` module declaration |
| `docs/llm/codex_local.md` | **Create** — this design doc |

## Implementation Details

### CLI Invocation

```
codex exec --json --dangerously-bypass-approvals-and-sandbox --sandbox danger-full-access --model <model> --prompt "<combined_prompt>"
```

**Key flags:**
- `--json` — outputs JSONL events to stdout
- `--dangerously-bypass-approvals-and-sandbox` — no interactive prompts
- `--sandbox danger-full-access` — full disk access for the agent
- `--model <model>` — model selection (may be restricted by ChatGPT account type)
- `--ephemeral` — don't persist session files

### JSON Output Format (from `codex exec --json`)

```jsonl
{"type":"thread.started","thread_id":"..."}
{"type":"turn.started"}
{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"response text"}}
{"type":"turn.completed","usage":{"input_tokens":24152,"cached_input_tokens":1920,"output_tokens":5}}
```

**Parsing rules:**
- Response text: `item.completed` → `item.text` (concatenate all agent_message items)
- Usage/cost: `turn.completed` → `usage.input_tokens`, `usage.output_tokens`
- Errors: `turn.failed` → `error.message`, or `error` type events

### Provider Struct

```rust
pub struct CodexLocalProvider {
    model: String,       // "auto" or specific model name
    cd: Option<String>,  // working directory
}
```

### Trait Implementation

- `render_dyn` — single-turn: combine system_prompt + user_input into prompt
- `chat_dyn` — multi-turn: format all messages as `[role]: content\n\n` blocks, prepend system prompt
- `chat_stream_dyn` — spawn `codex exec --json`, parse stdout line-by-line, emit `LlmStreamEvent`

### `run_stream` (blocking)

1. Build args: `["exec", "--json", "--dangerously-bypass-approvals-and-sandbox", "--sandbox", "danger-full-access", "--ephemeral", ...]`
2. If model is set and not "auto": add `["--model", model]`
3. Build combined prompt from system + messages
4. Spawn `codex` with stdin from prompt (or `--prompt` arg)
5. Read stdout line-by-line, parse JSONL
6. Parse exit status and stderr on failure
7. Return `(text, model, cost)`

### Cost Estimation

OpenAI Codex pricing (approximate):
- Input tokens: $2.50 / 1M tokens
- Output tokens: $10.00 / 1M tokens

### `list_models`

Codex CLI doesn't have a model listing API. Returns empty vec (same as current `codex` HTTP provider).

### Auth Error Detection

Check stderr for:
- "not authenticated", "please login", "authentication required"
- "unauthorized", "invalid credentials"

### Tests

1. `make_with_empty_model_returns_default` — unit test
2. `make_with_model_returns_provider` — unit test
3. `full_chat_via_cli_returns_text` — integration test (spawn codex, verify response)

## Constraints

- Follow `gemini_local.rs` patterns exactly (module structure, error handling, test style)
- Use `spawn_blocking` for CLI invocation (blocking I/O)
- Stderr goes to `/dev/null` to avoid skill loader noise (or filter it)
- Default model: "auto" (lets CLI choose)
- Stdin: pipe prompt via stdin (more reliable than `--prompt` for long text)

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Input method | stdin pipe | More reliable for long prompts than `--prompt` arg |
| Sandbox | `danger-full-access` | Matches `gemini_local`'s `yolo` mode — no command approval |
| Session | `--ephemeral` | No disk persistence for library use |
| Cost estimation | Token count × OpenAI pricing | Approximate but consistent with gemini_local approach |
| `list_models` | Empty vec | No public model listing API for Codex |
