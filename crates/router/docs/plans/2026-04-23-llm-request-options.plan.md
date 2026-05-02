# LlmRequestOptions + Token Usage + CLI Utilities Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `LlmRequestOptions`, token usage breakdown, and `cli_utils` module to llm-router so downstream apps (dear-jeongbin) can drop their hand-rolled CLI wrapper code.

**Architecture:** Three additive surfaces — typed options struct, optional response token fields, new `cli_utils` module. Plus a new trait method `chat_with_options_dyn` (default impl forwards to `chat_dyn` for backward compat). Per-provider overrides wire options into actual CLI args.

**Tech Stack:** Rust 1.85+, tokio, futures, async-stream, serde_json, dirs.

**Spec:** `docs/specs/2026-04-23-llm-request-options.md` — read this first.

**Working tree precondition:** As of 2026-04-23 the working tree contains in-progress `openai_compat` provider + `(api_key, model)` → `(api_key, model, base_url)` factory signature change. **Pre-Phase below finalizes that work first.**

---

## Pre-Phase: Finalize openai_compat work

Decision needed: do you want to ship the `openai_compat` provider as part of v0.2.0 or revert it? This plan **assumes ship** (it's compatible with the new options work).

### Task 0: Verify the in-progress openai_compat work compiles + passes tests

**Files:** None modified, just verification.

- [ ] **Step 1: Inspect uncommitted state**

```bash
cd ~/projects/llm-router
git status
git diff --stat HEAD
```

- [ ] **Step 2: Run full test suite to verify the in-progress work isn't broken**

```bash
cargo build
cargo test
```

Expected: builds cleanly, tests pass. If broken, fix the openai_compat work first before proceeding to Phase 1.

- [ ] **Step 3: Commit the openai_compat work as one focused commit**

```bash
cd ~/projects/llm-router
git add src/openai_compat.rs src/lib.rs src/claude.rs src/codex.rs src/copilot.rs src/dummy.rs src/gemini.rs src/claude_local.rs src/codex_local.rs src/gemini_local.rs examples/minimal.rs
git commit -m "feat: add openai_compat provider + base_url factory parameter

BREAKING CHANGE: ProviderFactory signature changed from
(api_key, model) to (api_key, model, base_url). All built-in
providers updated. Custom providers must update their make() fn."
```

---

## Phase 1: Add `LlmRequestOptions` + extended `LlmResponse` + new trait methods

### Task 1: Add `LlmRequestOptions` struct

**Files:**
- Modify: `src/lib.rs` (add struct + re-export, ~20 lines)
- Test: `src/lib.rs` (inline `#[cfg(test)] mod request_options_tests`)

- [ ] **Step 1: Write the failing test for default values**

Add to `src/lib.rs`:

```rust
#[cfg(test)]
mod request_options_tests {
    use super::*;

    #[test]
    fn default_options_are_empty() {
        let opts = LlmRequestOptions::default();
        assert!(opts.image_paths.is_empty());
        assert!(opts.working_dir.is_none());
        assert!(!opts.allow_web_search);
        assert!(!opts.allow_local_read);
        assert!(opts.max_turns.is_none());
        assert!(opts.reasoning_effort.is_none());
    }

    #[test]
    fn options_can_be_constructed_via_struct_update() {
        let opts = LlmRequestOptions {
            allow_web_search: true,
            max_turns: Some(5),
            ..Default::default()
        };
        assert!(opts.allow_web_search);
        assert_eq!(opts.max_turns, Some(5));
    }
}
```

- [ ] **Step 2: Run test — expect compile failure (struct doesn't exist)**

```bash
cargo test request_options_tests
```

Expected: FAIL with `LlmRequestOptions not found`.

- [ ] **Step 3: Add the struct in `src/lib.rs`**

Add after the `ChatMessage` struct, before `LlmResponse`:

```rust
use std::path::PathBuf;

/// Options that modify how a provider executes a request.
///
/// Different providers honor different subsets of these options. Unsupported
/// options are silently ignored. The default value is "no options" which
/// makes existing `chat_dyn` behavior the floor.
#[derive(Debug, Clone, Default)]
pub struct LlmRequestOptions {
    /// Image file paths to attach to the request.
    /// - claude_local: appended as `--image <path>` (verify CLI support; falls back to ignore)
    /// - codex_local: appended as `--image <path>`
    /// - gemini_local: appended to prompt as `@<abs_path>` to trigger read_many_files
    pub image_paths: Vec<PathBuf>,

    /// Working directory for the provider subprocess.
    /// - claude_local: `current_dir(...)` + `--add-dir <path>`
    /// - codex_local: `-C <path>`
    /// - gemini_local: `current_dir(...)` + `--include-directories <path>`
    pub working_dir: Option<PathBuf>,

    /// Allow the model to perform web searches.
    /// - claude_local: adds `WebSearch,WebFetch` to `--allowedTools`
    /// - codex_local: passes `--search`
    /// - gemini_local: ignored (no flag)
    pub allow_web_search: bool,

    /// Allow the model to read local files.
    /// - claude_local: adds `Read` to `--allowedTools`
    /// - codex_local: implicit in sandbox
    /// - gemini_local: implicit via `@path`
    pub allow_local_read: bool,

    /// Maximum agent turns.
    /// - claude_local: `--max-turns <n>`
    /// - other providers: ignored
    pub max_turns: Option<u32>,

    /// Reasoning effort. Values: "low", "medium", "high".
    /// - codex_local: `-c model_reasoning_effort="<value>"`
    /// - other providers: ignored
    pub reasoning_effort: Option<String>,
}
```

- [ ] **Step 4: Run test — expect PASS**

```bash
cargo test request_options_tests
```

Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs
git commit -m "feat: add LlmRequestOptions struct for typed per-request options

Per-request options that providers may honor: image attachments,
working directory, web search, local read, max turns, reasoning effort.
Each provider supports a different subset; unsupported options are
silently ignored. Default value is no-op so existing call sites
continue working."
```

---

### Task 2: Extend `LlmResponse` with token breakdown fields

**Files:**
- Modify: `src/lib.rs` (add 2 fields)
- Modify: every provider file that constructs `LlmResponse` (`claude.rs`, `claude_local.rs`, `codex.rs`, `codex_local.rs`, `copilot.rs`, `dummy.rs`, `gemini.rs`, `gemini_local.rs`, `openai_compat.rs`)
- Test: `src/lib.rs` (extend the existing inline tests)

- [ ] **Step 1: Add the test for new fields**

Add to `src/lib.rs`:

```rust
#[cfg(test)]
mod response_token_tests {
    use super::*;

    #[test]
    fn response_default_token_fields_are_none() {
        let resp = LlmResponse {
            rendered_text: String::new(),
            model: String::new(),
            estimated_cost_usd: 0.0,
            input_tokens: None,
            output_tokens: None,
        };
        assert_eq!(resp.input_tokens, None);
        assert_eq!(resp.output_tokens, None);
    }
}
```

- [ ] **Step 2: Run test — expect compile failure**

```bash
cargo test response_token_tests
```

Expected: FAIL with `unknown field input_tokens`.

- [ ] **Step 3: Add fields to `LlmResponse` in `src/lib.rs`**

Find the `LlmResponse` struct and replace with:

```rust
#[derive(Debug)]
pub struct LlmResponse {
    pub rendered_text: String,
    pub model: String,
    pub estimated_cost_usd: f64,
    /// Input (prompt) tokens consumed. None if the provider does not report it.
    pub input_tokens: Option<u32>,
    /// Output (completion) tokens generated. None if the provider does not report it.
    pub output_tokens: Option<u32>,
}
```

- [ ] **Step 4: Update every provider that constructs `LlmResponse`**

Run to find all sites:

```bash
grep -rn "LlmResponse {" src/
```

For each match, add `input_tokens: None, output_tokens: None,` to the struct literal. Example for `claude_local.rs`:

```rust
// BEFORE
Ok(LlmResponse {
    rendered_text: result.0,
    model: result.1,
    estimated_cost_usd: result.2,
})

// AFTER
Ok(LlmResponse {
    rendered_text: result.0,
    model: result.1,
    estimated_cost_usd: result.2,
    input_tokens: None,
    output_tokens: None,
})
```

For providers that already track tokens internally (e.g. `codex_local` has `input_tokens: u64` local var), thread the values through:

```rust
// codex_local.rs run_stream signature changes:
fn run_stream(...) -> Result<(String, String, f64, Option<u32>, Option<u32>), LlmError>
// Return tuple becomes:
Ok((full_text, model, cost, Some(input_tokens as u32), Some(output_tokens as u32)))
// And chat_impl wires them into LlmResponse:
Ok(LlmResponse {
    rendered_text: result.0,
    model: result.1,
    estimated_cost_usd: result.2,
    input_tokens: result.3,
    output_tokens: result.4,
})
```

Same pattern for `claude_local` (parses usage from stream-json) and any cloud providers that parse usage from API responses.

- [ ] **Step 5: Run all tests — expect PASS**

```bash
cargo build
cargo test
```

Expected: clean build, all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/
git commit -m "feat: add input_tokens/output_tokens to LlmResponse

Providers that parse token usage now report it via Option<u32> fields.
Providers without usage reporting set both to None.

BREAKING: struct literal construction of LlmResponse must add the new
fields (or use ..Default::default() once Default is impl'd)."
```

---

### Task 3: Add `chat_with_options_dyn` and `chat_stream_with_options_dyn` trait methods

**Files:**
- Modify: `src/lib.rs` (add trait methods with default impls)
- Test: new file `tests/options_passthrough.rs`

- [ ] **Step 1: Write the integration test**

Create `tests/options_passthrough.rs`:

```rust
use llm_router::{create_provider, ChatMessage, LlmRequestOptions};

#[tokio::test]
async fn dummy_provider_accepts_options_and_ignores_them() {
    let provider = create_provider("dummy", "", "", "").expect("dummy creates");
    let messages = vec![ChatMessage {
        role: "user".into(),
        content: "hi".into(),
    }];
    let options = LlmRequestOptions {
        allow_web_search: true,
        max_turns: Some(3),
        ..Default::default()
    };
    let resp = provider
        .chat_with_options_dyn("system", &messages, &options, &serde_json::json!({}))
        .await
        .expect("dummy responds");
    assert!(!resp.rendered_text.is_empty());
}
```

- [ ] **Step 2: Run test — expect compile failure (method not on trait)**

```bash
cargo test --test options_passthrough
```

Expected: FAIL with `chat_with_options_dyn not found`.

- [ ] **Step 3: Add the trait methods in `src/lib.rs`**

Inside `pub trait LlmProviderDyn`, after the existing `chat_stream_dyn` default method, add:

```rust
    /// Multi-turn chat with explicit per-request options.
    ///
    /// Default implementation discards options and forwards to `chat_dyn`.
    /// Providers that honor any options (e.g. `_local` providers wiring CLI
    /// flags) override this method.
    fn chat_with_options_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        _options: &'a LlmRequestOptions,
        result_json: &'a Value,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + 'a>,
    > {
        self.chat_dyn(system_prompt, messages, result_json)
    }

    /// Streaming variant with options. Default forwards to `chat_stream_dyn`.
    fn chat_stream_with_options_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        _options: &'a LlmRequestOptions,
        result_json: &'a Value,
    ) -> BoxStream<'a, LlmStreamEvent> {
        self.chat_stream_dyn(system_prompt, messages, result_json)
    }
```

- [ ] **Step 4: Run test — expect PASS**

```bash
cargo test --test options_passthrough
```

Expected: 1 passed.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs tests/options_passthrough.rs
git commit -m "feat: add chat_with_options_dyn trait methods

Adds optional sync + streaming variants accepting LlmRequestOptions.
Default impls forward to existing chat_dyn / chat_stream_dyn so all
existing providers compile unchanged. Providers that honor options
override these methods (next phase wires _local providers)."
```

---

## Phase 2: `cli_utils` module

### Task 4: Add `cli_utils::sanitize_prompt`

**Files:**
- Create: `src/cli_utils.rs`
- Modify: `src/lib.rs` (add `pub mod cli_utils;`)
- Test: inline in `src/cli_utils.rs`

- [ ] **Step 1: Create the file with the failing test**

Create `src/cli_utils.rs`:

```rust
//! Utilities for spawning CLI subprocesses, primarily used by the `_local`
//! providers but exposed publicly for downstream apps building their own
//! provider integrations.

/// Removes characters that break execve / CLI argument passing:
/// - Null bytes (`\0`): execve forbids these in argv
/// - ASCII C0 control characters except `\t`, `\n`, `\r`
///
/// Common need when prompts contain text extracted from PDF/DOCX/PPTX.
pub fn sanitize_prompt(input: &str) -> String {
    input
        .chars()
        .filter(|&c| c != '\0')
        .filter(|&c| c == '\t' || c == '\n' || c == '\r' || !c.is_control())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_null_bytes() {
        assert_eq!(sanitize_prompt("a\0b\0c"), "abc");
    }

    #[test]
    fn removes_c0_controls_except_whitespace() {
        let input = "hello\x01\x07world\t\nfoo";
        assert_eq!(sanitize_prompt(input), "helloworld\t\nfoo");
    }

    #[test]
    fn preserves_normal_text() {
        let input = "한글 ABC 123 !@#$%";
        assert_eq!(sanitize_prompt(input), input);
    }

    #[test]
    fn preserves_tab_newline_carriage_return() {
        assert_eq!(sanitize_prompt("a\tb\nc\rd"), "a\tb\nc\rd");
    }

    #[test]
    fn empty_input() {
        assert_eq!(sanitize_prompt(""), "");
    }
}
```

- [ ] **Step 2: Add module declaration in `src/lib.rs`**

Add at the top of `src/lib.rs` near other `pub mod` lines:

```rust
pub mod cli_utils;
```

- [ ] **Step 3: Run tests**

```bash
cargo test cli_utils
```

Expected: 5 passed.

- [ ] **Step 4: Commit**

```bash
git add src/cli_utils.rs src/lib.rs
git commit -m "feat: add cli_utils::sanitize_prompt

Removes null bytes and ASCII control chars (except tab/newline/CR)
that break execve when prompts contain extracted document text."
```

---

### Task 5: Add `cli_utils::resolve_binary` and `build_path_env`

**Files:**
- Modify: `src/cli_utils.rs` (add 2 functions + tests)

- [ ] **Step 1: Write the failing tests**

Add to `src/cli_utils.rs`:

```rust
use std::path::PathBuf;

/// Resolves a CLI binary by name. Search order:
/// 1. Environment variable `${NAME_UPPER}_CLI_PATH` (e.g. `CLAUDE_CLI_PATH`)
/// 2. Common install dirs: /opt/homebrew/bin, /usr/local/bin, /usr/bin,
///    $HOME/.local/bin, $HOME/bin
/// 3. Shell `command -v <name>` (uses $SHELL or /bin/zsh)
/// 4. Bare name (relies on subprocess inheriting PATH)
///
/// Critical for desktop apps (Tauri/Electron) where the GUI launch context
/// has no shell PATH.
pub fn resolve_binary(name: &str) -> PathBuf {
    let env_key = format!("{}_CLI_PATH", name.to_ascii_uppercase());
    if let Ok(path) = std::env::var(&env_key) {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    for candidate in common_paths(name) {
        if candidate.exists() {
            return candidate;
        }
    }

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let output = std::process::Command::new(shell)
        .arg("-lc")
        .arg(format!("command -v {name}"))
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let resolved = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !resolved.is_empty() {
                return PathBuf::from(resolved);
            }
        }
    }

    PathBuf::from(name)
}

fn common_paths(name: &str) -> Vec<PathBuf> {
    let mut candidates = vec![
        PathBuf::from("/opt/homebrew/bin").join(name),
        PathBuf::from("/usr/local/bin").join(name),
        PathBuf::from("/usr/bin").join(name),
    ];
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(PathBuf::from(&home).join(".local/bin").join(name));
        candidates.push(PathBuf::from(home).join("bin").join(name));
    }
    candidates
}

