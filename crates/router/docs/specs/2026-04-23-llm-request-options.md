# LlmRequestOptions + Token Usage + CLI Utilities Design

> **Status: draft**

## Goal

Add the missing capabilities that downstream apps (e.g. dear-jeongbin) need from llm-router so they can drop their hand-rolled CLI subprocess code and depend on llm-router instead.

## Motivation

`dear-jeongbin/src-tauri/src/services/ai.rs` is ~1570 lines wrapping Claude/Codex/Gemini CLIs. Most of it is generic and belongs in llm-router. Today llm-router has only the bare minimum (`render` / `chat` / `chat_stream` with model + system prompt), so dear-jeongbin can't use it without losing features:

- Image attachments (Codex `--image`, Gemini `@path`)
- Tool/permission control (Claude `--allowedTools`, `--add-dir`, `--max-turns`)
- Reasoning effort (Codex `model_reasoning_effort=high`)
- Web search toggle (Codex `--search`, Claude WebSearch tool)
- Working directory override (Codex `-C`, Gemini `--include-directories`)
- Token usage breakdown (input/output separately, not just cost)
- PATH/binary resolution (Tauri GUI launches without shell PATH)
- Prompt sanitization (null bytes from PDF text extracts break execve)

## Non-Goals

- Model alias mapping (`haiku`→`gpt-5.4-mini`). That's app policy, not library policy. Apps map their own aliases before calling.
- DB-backed settings lookup. App responsibility.
- Task tier classification (light/standard/heavy). App responsibility.
- SSE Event wrapping. App responsibility (use existing `LlmStreamEvent`).
- mpsc::Sender style streaming. App can adapt `chat_stream_dyn` BoxStream to mpsc with 5 lines.

## Architecture

Three additive changes, no breaking API changes (additive trait method with default impl, optional struct fields).

### 1. `LlmRequestOptions` struct (new public API)

```rust
#[derive(Debug, Clone, Default)]
pub struct LlmRequestOptions {
    /// Image file paths to attach (Claude/Codex via flag, Gemini via @path).
    pub image_paths: Vec<PathBuf>,
    /// Working directory for the subprocess. Affects file resolution.
    pub working_dir: Option<PathBuf>,
    /// Allow web search tool. Codex: `--search`. Claude: WebSearch+WebFetch.
    pub allow_web_search: bool,
    /// Allow local file read tool. Claude: Read tool. Codex: implicit.
    pub allow_local_read: bool,
    /// Maximum agent turns (Claude `--max-turns`). None = provider default.
    pub max_turns: Option<u32>,
    /// Reasoning effort (Codex `model_reasoning_effort`). Values: "low"|"medium"|"high".
    pub reasoning_effort: Option<String>,
}
```

### 2. Extend `LlmResponse` (additive — Option<T> fields, no constructor break)

```rust
pub struct LlmResponse {
    pub rendered_text: String,
    pub model: String,
    pub estimated_cost_usd: f64,
    // NEW:
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}
```

Existing constructions break unless they use struct update syntax. Mitigation: bump to **v0.2.0** semver (justified by openai_compat/base_url work also being breaking).

### 3. Trait method (additive, default impl delegates to existing)

```rust
pub trait LlmProviderDyn: Send + Sync {
    // existing render_dyn / chat_dyn / chat_stream_dyn unchanged
    
    /// Multi-turn chat with explicit options. Default ignores options.
    fn chat_with_options_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        options: &'a LlmRequestOptions,
        result_json: &'a Value,
    ) -> Pin<Box<dyn Future<Output = Result<LlmResponse, LlmError>> + Send + 'a>> {
        // Default: ignore options for backward compat
        self.chat_dyn(system_prompt, messages, result_json)
    }
    
    fn chat_stream_with_options_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        options: &'a LlmRequestOptions,
        result_json: &'a Value,
    ) -> BoxStream<'a, LlmStreamEvent> {
        self.chat_stream_dyn(system_prompt, messages, result_json)
    }
}
```

Each `_local` provider overrides these to actually wire options into CLI args. Cloud providers (`claude`, `codex`, `gemini`, `copilot`, `openai_compat`) override to wire into HTTP request bodies (image as base64 multipart, search tool definitions, max_tokens, etc).

### 4. `cli_utils` module (new public module)

Generic helpers extracted from dear-jeongbin's `ai.rs`:

```rust
pub mod cli_utils {
    /// Resolves a CLI binary by name. Tries: env var ${NAME_UPPER}_CLI_PATH,
    /// then /opt/homebrew/bin, /usr/local/bin, /usr/bin, $HOME/.local/bin,
    /// $HOME/bin. Falls back to shell `command -v <name>`. Last resort: bare name.
    /// 
    /// Critical for Tauri/desktop apps where the GUI launch context has no PATH.
    pub fn resolve_binary(name: &str) -> PathBuf;
    
    /// Builds a PATH env string with common tool dirs prepended to $PATH.
    /// Use as `cmd.env("PATH", build_path_env())` before spawn.
    pub fn build_path_env() -> String;
    
    /// Removes characters that break execve / CLI args:
    /// - null bytes (\0): execve forbids these
    /// - ASCII C0 control chars except \t \n \r
    /// 
    /// Common need when prompt contains text from PDF/DOCX extraction.
    pub fn sanitize_prompt(input: &str) -> String;
}
```

## Per-Provider Implementation Detail

### claude_local

| Option | Wiring |
|---|---|
| `image_paths` | Append `--image <path>` per image. (verify Claude CLI flag — fall back to in-prompt `[image: path]` markers if unsupported) |
| `working_dir` | `Command::current_dir(...)` + `--add-dir <path>` |
| `allow_web_search` | Add `WebSearch,WebFetch,Read` to `--allowedTools` |
| `allow_local_read` | Add `Read` to `--allowedTools` |
| `max_turns` | `--max-turns <n>` |
| `reasoning_effort` | Ignored (Claude has no equivalent flag) |

### codex_local

| Option | Wiring |
|---|---|
| `image_paths` | `--image <path>` per image |
| `working_dir` | `-C <path>` |
| `allow_web_search` | `--search` |
| `allow_local_read` | Already implicit in Codex sandbox |
| `max_turns` | Codex doesn't expose this; ignored |
| `reasoning_effort` | `-c model_reasoning_effort="<value>"` |

### gemini_local

| Option | Wiring |
|---|---|
| `image_paths` | Append `\n@<abs_path>` lines to prompt (Gemini CLI's read_many_files trigger) |
| `working_dir` | `Command::current_dir(...)` + `--include-directories <path>` |
| `allow_web_search` | Gemini CLI has no flag; ignored |
| `allow_local_read` | Implicit via `@path` |
| `max_turns` | Ignored |
| `reasoning_effort` | Ignored |

## Versioning

Bump to **v0.2.0** because:
1. The in-progress `openai_compat` work already changed `ProviderFactory` signature (breaking).
2. `LlmResponse` gains fields (technically breaking for struct construction).
3. New trait methods are additive (have default impls), but exposing `LlmRequestOptions` as a new public API is a meaningful surface.

CHANGELOG.md will document everything together at v0.2.0 release.

## Open Questions

1. **Option name bikeshed** — `LlmRequestOptions` vs `ChatOptions` vs `RequestOptions`? Pick `LlmRequestOptions` for grep-ability (matches `LlmResponse`/`LlmError` convention).
2. **Does Claude CLI actually accept `--image`?** Need to `claude --help` to confirm. If not, document as unsupported and skip the wiring for claude_local in Phase 3.
3. **Should `cli_utils` be feature-gated?** Pure-cloud users don't need spawn helpers. Decision: keep always-on. The functions are tiny (~30 LOC each) and importing them costs nothing if unused.

## Testing Strategy

- Unit tests for `sanitize_prompt` (null byte, control chars, valid input).
- Unit tests for `build_path_env` (no duplicates, system dirs first).
- Provider integration tests gated behind `--ignored` (need real CLI installed): smoke test that options reach the CLI by inspecting which flags get added.
- For each `_local` provider, add a `#[test]` that calls `build_args(options)` and asserts the CLI arg vec contains expected flags.

## Migration Impact for dear-jeongbin

After v0.2.0:
- `ai.rs` shrinks from 1570 → ~400 LOC
- Removed: 3 provider-specific spawn functions × 2 (sync + streaming) = ~700 LOC
- Removed: `resolve_cli_binary`, `build_cli_path_env`, `sanitize_prompt` (~80 LOC)
- Kept: task tier resolution, DB settings lookup, model alias mapping, SSE/mpsc adapters (~400 LOC) — these are app policy, not library
