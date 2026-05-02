use futures::stream::BoxStream;
use serde_json::Value;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::process::Command;
use std::thread;

use super::{ChatMessage, LlmError, LlmProviderDyn, LlmResponse, LlmStreamEvent};

/// Internal result of `run_stream` — full text, model, cost, input tokens, output tokens.
type RunStreamResult = Result<(String, String, f64, Option<u32>, Option<u32>), LlmError>;

const DEFAULT_MODEL: &str = "auto";

/// OpenAI Codex pricing (approximate, USD per 1M tokens).
/// Used by all three cost-estimation sites in this module.
const CODEX_INPUT_USD_PER_1M: f64 = 2.50;
const CODEX_OUTPUT_USD_PER_1M: f64 = 10.00;
const PER_1M: f64 = 1_000_000.0;

fn estimate_codex_cost(input_tokens: u64, output_tokens: u64) -> f64 {
    (input_tokens as f64) * CODEX_INPUT_USD_PER_1M / PER_1M
        + (output_tokens as f64) * CODEX_OUTPUT_USD_PER_1M / PER_1M
}

#[derive(Clone)]
pub struct CodexLocalProvider {
    model: String,
    cd: Option<String>,
}

impl CodexLocalProvider {
    pub fn new(model: String) -> Self {
        Self { model, cd: None }
    }

    pub fn with_working_dir(mut self, dir: String) -> Self {
        self.cd = Some(dir);
        self
    }
}

impl LlmProviderDyn for CodexLocalProvider {
    fn render_dyn(
        &self,
        system_prompt: &str,
        user_input: &str,
        _result_json: &Value,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + '_>,
    > {
        let provider = self.clone();
        let system = system_prompt.to_string();
        let input = user_input.to_string();
        Box::pin(async move {
            let msgs = vec![ChatMessage::user(input)];
            Self::chat_impl(&provider, &system, &msgs).await
        })
    }

    fn chat_dyn(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
        _result_json: &Value,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + '_>,
    > {
        let provider = self.clone();
        let system = system_prompt.to_string();
        let msgs = messages.to_vec();
        Box::pin(async move { Self::chat_impl(&provider, &system, &msgs).await })
    }

    fn chat_stream_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        _result_json: &'a Value,
    ) -> BoxStream<'a, LlmStreamEvent> {
        Self::run_stream_async(
            self.clone(),
            system_prompt.to_string(),
            messages.to_vec(),
            crate::LlmRequestOptions::default(),
        )
    }

    fn chat_with_options_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        options: &'a crate::LlmRequestOptions,
        result_json: &'a Value,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + 'a>,
    > {
        if options.timeout.is_some() {
            // Path A: timeout is set — route through the streaming path so that
            // on timeout the stream drop reaps the subprocess via kill_on_drop.
            // spawn_blocking + std::process::Command cannot be cancelled: dropping
            // the JoinHandle does NOT kill the blocking thread, and std::process::Child
            // has no kill_on_drop. Under sustained timeout load this exhausts the
            // default 512-thread blocking pool. The streaming path (run_stream_async)
            // uses tokio::process::Command with kill_on_drop(true), so cancellation
            // correctly reaps the subprocess.
            let system = system_prompt.to_string();
            let msgs = messages.to_vec();
            let opts = options.clone();
            let json = result_json.clone();
            Box::pin(async move {
                use futures::StreamExt;
                let mut stream = self.chat_stream_with_options_dyn(&system, &msgs, &opts, &json);
                let mut final_response: Option<LlmResponse> = None;
                while let Some(event) = stream.next().await {
                    match event {
                        LlmStreamEvent::Delta { .. } => {}
                        LlmStreamEvent::End { total } => {
                            final_response = Some(total);
                            break;
                        }
                        LlmStreamEvent::Error { message } => {
                            return Err(if message == "timeout" {
                                LlmError::Network {
                                    provider: "codex_local".into(),
                                    msg: "timeout".into(),
                                }
                            } else {
                                LlmError::Other {
                                    provider: "codex_local".into(),
                                    message,
                                }
                            });
                        }
                        _ => {}
                    }
                }
                final_response.ok_or_else(|| LlmError::Other {
                    provider: "codex_local".into(),
                    message: "stream ended without End event".into(),
                })
            })
        } else {
            // Path B: no timeout — preserve v0.3.0 blocking behavior verbatim.
            let provider = self.clone();
            let system = system_prompt.to_string();
            let msgs = messages.to_vec();
            let opts = options.clone();
            Box::pin(async move {
                let result = tokio::task::spawn_blocking(move || {
                    CodexLocalProvider::run_stream_with_options(&provider, &system, &msgs, &opts)
                })
                .await
                .unwrap_or_else(|e| {
                    Err(LlmError::Other {
                        provider: "codex_local".into(),
                        message: format!("spawn_blocking failed: {e}"),
                    })
                })?;

                Ok(LlmResponse {
                    rendered_text: result.0,
                    model: result.1,
                    estimated_cost_usd: result.2,
                    input_tokens: result.3,
                    output_tokens: result.4,
                    ..Default::default()
                })
            })
        }
    }

    fn chat_stream_with_options_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        options: &'a crate::LlmRequestOptions,
        _result_json: &'a Value,
    ) -> BoxStream<'a, LlmStreamEvent> {
        Self::run_stream_async(
            self.clone(),
            system_prompt.to_string(),
            messages.to_vec(),
            options.clone(),
        )
    }
}