/// Builds a PATH env string with common tool dirs prepended to current $PATH.
/// Use as `cmd.env("PATH", build_path_env())` before spawn — this is the only
/// reliable way to find `claude`/`codex`/`gemini` from a Tauri GUI launch.
pub fn build_path_env() -> String {
    let mut segments = Vec::new();

    for fixed in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
        push_unique(&mut segments, fixed.to_string());
    }
    if let Ok(home) = std::env::var("HOME") {
        push_unique(&mut segments, format!("{home}/.local/bin"));
        push_unique(&mut segments, format!("{home}/bin"));
    }
    if let Ok(existing) = std::env::var("PATH") {
        for segment in existing.split(':') {
            push_unique(&mut segments, segment.to_string());
        }
    }

    segments.join(":")
}

fn push_unique(segments: &mut Vec<String>, candidate: String) {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return;
    }
    if segments.iter().any(|s| s == trimmed) {
        return;
    }
    segments.push(trimmed.to_string());
}

#[cfg(test)]
mod resolve_tests {
    use super::*;

    #[test]
    fn env_var_override_takes_priority() {
        // SAFETY: setting an env var in tests can race with parallel tests.
        // Use a unique name unlikely to exist anywhere else.
        // SAFETY justification: this var is unique to this test (NONEXISTENT_BINARY_XYZ_CLI_PATH).
        unsafe {
            std::env::set_var("NONEXISTENT_BINARY_XYZ_CLI_PATH", "/custom/path/to/binary");
        }
        let resolved = resolve_binary("nonexistent_binary_xyz");
        assert_eq!(resolved, PathBuf::from("/custom/path/to/binary"));
        unsafe {
            std::env::remove_var("NONEXISTENT_BINARY_XYZ_CLI_PATH");
        }
    }

