# claude_local Provider

## Purpose

Add a CLI-based Claude Code provider (`claude_local`) that invokes the `claude` CLI (`claude -p --output-format json`) for non-interactive LLM calls, following the same pattern as `gemini_local` and `codex_local`.

## File Changes

| File | Change |
|------|--------|
| `src/claude_local.rs` | **Create** — new CLI provider module |
| `src/lib.rs` | Register `claude_local` in registry, add `list_models` dispatch, add `claude_local` module declaration |
| `docs/llm/claude_local.md` | **Create** — this design doc |

## Implementation Details

### CLI Invocation

```bash
claude -p --output-format json --dangerously-skip-permissions --no-session-persistence --model <model> "<prompt>"
```

**Key flags:**
- `-p` / `--print` — print response and exit (non-interactive)
- `--output-format json` — single JSON result (not streaming)
- `--dangerously-skip-permissions` — no permission prompts
- `--no-session-persistence` — no session files
- `--model <model>` — model selection (aliases like "sonnet", "opus", or full names like "claude-sonnet-4-6")

**NOT using `--bare`** — it skips OAuth/keychain which breaks authentication for Claude subscription users.

### JSON Output Format (from `claude -p --output-format json`)

```json
{
  "type": "result",
  "subtype": "success",
  "is_error": false,
  "result": "Hello",
  "stop_reason": "end_turn",
  "total_cost_usd": 0.105699,
  "usage": {
    "input_tokens": 3,
    "cache_creation_input_tokens": 28168,
    "output_tokens": 4
  },
  "modelUsage": {
    "claude-sonnet-4-6": {
      "inputTokens": 3,
      "outputTokens": 4,
      "costUSD": 0.105699
    }
  }
}
```

Parsing:
- Response text: `result` field
- Cost: `total_cost_usd` or `modelUsage.<model>.costUSD`
- Model: `modelUsage` keys (first key)
- Usage: `usage.input_tokens`, `usage.output_tokens`
- Errors: `is_error: true` or `subtype: "error"`

### Provider Struct

```rust
pub struct ClaudeLocalProvider {
    model: String,       // "auto" or alias (sonnet, opus, haiku) or full name
}
```

### `list_models` — Config File Parsing

Read `~/.claude/claude.json` for the configured `defaultModel`:

```json
{
  "settings": {
    "env": {
      "ANTHROPIC_ACCESS_TOKEN": "..."
    }
  },
  "defaultModel": "claude-sonnet-4-6",
  "version": "..."
}
```

If not found, fall back to `CLAUDE_MODEL` env var, then return empty vec.

### Cost Estimation

Claude pricing (approximate, varies by model):
- Use `total_cost_usd` from API response directly (most accurate)
- If unavailable, estimate from token counts:
  - sonnet: input $3.00/M, output $15.00/M
  - haiku: input $0.80/M, output $4.00/M
  - opus: input $15.00/M, output $75.00/M

### Tests

1. `make_with_empty_model_returns_default` — unit test
2. `make_with_model_returns_provider` — unit test
3. `list_models_reads_configured_model` — integration test
4. `full_chat_via_cli_returns_text` — integration test

## Constraints

- Follow `gemini_local.rs` and `codex_local.rs` patterns exactly
- Use `spawn_blocking` for CLI invocation
- Auth via Claude subscription (OAuth/keychain) — no API key needed
- Default model: "auto" (uses configured default)

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Output format | `--output-format json` (single result) | Simpler than streaming; Claude response is fast |
| Auth | Claude subscription (no `--bare`) | `--bare` breaks OAuth/keychain auth |
| Session | `--no-session-persistence` | No disk persistence for library use |
| `list_models` | Read `~/.claude/claude.json` `defaultModel` | Most accurate — reflects user's actual config |
| Cost | Use `total_cost_usd` from response | Most accurate vs. token-based estimation |