impl CodexLocalProvider {
    /// Async streaming helper: spawns the Codex CLI via `tokio::process::Command`,
    /// reads stdout line by line with `AsyncBufReadExt::lines()`, parses each line
    /// as stream-json, and yields `LlmStreamEvent::Delta` for each agent_message
    /// text chunk. Emits a final `LlmStreamEvent::End` with the full accumulated
    /// text, model (captured from `thread.started`), cost, and token usage.
    ///
    /// Both `chat_stream_dyn` and `chat_stream_with_options_dyn` delegate here.
    /// Accepts fully owned inputs so the returned `BoxStream` is `'static`.
    fn run_stream_async(
        provider: CodexLocalProvider,
        system: String,
        msgs: Vec<ChatMessage>,
        opts: crate::LlmRequestOptions,
    ) -> BoxStream<'static, LlmStreamEvent> {
        Box::pin(async_stream::stream! {
            use tokio::io::AsyncBufReadExt;
            use tokio::io::AsyncReadExt;
            use tokio::process::Command as TokioCommand;

            let (args, _temp_images) = Self::build_args_with_options(
                &provider.model,
                provider.cd.as_deref(),
                &msgs,
                &system,
                &opts,
            );
            // _temp_images held alive until stream generator drops (stream end).
            let binary = crate::cli_utils::resolve_binary("codex");

            let mut command = TokioCommand::new(&binary);
            command
                .args(&args)
                .env("PATH", crate::cli_utils::build_path_env())
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true);

            // NOTE: Codex uses `-C <path>` inside build_args_with_options to set
            // working directory, so we do NOT call command.current_dir() here
            // (unlike gemini_local which uses current_dir instead of a CLI flag).
            // NOTE (asymmetry with gemini_local): no post-loop has_error check
            // because `turn.failed`/`error` events inline `yield Error + return`
            // from the stdout loop, making any post-loop error accumulator
            // unreachable.

            let mut child = match command.spawn() {
                Ok(c) => c,
                Err(e) => {
                    yield LlmStreamEvent::Error {
                        message: format!("failed to spawn codex CLI: {e}"),
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

            // Drain stderr concurrently to avoid pipe-fill deadlock.
            let stderr_task = child.stderr.take().map(|mut s| {
                tokio::spawn(async move {
                    let mut buf = String::new();
                    let _ = s.read_to_string(&mut buf).await;
                    buf
                })
            });

            let reader = tokio::io::BufReader::new(stdout);
            let mut lines = reader.lines();

            let mut accumulated = String::new();
            // NOTE: The existing blocking helpers (run_stream / run_stream_with_options)
            // capture `thread_id` from the `thread.started` event and store it as the
            // model name (lines 253-256 and 441-444). That is a pre-existing bug —
            // the `thread.started` event carries a `model` field, not a thread
            // identifier. This async path correctly uses `event.get("model")`.
            let mut model = String::new();
            let mut input_tokens: Option<u32> = None;
            let mut output_tokens: Option<u32> = None;

            // When a timeout is set, create a pinned sleep future. On each
            // iteration we race the next line against the sleep; if sleep wins
            // first, emit an Error event and return (dropping `child` here
            // triggers kill_on_drop to reap the subprocess).
            let timeout_dur = opts.timeout;
            let sleep_opt = timeout_dur.map(tokio::time::sleep);
            tokio::pin!(sleep_opt);

            loop {
                // Build the next-line future once per iteration.
                let line_result = if let Some(ref mut sleep) = sleep_opt.as_mut().as_pin_mut() {
                    tokio::select! {
                        biased;
                        res = lines.next_line() => Some(res),
                        _ = sleep => None,
                    }
                } else {
                    Some(lines.next_line().await)
                };

                match line_result {
                    // Timeout fired — emit error and exit (kill_on_drop reaps child).
                    None => {
                        yield LlmStreamEvent::Error { message: "timeout".into() };
                        return;
                    }
                    Some(Ok(Some(line))) => {
                        if line.is_empty() {
                            continue;
                        }
                        let event: serde_json::Value = match serde_json::from_str(&line) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        let event_type = event.get("type").and_then(serde_json::Value::as_str).unwrap_or("");
                        match event_type {
                            "thread.started" => {
                                if let Some(m) = event.get("model").and_then(serde_json::Value::as_str) {
                                    model = m.to_string();
                                }
                            }
                            "item.completed" => {
                                if let Some(item) = event.get("item") {
                                    if item.get("type").and_then(serde_json::Value::as_str) == Some("agent_message") {
                                        if let Some(text) = item.get("text").and_then(serde_json::Value::as_str) {
                                            if !text.is_empty() {
                                                accumulated.push_str(text);
                                                yield LlmStreamEvent::Delta {
                                                    text: text.to_string(),
                                                };
                                            }
                                        }
                                    }
                                }
                            }
                            "turn.completed" => {
                                if let Some(usage) = event.get("usage") {
                                    input_tokens = usage
                                        .get("input_tokens")
                                        .and_then(serde_json::Value::as_u64)
                                        .and_then(|n| u32::try_from(n).ok());
                                    output_tokens = usage
                                        .get("output_tokens")
                                        .and_then(serde_json::Value::as_u64)
                                        .and_then(|n| u32::try_from(n).ok());
                                }
                            }
                            "turn.failed" | "error" => {
                                let message = event
                                    .get("message")
                                    .and_then(serde_json::Value::as_str)
                                    .map(|s| s.to_string())
                                    .or_else(|| {
                                        event
                                            .get("error")
                                            .and_then(|e| e.get("message"))
                                            .and_then(serde_json::Value::as_str)
                                            .map(|s| s.to_string())
                                    })
                                    .unwrap_or_else(|| "codex error".to_string());
                                yield LlmStreamEvent::Error { message };
                                return;
                            }
                            _ => {}
                        }
                    }
                    Some(Ok(None)) => break,
                    Some(Err(e)) => {
                        yield LlmStreamEvent::Error {
                            message: format!("failed to read codex stdout: {e}"),
                        };
                        return;
                    }
                }
            }

            let exit_status = match child.wait().await {
                Ok(s) => s,
                Err(e) => {
                    yield LlmStreamEvent::Error {
                        message: format!("failed to wait for codex: {e}"),
                    };
                    return;
                }
            };

            let stderr_text = if let Some(task) = stderr_task {
                task.await.unwrap_or_default()
            } else {
                String::new()
            };

            let cost = estimate_codex_cost(
                input_tokens.unwrap_or(0) as u64,
                output_tokens.unwrap_or(0) as u64,
            );

            if !exit_status.success() {
                // TODO: surface as LlmStreamEvent::Auth once that variant exists.
                // The blocking path (run_stream / run_stream_with_options) returns
                // LlmError::Auth; the stream path collapses to a plain Error with a
                // conventional "authentication required" message because
                // LlmStreamEvent has no Auth variant — same gap as gemini_local.
                let src = if !stderr_text.is_empty() {
                    stderr_text
                } else {
                    format!("codex CLI exited with status: {exit_status}")
                };
                let hay = src.to_lowercase();
                let msg = if hay.contains("not authenticated")
                    || hay.contains("please login")
                    || hay.contains("authentication required")
                    || hay.contains("unauthorized")
                    || hay.contains("invalid credentials")
                    || hay.contains("not logged in")
                    || hay.contains("login required")
                    || hay.contains("run `codex login`")
                    || hay.contains("codex login")
                {
                    "authentication required".to_string()
                } else {
                    src
                };
                yield LlmStreamEvent::Error { message: msg };
                return;
            }

            if model.is_empty() {
                model = "codex-auto".to_string();
            }

            yield LlmStreamEvent::End {
                total: LlmResponse {
                    rendered_text: accumulated,
                    model,
                    estimated_cost_usd: cost,
                    input_tokens,
                    output_tokens,
                    ..Default::default()
                },
            };
        })
    }

    async fn chat_impl(
        provider: &CodexLocalProvider,
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> Result<LlmResponse, LlmError> {
        let provider = provider.clone();
        let system = system_prompt.to_string();
        let msgs = messages.to_vec();
        let result = tokio::task::spawn_blocking(move || {
            CodexLocalProvider::run_stream(&provider, &system, &msgs)
        })
        .await
        .unwrap_or_else(|e| {
            Err(LlmError::Other {
                provider: "codex_local".into(),
                message: format!("spawn_blocking failed: {e}"),
            })
        })?;

        Ok(LlmResponse {
            rendered_text: result.0,
            model: result.1,
            estimated_cost_usd: result.2,
            input_tokens: result.3,
            output_tokens: result.4,
            ..Default::default()
        })
    }

    fn run_stream(
        provider: &CodexLocalProvider,
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> RunStreamResult {
        let args = Self::build_args(
            &provider.model,
            provider.cd.as_deref(),
            messages,
            system_prompt,
        );
        let mut cmd = Command::new("codex")
            .args(&args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| LlmError::Other {
                provider: "codex_local".into(),
                message: format!("failed to spawn codex CLI: {e}"),
            })?;

        let stdout = cmd.stdout.take().ok_or_else(|| LlmError::Other {
            provider: "codex_local".into(),
            message: "failed to take stdout".into(),
        })?;

        let reader = BufReader::new(stdout);
        let mut full_text = String::new();
        let mut model = String::new();
        let mut cost = 0.0;
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;
        let mut has_error = false;
        let mut error_msg = String::new();

        for line in reader.lines() {
            let line = line.map_err(|e| LlmError::Other {
                provider: "codex_local".into(),
                message: format!("failed to read stdout line: {e}"),
            })?;

            if line.is_empty() {
                continue;
            }

            if let Ok(event) = serde_json::from_str::<Value>(&line) {
                if let Some(event_type) = event.get("type").and_then(|v| v.as_str()) {
                    match event_type {
                        "thread.started" => {
                            if let Some(thread_id) = event.get("thread_id").and_then(|v| v.as_str())
                            {
                                model = thread_id.to_string();
                            }
                        }
                        "item.completed" => {
                            if let Some(item) = event.get("item") {
                                if item.get("type").and_then(|t| t.as_str())
                                    == Some("agent_message")
                                {
                                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                        full_text.push_str(text);
                                    }
                                }
                            }
                        }
                        "turn.completed" => {
                            if let Some(usage) = event.get("usage") {
                                input_tokens = usage
                                    .get("input_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                output_tokens = usage
                                    .get("output_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                            }
                        }
                        "turn.failed" | "error" => {
                            has_error = true;
                            if let Some(msg) = event.get("message").and_then(|v| v.as_str()) {
                                error_msg = msg.to_string();
                            } else if let Some(err_obj) = event.get("error") {
                                if let Some(msg) = err_obj.get("message").and_then(|v| v.as_str()) {
                                    error_msg = msg.to_string();
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let exit_status = cmd.wait().map_err(|e| LlmError::Other {
            provider: "codex_local".into(),
            message: format!("failed to wait for codex CLI: {e}"),
        })?;

        cost += estimate_codex_cost(input_tokens, output_tokens);

        if !exit_status.success() {
            let stderr = cmd
                .stderr
                .take()
                .map(|mut s| {
                    let mut buf = String::new();
                    s.read_to_string(&mut buf).unwrap_or_default();
                    buf
                })
                .unwrap_or_default();

            let err_source = if !error_msg.is_empty() {
                error_msg
            } else if !stderr.is_empty() {
                stderr
            } else {
                format!("codex CLI exited with status: {exit_status}")
            };

            // Detect auth-required patterns
            let haystack = err_source.to_lowercase();
            if haystack.contains("not authenticated")
                || haystack.contains("please login")
                || haystack.contains("authentication required")
                || haystack.contains("unauthorized")
                || haystack.contains("invalid credentials")
                || haystack.contains("not logged in")
                || haystack.contains("login required")
                || haystack.contains("run `codex login`")
                || haystack.contains("codex login")
            {
                return Err(LlmError::Auth {
                    provider: "codex_local".into(),
                });
            }

            return Err(LlmError::Other {
                provider: "codex_local".into(),
                message: err_source,
            });
        }

        if has_error {
            return Err(LlmError::Other {
                provider: "codex_local".into(),
                message: if error_msg.is_empty() {
                    "unknown error".into()
                } else {
                    error_msg
                },
            });
        }

        if model.is_empty() {
            model = "codex-auto".to_string();
        }

        Ok((
            full_text,
            model,
            cost,
            u32::try_from(input_tokens).ok(),
            u32::try_from(output_tokens).ok(),
        ))
    }

    fn run_stream_with_options(
        provider: &CodexLocalProvider,
        system_prompt: &str,
        messages: &[ChatMessage],
        options: &crate::LlmRequestOptions,
    ) -> RunStreamResult {
        let (args, _temp_images) = Self::build_args_with_options(
            &provider.model,
            provider.cd.as_deref(),
            messages,
            system_prompt,
            options,
        );
        // _temp_images held alive until subprocess exits (end of this function).
        let binary = crate::cli_utils::resolve_binary("codex");
        let mut command = Command::new(&binary);
        command
            .args(&args)
            .env("PATH", crate::cli_utils::build_path_env())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Belt-and-suspenders with `-C <path>`: also set subprocess cwd so
        // relative paths (e.g. for images) resolve the same way the CLI does.
        if let Some(dir) = options.working_dir.as_ref() {
            command.current_dir(dir);
        }

        let mut cmd = command.spawn().map_err(|e| LlmError::Other {
            provider: "codex_local".into(),
            message: format!("failed to spawn codex CLI: {e}"),
        })?;

        let stdout = cmd.stdout.take().ok_or_else(|| LlmError::Other {
            provider: "codex_local".into(),
            message: "failed to take stdout".into(),
        })?;

        // Drain stderr concurrently with stdout to avoid a pipe-fill deadlock.
        let stderr_handle = cmd.stderr.take();
        let stderr_thread = stderr_handle.map(|mut s| {
            thread::spawn(move || {
                let mut buf = String::new();
                let _ = s.read_to_string(&mut buf);
                buf
            })
        });

        let reader = BufReader::new(stdout);
        let mut full_text = String::new();
        let mut model = String::new();
        let mut cost = 0.0;
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;
        let mut has_error = false;
        let mut error_msg = String::new();

        for line in reader.lines() {
            let line = line.map_err(|e| LlmError::Other {
                provider: "codex_local".into(),
                message: format!("failed to read stdout line: {e}"),
            })?;

            if line.is_empty() {
                continue;
            }

            if let Ok(event) = serde_json::from_str::<Value>(&line) {
                if let Some(event_type) = event.get("type").and_then(|v| v.as_str()) {
                    match event_type {
                        "thread.started" => {
                            if let Some(thread_id) = event.get("thread_id").and_then(|v| v.as_str())
                            {
                                model = thread_id.to_string();
                            }
                        }
                        "item.completed" => {
                            if let Some(item) = event.get("item") {
                                if item.get("type").and_then(|t| t.as_str())
                                    == Some("agent_message")
                                {
                                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                        full_text.push_str(text);
                                    }
                                }
                            }
                        }
                        "turn.completed" => {
                            if let Some(usage) = event.get("usage") {
                                input_tokens = usage
                                    .get("input_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                output_tokens = usage
                                    .get("output_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                            }
                        }
                        "turn.failed" | "error" => {
                            has_error = true;
                            if let Some(msg) = event.get("message").and_then(|v| v.as_str()) {
                                error_msg = msg.to_string();
                            } else if let Some(err_obj) = event.get("error") {
                                if let Some(msg) = err_obj.get("message").and_then(|v| v.as_str()) {
                                    error_msg = msg.to_string();
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let exit_status = cmd.wait().map_err(|e| LlmError::Other {
            provider: "codex_local".into(),
            message: format!("failed to wait for codex CLI: {e}"),
        })?;

        let stderr = stderr_thread
            .map(|h| h.join().unwrap_or_default())
            .unwrap_or_default();

        cost += estimate_codex_cost(input_tokens, output_tokens);

        if !exit_status.success() {
            let err_source = if !error_msg.is_empty() {
                error_msg
            } else if !stderr.is_empty() {
                stderr
            } else {
                format!("codex CLI exited with status: {exit_status}")
            };

            // Detect auth-required patterns
            let haystack = err_source.to_lowercase();
            if haystack.contains("not authenticated")
                || haystack.contains("please login")
                || haystack.contains("authentication required")
                || haystack.contains("unauthorized")
                || haystack.contains("invalid credentials")
                || haystack.contains("not logged in")
                || haystack.contains("login required")
                || haystack.contains("run `codex login`")
                || haystack.contains("codex login")
            {
                return Err(LlmError::Auth {
                    provider: "codex_local".into(),
                });
            }

            return Err(LlmError::Other {
                provider: "codex_local".into(),
                message: err_source,
            });
        }

        if has_error {
            return Err(LlmError::Other {
                provider: "codex_local".into(),
                message: if error_msg.is_empty() {
                    "unknown error".into()
                } else {
                    error_msg
                },
            });
        }

        if model.is_empty() {
            model = "codex-auto".to_string();
        }

        Ok((
            full_text,
            model,
            cost,
            u32::try_from(input_tokens).ok(),
            u32::try_from(output_tokens).ok(),
        ))
    }

    /// Build the base CLI args from messages. Returns the arg list and any
    /// temporary image files that must stay alive until the subprocess exits.
    ///
    /// Multipart [`ChatContent`] handling:
    /// - `Text`: appended to the prompt (backward-compatible).
    /// - `Image { Base64 }`: decoded → temp file → path collected for `--image`.
    /// - `Image { Url }`: warn + skip.
    /// - `ToolResult`: debug-log + skip.
    fn build_args_inner(
        model: &str,
        cd: Option<&str>,
        messages: &[ChatMessage],
        system_prompt: &str,
    ) -> (Vec<String>, Vec<crate::cli_utils::TempFile>) {
        let mut fixed_args = vec![
            "exec".to_string(),
            "--json".to_string(),
            "--dangerously-bypass-approvals-and-sandbox".to_string(),
            "--sandbox".to_string(),
            "danger-full-access".to_string(),
            "--ephemeral".to_string(),
        ];

        if !model.is_empty() && model != DEFAULT_MODEL {
            fixed_args.push("--model".to_string());
            fixed_args.push(model.to_string());
        }

        if let Some(dir) = cd {
            fixed_args.push("-C".to_string());
            fixed_args.push(dir.to_string());
        }

        let mut prompt = String::new();
        if !system_prompt.is_empty() {
            prompt.push_str(system_prompt);
            prompt.push_str("\n\n");
        }

        let mut temp_images: Vec<crate::cli_utils::TempFile> = Vec::new();

        for m in messages {
            let has_text = m
                .content
                .iter()
                .any(|b| matches!(b, crate::ChatContent::Text(_)));
            if has_text {
                prompt.push_str(&format!("[{}]:\n", m.role));
            }
            for block in &m.content {
                match block {
                    crate::ChatContent::Text(t) => {
                        prompt.push_str(t);
                    }
                    crate::ChatContent::Image { source, mime_type } => match source {
                        crate::ImageSource::Base64(data) => {
                            match crate::cli_utils::decode_base64_image(data, mime_type) {
                                Ok(tmp) => temp_images.push(tmp),
                                Err(e) => {
                                    tracing::warn!(
                                        "codex_local: failed to decode base64 image; skipping: {}",
                                        e
                                    );
                                }
                            }
                        }
                        crate::ImageSource::Url(url) => {
                            tracing::warn!(
                                "codex_local: URL image source unsupported (would need pre-download); skipping: {}",
                                url
                            );
                        }
                    },
                    crate::ChatContent::ToolResult { tool_use_id, .. } => {
                        tracing::debug!(
                            "codex_local: dropping tool_result block (CLI handles its own internal tools); tool_use_id={}",
                            tool_use_id
                        );
                    }
                }
            }
            if has_text {
                prompt.push_str("\n\n");
            }
        }

        fixed_args.push(prompt);
        (fixed_args, temp_images)
    }

    /// Backward-compatible entry point used by `run_stream` (legacy blocking path).
    fn build_args(
        model: &str,
        cd: Option<&str>,
        messages: &[ChatMessage],
        system_prompt: &str,
    ) -> Vec<String> {
        let (args, _temp_images) = Self::build_args_inner(model, cd, messages, system_prompt);
        args
    }

    fn build_args_with_options(
        model: &str,
        cd: Option<&str>,
        messages: &[ChatMessage],
        system_prompt: &str,
        options: &crate::LlmRequestOptions,
    ) -> (Vec<String>, Vec<crate::cli_utils::TempFile>) {
        // Resolve working dir: explicit cd param takes priority, else option.
        let dir_owned = options
            .working_dir
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned());
        let effective_cd: Option<&str> = cd.or(dir_owned.as_deref());

        let (base, temp_images) =
            Self::build_args_inner(model, effective_cd, messages, system_prompt);
        let (prompt, head) = base
            .split_last()
            .expect("build_args_inner must return at least one element (prompt)");
        let mut args: Vec<String> = head.to_vec();
        let prompt = prompt.clone();

        if options.allow_web_search {
            args.push("--search".to_string());
        }
        // Image paths from decoded message blocks first, then caller-supplied.
        for tmp in &temp_images {
            args.push("--image".to_string());
            args.push(tmp.0.to_string_lossy().into_owned());
        }
        for img in &options.image_paths {
            args.push("--image".to_string());
            args.push(img.to_string_lossy().into_owned());
        }
        if let Some(effort) = options.reasoning_effort.as_ref() {
            args.push("-c".to_string());
            args.push(format!("model_reasoning_effort=\"{effort}\""));
        }

        args.push(prompt);
        (args, temp_images)
    }
}

pub fn make(
    _api_key: &str,
    model: &str,
    _base_url: &str,
) -> Result<Box<dyn LlmProviderDyn>, LlmError> {
    // codex_local does not require an API key — it uses the local codex CLI
    // which authenticates via the user's ChatGPT account.
    let model = if model.is_empty() {
        DEFAULT_MODEL.to_string()
    } else {
        model.to_string()
    };
    Ok(Box::new(CodexLocalProvider::new(model)))
}

/// List available models from `~/.codex/models_cache.json` (populated by the
/// Codex CLI on login/startup), falling back to the configured model in
/// `~/.codex/config.toml` if the cache is absent.
pub async fn list_models() -> Result<Vec<crate::ListModel>, LlmError> {
    let home = std::env::var("HOME").ok().map(std::path::PathBuf::from);

    // Primary: models_cache.json written by the Codex CLI
    if let Some(cache_path) = home
        .as_ref()
        .map(|h| h.join(".codex").join("models_cache.json"))
    {
        if let Ok(contents) = tokio::fs::read_to_string(&cache_path).await {
            let models = parse_cache(&contents);
            if !models.is_empty() {
                return Ok(models);
            }
        }
    }

    // Fallback: single model from config.toml
    let config_path = home.map(|h| h.join(".codex").join("config.toml"));
    let contents = match config_path {
        Some(p) => tokio::fs::read_to_string(&p).await.ok(),
        None => None,
    };

    let contents = match contents {
        Some(c) if !c.is_empty() => c,
        _ => {
            if let Some(dir) = dirs::config_dir() {
                let xdg_path = dir.join("codex").join("config.toml");
                if let Ok(c) = tokio::fs::read_to_string(&xdg_path).await {
                    if !c.is_empty() {
                        return Ok(parse_config_model(&c));
                    }
                }
            }
            return Ok(Vec::new());
        }
    };

    Ok(parse_config_model(&contents))
}

fn parse_cache(contents: &str) -> Vec<crate::ListModel> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(contents) else {
        return Vec::new();
    };
    let Some(models) = json.get("models").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    models
        .iter()
        .filter(|m| m.get("visibility").and_then(|v| v.as_str()) != Some("hide"))
        .filter_map(|m| {
            let name = m.get("slug")?.as_str()?.to_string();
            let display_name = m
                .get("display_name")
                .and_then(|v| v.as_str())
                .unwrap_or(&name)
                .to_string();
            let description = m
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Some(crate::ListModel {
                name,
                display_name,
                description,
                input_token_limit: None,
                output_token_limit: None,
            })
        })
        .collect()
}

fn parse_config_model(contents: &str) -> Vec<crate::ListModel> {
    let model = contents.lines().find_map(|l| {
        let trimmed = l.trim();
        if trimmed.starts_with("model = ") {
            trimmed
                .strip_prefix("model = ")
                .and_then(|v| v.strip_prefix('"'))
                .and_then(|v| v.strip_suffix('"'))
                .map(|s| s.to_string())
        } else {
            None
        }
    });

    model
        .map(|name| {
            vec![crate::ListModel {
                name,
                display_name: String::new(),
                description: None,
                input_token_limit: None,
                output_token_limit: None,
            }]
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialize tests that mutate CODEX_CLI_PATH so their env-var sets/unsets
    /// don't race when cargo-test runs them in parallel threads. Uses
    /// `tokio::sync::Mutex` so it's safe to hold the guard across `.await`
    /// while the stream is being consumed (`std::sync::Mutex` would trip
    /// clippy::await_holding_lock).
    static CODEX_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[test]
    fn make_with_empty_model_returns_default() {
        let result = make("", "", "");
        assert!(result.is_ok(), "make should succeed with empty model");
    }

    #[test]
    fn make_with_model_returns_provider() {
        let result = make("", "o4-mini", "");
        assert!(result.is_ok(), "make should succeed with model");
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "requires codex CLI installed locally with a current model"]
    async fn full_chat_via_cli_returns_text() {
        let provider = CodexLocalProvider::new("auto".to_string());
        let msgs = vec![ChatMessage::user(
            "Say 'hello' in exactly one word. Nothing else.",
        )];
        let resp = provider
            .chat_dyn(
                "You are a test assistant. Respond briefly.",
                &msgs,
                &serde_json::json!({}),
            )
            .await
            .expect("codex CLI should respond");

        println!(
            "model={}, cost={:.6}, text=[{}]",
            resp.model,
            resp.estimated_cost_usd,
            &resp.rendered_text[..resp.rendered_text.len().min(200)]
        );

        let text_lower = resp.rendered_text.to_lowercase();
        assert!(
            text_lower.contains("hello") || text_lower.contains("hi"),
            "content should contain 'hello' or 'hi': [{}]",
            resp.rendered_text
        );
    }

    #[tokio::test]
    #[ignore = "requires codex CLI installed locally (or models_cache.json present)"]
    async fn list_models_returns_multiple_from_cache() {
        let models = list_models().await.expect("list_models should succeed");
        assert!(
            !models.is_empty(),
            "list_models should return at least one model"
        );
        // When models_cache.json is present, expect more than one model
        for m in &models {
            assert!(
                !m.name.is_empty(),
                "each model should have a non-empty slug"
            );
        }
        println!(
            "codex list_models returned {} model(s): {:?}",
            models.len(),
            models.iter().map(|m| &m.name).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn parse_cache_filters_hidden_models() {
        let json = r#"{
            "models": [
                {"slug": "gpt-5.4", "display_name": "GPT-5.4", "description": "Fast model", "visibility": "list"},
                {"slug": "internal-model", "display_name": "Internal", "description": "Hidden", "visibility": "hide"}
            ]
        }"#;
        let models = parse_cache(json);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].name, "gpt-5.4");
        assert_eq!(models[0].description.as_deref(), Some("Fast model"));
    }

    #[test]
    fn build_args_includes_search_flag_when_enabled() {
        let opts = crate::LlmRequestOptions {
            allow_web_search: true,
            ..Default::default()
        };
        let messages = vec![crate::ChatMessage::user("hi")];
        let (args, _) =
            CodexLocalProvider::build_args_with_options("auto", None, &messages, "sys", &opts);
        assert!(args.iter().any(|a| a == "--search"), "args: {args:?}");
        assert!(
            args.last().unwrap().contains("[user]"),
            "prompt must be last: {args:?}"
        );
    }

    #[test]
    fn build_args_includes_reasoning_effort_when_set() {
        let opts = crate::LlmRequestOptions {
            reasoning_effort: Some("high".into()),
            ..Default::default()
        };
        let messages = vec![crate::ChatMessage::user("hi")];
        let (args, _) =
            CodexLocalProvider::build_args_with_options("auto", None, &messages, "sys", &opts);
        let c_idx = args.iter().position(|a| a == "-c").expect("-c present");
        assert!(args[c_idx + 1].contains("model_reasoning_effort"));
        assert!(args[c_idx + 1].contains("high"));
        assert!(
            args.last().unwrap().contains("[user]"),
            "prompt must be last: {args:?}"
        );
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
        let messages = vec![crate::ChatMessage::user("hi")];
        let (args, _) =
            CodexLocalProvider::build_args_with_options("auto", None, &messages, "sys", &opts);
        let image_count = args.iter().filter(|a| *a == "--image").count();
        assert_eq!(image_count, 2);
        assert!(
            args.last().unwrap().contains("[user]"),
            "prompt must be last: {args:?}"
        );
    }

    #[tokio::test]
    async fn codex_stream_emits_deltas_and_captures_usage() {
        use crate::{LlmProviderDyn, LlmRequestOptions, LlmStreamEvent};
        use futures::StreamExt;
        use serde_json::Value;

        let _guard = CODEX_ENV_LOCK.lock().await;

        let fixture = std::env::current_dir()
            .unwrap()
            .join("tests/fixtures/fake_codex.sh");
        assert!(fixture.exists(), "fixture missing: {}", fixture.display());

        // UNSAFETY RATIONALE: `set_var` is not thread-safe; the CODEX_ENV_LOCK
        // mutex above serializes this test with codex_stream_emits_error_on_turn_failed,
        // the only other test that mutates CODEX_CLI_PATH.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("CODEX_CLI_PATH", fixture.to_str().unwrap());
        }

        let provider = super::CodexLocalProvider::new("gpt-5-codex".to_string());
        let opts = LlmRequestOptions::default();
        let messages = vec![crate::ChatMessage::user("hi")];
        let mut stream = provider.chat_stream_with_options_dyn("", &messages, &opts, &Value::Null);

        let mut deltas: Vec<String> = Vec::new();
        let mut end_resp = None;
        while let Some(event) = stream.next().await {
            match event {
                LlmStreamEvent::Delta { text } => deltas.push(text),
                LlmStreamEvent::End { total } => end_resp = Some(total),
                LlmStreamEvent::Error { message } => panic!("unexpected error: {message}"),
                _ => {}
            }
        }

        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("CODEX_CLI_PATH");
        }

        assert_eq!(deltas.len(), 2, "expected 2 deltas, got {deltas:?}");
        assert_eq!(deltas.concat(), "Hello world!");
        let end = end_resp.expect("End missing");
        assert_eq!(end.rendered_text, "Hello world!");
        assert_eq!(end.input_tokens, Some(10));
        assert_eq!(end.output_tokens, Some(20));
        assert_eq!(end.model, "gpt-5-codex");
    }

    #[test]
    fn build_args_uses_options_working_dir_when_cd_param_absent() {
        let opts = crate::LlmRequestOptions {
            working_dir: Some(std::path::PathBuf::from("/tmp/work")),
            ..Default::default()
        };
        let messages = vec![crate::ChatMessage::user("hi")];
        let (args, _) =
            CodexLocalProvider::build_args_with_options("auto", None, &messages, "sys", &opts);
        let c_idx = args.iter().position(|a| a == "-C").expect("-C present");
        assert_eq!(args[c_idx + 1], "/tmp/work");
    }

    #[tokio::test]
    async fn codex_stream_emits_error_on_turn_failed() {
        use crate::{LlmProviderDyn, LlmRequestOptions, LlmStreamEvent};
        use futures::StreamExt;
        use serde_json::Value;

        let _guard = CODEX_ENV_LOCK.lock().await;

        let fixture = std::env::current_dir()
            .unwrap()
            .join("tests/fixtures/fake_codex_error.sh");
        assert!(fixture.exists(), "fixture missing: {}", fixture.display());

        // UNSAFETY RATIONALE: `set_var` is not thread-safe; the CODEX_ENV_LOCK
        // mutex above serializes this test with codex_stream_emits_deltas_and_captures_usage,
        // the only other test that mutates CODEX_CLI_PATH.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("CODEX_CLI_PATH", fixture.to_str().unwrap());
        }

        let provider = super::CodexLocalProvider::new("gpt-5-codex".to_string());
        let opts = LlmRequestOptions::default();
        let messages = vec![crate::ChatMessage::user("hi")];
        let mut stream = provider.chat_stream_with_options_dyn("", &messages, &opts, &Value::Null);

        let mut got_error = false;
        let mut got_end = false;
        let mut error_message = String::new();
        while let Some(event) = stream.next().await {
            match event {
                LlmStreamEvent::Error { message } => {
                    got_error = true;
                    error_message = message;
                }
                LlmStreamEvent::End { .. } => got_end = true,
                LlmStreamEvent::Delta { .. } => {}
                _ => {}
            }
        }

        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("CODEX_CLI_PATH");
        }

        assert!(got_error, "expected Error event");
        assert!(!got_end, "End must not be emitted when Error fires");
        assert!(
            error_message.contains("rate limited"),
            "expected message to contain 'rate limited', got: {error_message}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn timeout_aborts_blocking_call() {
        use crate::{LlmError, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use std::time::Duration;

        let _guard = CODEX_ENV_LOCK.lock().await;

        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/fake_codex_slow.sh");
        assert!(fixture.exists(), "fixture missing: {}", fixture.display());

        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("CODEX_CLI_PATH", fixture.to_str().unwrap());
        }

        let provider = CodexLocalProvider::new("auto".to_string());
        let messages = vec![crate::ChatMessage::user("hi")];
        let opts = LlmRequestOptions {
            timeout: Some(Duration::from_millis(500)),
            ..Default::default()
        };

        let started = std::time::Instant::now();
        let res = provider
            .chat_with_options_dyn("sys", &messages, &opts, &Value::Null)
            .await;
        let elapsed = started.elapsed();

        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("CODEX_CLI_PATH");
        }

        assert!(
            matches!(
                res,
                Err(LlmError::Network { ref provider, ref msg })
                    if provider == "codex_local" && msg.contains("timeout")
            ),
            "expected Network/timeout error, got {res:?}"
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "should have aborted within 2s, took {elapsed:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn timeout_aborts_stream() {
        use crate::{LlmProviderDyn, LlmRequestOptions, LlmStreamEvent};
        use futures::StreamExt;
        use serde_json::Value;
        use std::time::Duration;

        let _guard = CODEX_ENV_LOCK.lock().await;

        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/fake_codex_slow.sh");
        assert!(fixture.exists(), "fixture missing: {}", fixture.display());

        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("CODEX_CLI_PATH", fixture.to_str().unwrap());
        }

        let provider = CodexLocalProvider::new("auto".to_string());
        let messages = vec![crate::ChatMessage::user("hi")];
        let opts = LlmRequestOptions {
            timeout: Some(Duration::from_millis(500)),
            ..Default::default()
        };

        let mut stream =
            provider.chat_stream_with_options_dyn("sys", &messages, &opts, &Value::Null);

        let started = std::time::Instant::now();
        let mut got_timeout_error = false;
        while let Some(ev) = stream.next().await {
            match ev {
                LlmStreamEvent::Error { ref message } if message.contains("timeout") => {
                    got_timeout_error = true;
                    break;
                }
                LlmStreamEvent::Delta { .. } => {}
                LlmStreamEvent::Error { message } => {
                    panic!("unexpected Error event before timeout: {message}")
                }
                LlmStreamEvent::End { .. } => {
                    panic!("unexpected End event before timeout fired")
                }
                _ => {}
            }
        }
        let elapsed = started.elapsed();

        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("CODEX_CLI_PATH");
        }

        assert!(
            got_timeout_error,
            "expected LlmStreamEvent::Error with 'timeout'"
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "should have aborted within 2s, took {elapsed:?}"
        );
    }

    /// A 1x1 transparent PNG in raw bytes — smallest valid PNG for testing.
    const TINY_PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8,
        0xCF, 0xC0, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0xE2, 0x21, 0xBC, 0x33, 0x00, 0x00, 0x00,
        0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn multipart_image_base64_block_adds_image_path_arg() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD.encode(TINY_PNG);
        let img_block = crate::ChatContent::Image {
            source: crate::ImageSource::Base64(b64),
            mime_type: "image/png".to_string(),
        };
        let messages = vec![crate::ChatMessage::user_with_image("describe", img_block)];
        let opts = crate::LlmRequestOptions::default();
        let (args, temp_images) =
            CodexLocalProvider::build_args_with_options("auto", None, &messages, "", &opts);

        assert!(
            args.iter().any(|a| a == "--image"),
            "expected --image flag in args: {args:?}"
        );
        let img_idx = args.iter().position(|a| a == "--image").unwrap();
        let img_path = std::path::PathBuf::from(&args[img_idx + 1]);
        assert!(
            img_path.exists(),
            "temp image file should exist: {}",
            img_path.display()
        );
        assert!(
            args.last().unwrap().contains("describe"),
            "prompt must be last: {args:?}"
        );
        drop(temp_images);
        assert!(
            !img_path.exists(),
            "temp image file should be removed after TempFile drop"
        );
    }

    #[test]
    fn multipart_tool_result_block_dropped_from_prompt() {
        let tool_result = crate::ChatMessage::tool_result("toolu_01", "25C in Seoul", false);
        let text_msg = crate::ChatMessage::user("What is the weather?");
        let messages = vec![text_msg, tool_result];
        let opts = crate::LlmRequestOptions::default();
        let (args, _) =
            CodexLocalProvider::build_args_with_options("auto", None, &messages, "", &opts);

        let prompt = args.last().expect("args not empty");
        assert!(
            !prompt.contains("25C in Seoul"),
            "tool_result content must be dropped from prompt, got: {prompt}"
        );
        assert!(
            prompt.contains("What is the weather?"),
            "text message must appear in prompt: {prompt}"
        );
    }
}