    #[test]
    fn returns_bare_name_for_unknown_binary() {
        // Should fall back to bare name when nothing else found.
        let resolved = resolve_binary("definitely_does_not_exist_zzz_99");
        assert_eq!(resolved, PathBuf::from("definitely_does_not_exist_zzz_99"));
    }

    #[test]
    fn build_path_env_includes_common_dirs() {
        let path = build_path_env();
        assert!(path.contains("/opt/homebrew/bin"));
        assert!(path.contains("/usr/local/bin"));
        assert!(path.contains("/usr/bin"));
    }

    #[test]
    fn build_path_env_no_duplicates() {
        let path = build_path_env();
        let segments: Vec<&str> = path.split(':').collect();
        let mut seen = std::collections::HashSet::new();
        for segment in &segments {
            assert!(seen.insert(*segment), "duplicate segment: {segment}");
        }
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test cli_utils
```

Expected: 9 passed (5 from sanitize + 4 from resolve).

- [ ] **Step 3: Commit**

```bash
git add src/cli_utils.rs
git commit -m "feat: add cli_utils::resolve_binary and build_path_env

resolve_binary searches env var → common install dirs → shell PATH
→ bare name fallback. Critical for Tauri/Electron GUI launches
where shell PATH isn't inherited.

build_path_env returns a PATH string with /opt/homebrew/bin etc
prepended; pair with Command::env('PATH', ...) before spawn."
```

---

## Phase 3: Wire options into `_local` providers

### Task 6: Wire options into `claude_local`

**Files:**
- Modify: `src/claude_local.rs` (override `chat_with_options_dyn`, extend `build_args`)

- [ ] **Step 1: Add unit test for arg construction**

Add to the existing `mod tests` block in `src/claude_local.rs`:

```rust
#[test]
fn build_args_includes_max_turns_when_set() {
    let opts = crate::LlmRequestOptions {
        max_turns: Some(7),
        ..Default::default()
    };
    let messages = vec![crate::ChatMessage {
        role: "user".into(),
        content: "hi".into(),
    }];
    let args = ClaudeLocalProvider::build_args_with_options("auto", &messages, "sys", &opts);
    assert!(args.iter().any(|a| a == "--max-turns"), "args: {args:?}");
    assert!(args.iter().any(|a| a == "7"), "args: {args:?}");
}

#[test]
fn build_args_includes_allowed_tools_when_search_enabled() {
    let opts = crate::LlmRequestOptions {
        allow_web_search: true,
        ..Default::default()
    };
    let messages = vec![crate::ChatMessage {
        role: "user".into(),
        content: "hi".into(),
    }];
    let args = ClaudeLocalProvider::build_args_with_options("auto", &messages, "sys", &opts);
    let tools_idx = args.iter().position(|a| a == "--allowedTools").expect("--allowedTools present");
    let tools_value = &args[tools_idx + 1];
    assert!(tools_value.contains("WebSearch"), "tools: {tools_value}");
    assert!(tools_value.contains("WebFetch"), "tools: {tools_value}");
}

#[test]
fn build_args_includes_add_dir_when_working_dir_set() {
    let opts = crate::LlmRequestOptions {
        working_dir: Some(std::path::PathBuf::from("/tmp/work")),
        ..Default::default()
    };
    let messages = vec![crate::ChatMessage {
        role: "user".into(),
        content: "hi".into(),
    }];
    let args = ClaudeLocalProvider::build_args_with_options("auto", &messages, "sys", &opts);
    assert!(args.iter().any(|a| a == "--add-dir"), "args: {args:?}");
    assert!(args.iter().any(|a| a == "/tmp/work"), "args: {args:?}");
}
```

- [ ] **Step 2: Run tests — expect compile failure**

```bash
cargo test --lib claude_local
```

Expected: FAIL — `build_args_with_options` not defined.

- [ ] **Step 3: Add `build_args_with_options` and override the trait method**

In `src/claude_local.rs`, add a new associated function next to the existing `build_args`:

```rust
fn build_args_with_options(
    model: &str,
    messages: &[ChatMessage],
    system_prompt: &str,
    options: &crate::LlmRequestOptions,
) -> Vec<String> {
    let mut args = Self::build_args(model, messages, system_prompt);

    // Tools allow-list
    let mut allowed_tools: Vec<&str> = Vec::new();
    if options.allow_web_search {
        allowed_tools.push("WebSearch");
        allowed_tools.push("WebFetch");
        allowed_tools.push("Read");
    } else if options.allow_local_read {
        allowed_tools.push("Read");
    }
    if !allowed_tools.is_empty() {
        args.push("--allowedTools".to_string());
        args.push(allowed_tools.join(","));
    }

    // Working dir → --add-dir
    if let Some(dir) = options.working_dir.as_ref() {
        args.push("--add-dir".to_string());
        args.push(dir.to_string_lossy().to_string());
    }

    // Max turns
    if let Some(max) = options.max_turns {
        args.push("--max-turns".to_string());
        args.push(max.to_string());
    }

    // Image attachments — IMPORTANT: verify Claude CLI accepts --image flag
    // by running `claude --help`. If not supported, this loop is a no-op
    // for end users (CLI rejects unknown flags, so wrap in feature check
    // or document as untested). For now, append optimistically.
    for img in &options.image_paths {
        args.push("--image".to_string());
        args.push(img.to_string_lossy().to_string());
    }

    args
}
```

Then override the trait method (add to the `impl LlmProviderDyn for ClaudeLocalProvider` block):

```rust
fn chat_with_options_dyn<'a>(
    &'a self,
    system_prompt: &'a str,
    messages: &'a [ChatMessage],
    options: &'a crate::LlmRequestOptions,
    _result_json: &'a serde_json::Value,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + 'a>,
> {
    let provider = self.clone();
    let system = system_prompt.to_string();
    let msgs = messages.to_vec();
    let opts = options.clone();
    Box::pin(async move {
        let result = tokio::task::spawn_blocking(move || {
            ClaudeLocalProvider::run_stream_with_options(&provider, &system, &msgs, &opts)
        })
        .await
        .unwrap_or_else(|e| Err(LlmError::Other {
            provider: "claude_local".into(),
            message: format!("spawn_blocking failed: {e}"),
        }))?;

        Ok(LlmResponse {
            rendered_text: result.0,
            model: result.1,
            estimated_cost_usd: result.2,
            input_tokens: result.3,
            output_tokens: result.4,
        })
    })
}
```

And add `run_stream_with_options` (refactor of existing `run_stream` that uses `build_args_with_options` and applies `working_dir` to the `Command`):

```rust
fn run_stream_with_options(
    provider: &ClaudeLocalProvider,
    system_prompt: &str,
    messages: &[ChatMessage],
    options: &crate::LlmRequestOptions,
) -> Result<(String, String, f64, Option<u32>, Option<u32>), LlmError> {
    let args = Self::build_args_with_options(&provider.model, messages, system_prompt, options);
    let mut command = std::process::Command::new(crate::cli_utils::resolve_binary("claude"));
    command.env("PATH", crate::cli_utils::build_path_env());
    if let Some(dir) = options.working_dir.as_ref() {
        command.current_dir(dir);
    }
    let mut cmd = command
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| LlmError::Other {
            provider: "claude_local".into(),
            message: format!("failed to spawn claude CLI: {e}"),
        })?;

    // ... rest of logic identical to existing run_stream, but ALSO parse
    // input_tokens/output_tokens from the stream-json events and return them
    // as the new tuple slots. See existing usage parsing in dear-jeongbin
    // ai.rs lines ~449-456 and ~470-477 for the pattern.

    // For brevity: copy existing run_stream body, replace the return tuple
    // with (text, model, cost, Some(input_tokens), Some(output_tokens)).
    todo!("copy existing run_stream body, parse usage, return 5-tuple")
}
```

> ⚠️ The `todo!()` above is a marker — when implementing, COPY the existing `run_stream` body verbatim, then add usage parsing in the `assistant`/`message_delta`/`result` event handlers (parse `usage.input_tokens` / `usage.output_tokens` from the JSON value), and return the 5-tuple. The existing `run_stream` keeps working unchanged for the default `chat_dyn` path.

Then update the existing `chat_impl` to also wire `input_tokens`/`output_tokens: None` since the no-options path doesn't parse usage:

```rust
// existing chat_impl returns LlmResponse — add the new fields as None
Ok(LlmResponse {
    rendered_text: result.0,
    model: result.1,
    estimated_cost_usd: result.2,
    input_tokens: None,  // existing run_stream doesn't track tokens
    output_tokens: None,
})
```

- [ ] **Step 4: Run tests**

```bash
cargo test --lib claude_local
```

Expected: all unit tests pass (3 new build_args tests + existing tests).

- [ ] **Step 5: Commit**

```bash
git add src/claude_local.rs
git commit -m "feat(claude_local): wire LlmRequestOptions into CLI args

Implements chat_with_options_dyn for ClaudeLocalProvider:
- working_dir → current_dir + --add-dir
- allow_web_search → --allowedTools WebSearch,WebFetch,Read
- allow_local_read → --allowedTools Read
- max_turns → --max-turns N
- image_paths → --image <path> per file (verify CLI flag support)

Existing chat_dyn path unchanged. Token usage parsed from stream-json
when chat_with_options_dyn is used."
```

---

### Task 7: Wire options into `codex_local`

**Files:**
- Modify: `src/codex_local.rs`

Mirror Task 6 structure but with codex-specific arg mappings:

| Option | Codex arg |
|---|---|
| `image_paths` | `--image <path>` per file |
| `working_dir` | `-C <path>` (already supported via `with_working_dir`!) |
| `allow_web_search` | `--search` |
| `allow_local_read` | (no-op, implicit) |
| `max_turns` | (no-op, ignored) |
| `reasoning_effort` | `-c model_reasoning_effort="<value>"` |

- [ ] **Step 1: Add unit tests for arg construction**

Add to `mod tests` in `src/codex_local.rs`:

```rust
#[test]
fn build_args_includes_search_flag_when_enabled() {
    let opts = crate::LlmRequestOptions {
        allow_web_search: true,
        ..Default::default()
    };
    let messages = vec![crate::ChatMessage {
        role: "user".into(),
        content: "hi".into(),
    }];
    let args = CodexLocalProvider::build_args_with_options("auto", None, &messages, "sys", &opts);
    assert!(args.iter().any(|a| a == "--search"), "args: {args:?}");
}

#[test]
fn build_args_includes_reasoning_effort_when_set() {
    let opts = crate::LlmRequestOptions {
        reasoning_effort: Some("high".into()),
        ..Default::default()
    };
    let messages = vec![crate::ChatMessage {
        role: "user".into(),
        content: "hi".into(),
    }];
    let args = CodexLocalProvider::build_args_with_options("auto", None, &messages, "sys", &opts);
    let c_idx = args.iter().position(|a| a == "-c").expect("-c present");
    assert!(args[c_idx + 1].contains("model_reasoning_effort"));
    assert!(args[c_idx + 1].contains("high"));
}

#[test]
fn build_args_includes_image_flags() {
    let opts = crate::LlmRequestOptions {
        image_paths: vec![
            std::path::PathBuf::from("/tmp/a.png"),
            std::path::PathBuf::from("/tmp/b.png"),
        ],
        ..Default::default()
    };
    let messages = vec![crate::ChatMessage {
        role: "user".into(),
        content: "hi".into(),
    }];
    let args = CodexLocalProvider::build_args_with_options("auto", None, &messages, "sys", &opts);
    let image_count = args.iter().filter(|a| *a == "--image").count();
    assert_eq!(image_count, 2);
}
```

- [ ] **Step 2: Run tests — expect compile failure**

```bash
cargo test --lib codex_local
```

Expected: FAIL — `build_args_with_options` not defined.

- [ ] **Step 3: Add `build_args_with_options` to `CodexLocalProvider`**

```rust
fn build_args_with_options(
    model: &str,
    cd: Option<&str>,
    messages: &[ChatMessage],
    system_prompt: &str,
    options: &crate::LlmRequestOptions,
) -> Vec<String> {
    // Start from existing build_args, then append option-driven args.
    // CRITICAL: existing build_args puts the prompt as the LAST arg.
    // Options must be inserted BEFORE the prompt arg.
    let base = Self::build_args(model, cd.or(options.working_dir.as_deref().map(|p| p.to_string_lossy().into_owned()).as_deref()), messages, system_prompt);
    // ... actually, refactor: split base into [args..., prompt] and re-merge.
    let (head, prompt) = base.split_at(base.len() - 1);
    let prompt = prompt[0].clone();
    let mut args: Vec<String> = head.to_vec();

    if options.allow_web_search {
        args.push("--search".into());
    }
    for img in &options.image_paths {
        args.push("--image".into());
        args.push(img.to_string_lossy().into_owned());
    }
    if let Some(effort) = options.reasoning_effort.as_ref() {
        args.push("-c".into());
        args.push(format!("model_reasoning_effort=\"{}\"", effort));
    }

    args.push(prompt);
    args
}
```

> Note: the `cd.or(...)` line is messy because of `&str` lifetimes. Cleaner: inline the option's working_dir directly:

```rust
fn build_args_with_options(
    model: &str,
    cd: Option<&str>,
    messages: &[ChatMessage],
    system_prompt: &str,
    options: &crate::LlmRequestOptions,
) -> Vec<String> {
    // Resolve working dir: explicit cd param takes priority, else option.
    let dir_owned = options
        .working_dir
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned());
    let effective_cd: Option<&str> = cd.or(dir_owned.as_deref());

    let base = Self::build_args(model, effective_cd, messages, system_prompt);
    let (head, prompt) = base.split_at(base.len() - 1);
    let prompt = prompt[0].clone();
    let mut args: Vec<String> = head.to_vec();

    if options.allow_web_search {
        args.push("--search".into());
    }
    for img in &options.image_paths {
        args.push("--image".into());
        args.push(img.to_string_lossy().into_owned());
    }
    if let Some(effort) = options.reasoning_effort.as_ref() {
        args.push("-c".into());
        args.push(format!("model_reasoning_effort=\"{}\"", effort));
    }

    args.push(prompt);
    args
}
```

- [ ] **Step 4: Override the trait method (mirror claude_local Task 6 Step 3)**

Add `chat_with_options_dyn` to the `impl LlmProviderDyn for CodexLocalProvider` block, calling a new `run_stream_with_options` that uses `build_args_with_options` and threads the existing `input_tokens`/`output_tokens` (already parsed in `run_stream`) into the 5-tuple return.

- [ ] **Step 5: Run tests**

```bash
cargo test --lib codex_local
```

Expected: 6+ passed (3 new + existing).

- [ ] **Step 6: Commit**

```bash
git add src/codex_local.rs
git commit -m "feat(codex_local): wire LlmRequestOptions into CLI args

Implements chat_with_options_dyn for CodexLocalProvider:
- working_dir → -C <path> (or use existing with_working_dir)
- allow_web_search → --search
- image_paths → --image <path> per file
- reasoning_effort → -c model_reasoning_effort=\"<value>\"
- max_turns → no-op (Codex doesn't expose)
- allow_local_read → no-op (implicit in sandbox)

Token usage already parsed in run_stream — now propagates to
LlmResponse.input_tokens/output_tokens fields."
```

---

### Task 8: Wire options into `gemini_local`

**Files:**
- Modify: `src/gemini_local.rs`

Mirror prior tasks. Gemini specifics:

| Option | Gemini arg / behavior |
|---|---|
| `image_paths` | Append `\n@<abs_path>` to prompt body (triggers read_many_files) |
| `working_dir` | `current_dir(...)` + `--include-directories <path>` |
| `allow_web_search` | (no-op, no flag) |
| `allow_local_read` | (no-op, implicit via @path) |
| `max_turns` | (no-op) |
| `reasoning_effort` | (no-op) |

- [ ] **Step 1: Read `src/gemini_local.rs` to understand current structure**

```bash
cat src/gemini_local.rs | head -100
```

- [ ] **Step 2: Add unit tests + impl + override** following Task 6/7 pattern

Specifics:
- `build_prompt_with_options` instead of `build_args_with_options` (Gemini puts attachments in prompt body, not argv)
- Test: prompt contains `@/tmp/x.png` line when `image_paths = vec!["/tmp/x.png"]`
- Test: args contain `--include-directories /tmp/work` when `working_dir = Some("/tmp/work")`

- [ ] **Step 3: Run tests**

```bash
cargo test --lib gemini_local
```

- [ ] **Step 4: Commit**

```bash
git add src/gemini_local.rs
git commit -m "feat(gemini_local): wire LlmRequestOptions into prompt + CLI args

Implements chat_with_options_dyn for GeminiLocalProvider:
- working_dir → current_dir + --include-directories <path>
- image_paths → @<abs_path> appended to prompt (triggers read_many_files)
- allow_web_search, allow_local_read, max_turns, reasoning_effort: no-op
  (Gemini CLI does not expose these)

Note: Gemini CLI does not report token usage, so input_tokens and
output_tokens remain None even with options."
```

---

## Phase 4: Release v0.2.0

### Task 9: Update CHANGELOG, bump version, tag, push

- [ ] **Step 1: Bump version in `Cargo.toml`**

```toml
[package]
name = "llm-router"
version = "0.2.0"
```

- [ ] **Step 2: Update `CHANGELOG.md`**

```markdown
## [0.2.0] - 2026-04-XX

### Added
- `LlmRequestOptions` struct for typed per-request options (image attachments,
  working directory, web search, local read, max turns, reasoning effort).
- `LlmResponse.input_tokens` and `LlmResponse.output_tokens` (Option<u32>) for
  per-request token usage breakdown.
- `LlmProviderDyn::chat_with_options_dyn` and `chat_stream_with_options_dyn`
  trait methods (default impls forward to existing `chat_dyn`/`chat_stream_dyn`).
- `cli_utils` module with `resolve_binary`, `build_path_env`, `sanitize_prompt`.
- `openai_compat` provider for OpenAI-compatible HTTP endpoints.

### Changed
- `ProviderFactory` signature: `(api_key, model)` → `(api_key, model, base_url)`.
  Affects custom providers that implemented the factory trait.
- `claude_local`, `codex_local`, `gemini_local` now honor `LlmRequestOptions`
  when called via `chat_with_options_dyn`. Existing `chat_dyn` calls unchanged.

### Migration
- Custom `ProviderFactory` implementations: add unused `_base_url: &str` param.
- Code constructing `LlmResponse` with struct literals: add
  `input_tokens: None, output_tokens: None` (or use `..Default::default()`
  once Default is impl'd).
- No changes needed for callers using `create_provider(...)` + `chat_dyn(...)`.

[0.2.0]: https://github.com/kangnam7654/llm-router/compare/v0.1.0...v0.2.0
```

- [ ] **Step 3: Run final test suite**

```bash
cargo build --release
cargo test
cargo clippy -- -D warnings
```

Expected: all green.

- [ ] **Step 4: Commit version bump + changelog**

```bash
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore(release): bump to v0.2.0

See CHANGELOG.md for the full feature list. Headline changes:
- LlmRequestOptions for typed per-request options
- LlmResponse token usage breakdown
- cli_utils module
- openai_compat provider
- ProviderFactory signature change (breaking)"
```

- [ ] **Step 5: Tag and push**

```bash
git tag -a v0.2.0 -m "v0.2.0 — request options + cli utils + openai_compat"
git push origin main
git push origin v0.2.0
```

- [ ] **Step 6: Verify GitHub release tag**

```bash
gh release create v0.2.0 --title "v0.2.0" --notes-from-tag
# OR if you want manual notes:
# gh release create v0.2.0 --title "v0.2.0" --notes "$(awk '/^## \[0.2.0\]/,/^## \[0.1.0\]/' CHANGELOG.md | head -n -1)"
```

---

## Phase 5 (Future, separate plan): Migrate dear-jeongbin to llm-router v0.2.0

This is a separate effort with its own design doc. Sketch:

1. Add `llm-router = { git = "ssh://git@github.com/kangnam7654/llm-router.git", tag = "v0.2.0" }` to `dear-jeongbin/src-tauri/Cargo.toml`.
2. Replace `services::ai::generate_*` internals with `llm_router::create_provider` + `chat_with_options_dyn`.
3. Keep dear-jeongbin's `resolve_provider` (task tier → provider/model alias) — that's app policy.
4. Keep dear-jeongbin's SSE Event adapter — wrap `LlmStreamEvent::Delta { text }` → `Event::default().event("chunk").data(...)`.
5. Delete `sanitize_prompt`, `resolve_cli_binary`, `build_cli_path_env`, `common_cli_paths`, `push_unique_path` from dear-jeongbin (now in llm-router).
6. Verify all dear-jeongbin tests still pass.
7. Bump dear-jeongbin to 0.2.0.

Estimated reduction: `ai.rs` 1570 LOC → ~400 LOC.

---

## Self-Review Checklist (before starting)

- [ ] Pre-Phase: openai_compat work compiles + tests pass before starting Phase 1.
- [ ] All file paths in this plan exist or are explicitly marked "Create:".
- [ ] Every code block compiles standalone (or notes what surrounding context is needed).
- [ ] Every step has a concrete pass/fail criterion.
- [ ] Every commit message follows existing style (`feat:`, `chore:`, scope in parens).
- [ ] No "TBD" / "implement later" in steps that aren't explicitly marked future.
- [ ] Consistency check: trait method name `chat_with_options_dyn` used everywhere (not `chat_dyn_with_options` etc).

## Known Gaps to Verify Mid-Implementation

1. **Does Claude CLI actually accept `--image`?** If not, claude_local image_paths becomes no-op + doc note.
2. **Does Codex CLI's `-c model_reasoning_effort` syntax match?** Verify with `codex exec --help` before Task 7.
3. **`build_args_with_options` lifetime juggling** in Task 7 may need adjustment — Rust borrow checker has opinions about the `dir_owned` pattern.
4. The `todo!()` in Task 6 Step 3 must be replaced with the actual `run_stream` body before commit. Don't ship `todo!()`.
