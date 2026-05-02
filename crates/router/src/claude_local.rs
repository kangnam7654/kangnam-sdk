use futures::stream::BoxStream;
use serde_json::Value;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::process::Command;
use std::thread;

use super::{ChatMessage, LlmError, LlmProviderDyn, LlmResponse, LlmStreamEvent};

const DEFAULT_MODEL: &str = "auto";

type RunStreamOptResult = Result<(String, String, f64, Option<u32>, Option<u32>), LlmError>;

#[derive(Clone)]
pub struct ClaudeLocalProvider {
    model: String,
}

impl ClaudeLocalProvider {
    pub fn new(model: String) -> Self {
        Self { model }
    }
}

impl LlmProviderDyn for ClaudeLocalProvider {
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
                                    provider: "claude_local".into(),
                                    msg: "timeout".into(),
                                }
                            } else {
                                LlmError::Other {
                                    provider: "claude_local".into(),
                                    message,
                                }
                            });
                        }
                        _ => {}
                    }
                }
                final_response.ok_or_else(|| LlmError::Other {
                    provider: "claude_local".into(),
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
                    ClaudeLocalProvider::run_stream_with_options(&provider, &system, &msgs, &opts)
                })
                .await
                .unwrap_or_else(|e| {
                    Err(LlmError::Other {
                        provider: "claude_local".into(),
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

impl ClaudeLocalProvider {
    async fn chat_impl(
        provider: &ClaudeLocalProvider,
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> Result<LlmResponse, LlmError> {
        let provider = provider.clone();
        let system = system_prompt.to_string();
        let msgs = messages.to_vec();
        let result = tokio::task::spawn_blocking(move || {
            ClaudeLocalProvider::run_stream(&provider, &system, &msgs)
        })
        .await
        .unwrap_or_else(|e| {
            Err(LlmError::Other {
                provider: "claude_local".into(),
                message: format!("spawn_blocking failed: {e}"),
            })
        })?;

        Ok(LlmResponse {
            rendered_text: result.0,
            model: result.1,
            estimated_cost_usd: result.2,
            ..Default::default()
        })
    }

    /// Async streaming helper: spawns the Claude CLI via `tokio::process::Command`,
    /// reads stdout line by line with `AsyncBufReadExt::lines()`, parses each line
    /// as stream-json, and yields `LlmStreamEvent::Delta` for each `assistant` event
    /// text block. Emits a final `LlmStreamEvent::End` with the full accumulated
    /// text, model, cost, and token usage.
    ///
    /// Both `chat_stream_dyn` and `chat_stream_with_options_dyn` delegate here.
    /// Accepts fully owned inputs so the returned `BoxStream` is `'static`.
    fn run_stream_async(
        provider: ClaudeLocalProvider,
        system: String,
        msgs: Vec<ChatMessage>,
        opts: crate::LlmRequestOptions,
    ) -> BoxStream<'static, LlmStreamEvent> {
        Box::pin(async_stream::stream! {
            use tokio::io::AsyncBufReadExt;
            use tokio::io::AsyncReadExt;
            use tokio::process::Command as TokioCommand;

            let (args, _temp_images) =
                Self::build_args_with_options(&provider.model, &msgs, &system, &opts);
            // _temp_images is held here until the stream generator drops at
            // stream end — this keeps the temp files alive for the subprocess.
            let binary = crate::cli_utils::resolve_binary("claude");

            let mut command = TokioCommand::new(&binary);
            command
                .args(&args)
                .env("PATH", crate::cli_utils::build_path_env())
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true);

            if let Some(dir) = opts.working_dir.as_ref() {
                command.current_dir(dir);
            }

            let mut child = match command.spawn() {
                Ok(c) => c,
                Err(e) => {
                    yield LlmStreamEvent::Error {
                        message: format!("failed to spawn claude: {e}"),
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
            let mut thinking_accumulator = String::new();
            let mut model = String::new();
            let mut cost: f64 = 0.0;
            let mut input_tokens: Option<u32> = None;
            let mut output_tokens: Option<u32> = None;
            let mut has_error = false;
            let mut error_msg = String::new();

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
                        if line.trim().is_empty() {
                            continue;
                        }
                        let ev: serde_json::Value = match serde_json::from_str(&line) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        let et = ev.get("type").and_then(serde_json::Value::as_str).unwrap_or("");
                        match et {
                            "system" => {
                                if let Some(m) = ev.get("model").and_then(serde_json::Value::as_str) {
                                    model = m.to_string();
                                }
                            }
                            "assistant" => {
                                if let Some(msg) = ev.get("message") {
                                    if let Some(m) = msg.get("model").and_then(serde_json::Value::as_str) {
                                        model = m.to_string();
                                    }
                                    if let Some(arr) = msg.get("content").and_then(|c| c.as_array()) {
                                        for part in arr {
                                            match part.get("type").and_then(serde_json::Value::as_str) {
                                                Some("thinking") => {
                                                    if let Some(t) = part.get("thinking").and_then(serde_json::Value::as_str) {
                                                        if !t.is_empty() {
                                                            thinking_accumulator.push_str(t);
                                                            yield LlmStreamEvent::Thinking { text: t.to_string() };
                                                        }
                                                    }
                                                }
                                                Some("text") => {
                                                    if let Some(t) = part.get("text").and_then(serde_json::Value::as_str) {
                                                        if !t.is_empty() {
                                                            accumulated.push_str(t);
                                                            yield LlmStreamEvent::Delta { text: t.to_string() };
                                                        }
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                    if let Some(u) = msg.get("usage") {
                                        if let Some(n) = u.get("input_tokens").and_then(serde_json::Value::as_u64) {
                                            input_tokens = u32::try_from(n).ok();
                                        }
                                        if let Some(n) = u.get("output_tokens").and_then(serde_json::Value::as_u64) {
                                            output_tokens = u32::try_from(n).ok();
                                        }
                                    }
                                }
                            }
                            "result" => {
                                cost = ev.get("total_cost_usd").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
                                if let Some(is_err) = ev.get("is_error").and_then(serde_json::Value::as_bool) {
                                    if is_err {
                                        has_error = true;
                                        if let Some(r) = ev.get("result").and_then(serde_json::Value::as_str) {
                                            error_msg = r.to_string();
                                        }
                                    }
                                }
                                // NOTE: do NOT push result.result text into accumulated or emit Delta —
                                // assistant events already provided it. Emitting here would duplicate.
                                // This means End.rendered_text is the concatenation of assistant
                                // `content[].text`, NOT result.result. In practice these are the same
                                // string; if the CLI ever normalizes result.result differently, the
                                // authoritative source for streaming consumers is the assistant text
                                // (so Delta events and End.rendered_text remain consistent).
                                // The blocking helpers (run_stream / run_stream_with_options) use
                                // result.result instead — to be unified in Task 4 when the blocking
                                // helpers are retired.
                                // result.usage takes precedence over assistant.usage (emitted last).
                                if let Some(u) = ev.get("usage") {
                                    if let Some(n) = u.get("input_tokens").and_then(serde_json::Value::as_u64) {
                                        input_tokens = u32::try_from(n).ok();
                                    }
                                    if let Some(n) = u.get("output_tokens").and_then(serde_json::Value::as_u64) {
                                        output_tokens = u32::try_from(n).ok();
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Some(Ok(None)) => break,
                    Some(Err(e)) => {
                        yield LlmStreamEvent::Error {
                            message: format!("claude stdout: {e}"),
                        };
                        return;
                    }
                }
            }

            let status = match child.wait().await {
                Ok(s) => s,
                Err(e) => {
                    yield LlmStreamEvent::Error {
                        message: format!("claude wait: {e}"),
                    };
                    return;
                }
            };

            let stderr_text = if let Some(task) = stderr_task {
                task.await.unwrap_or_default()
            } else {
                String::new()
            };

            if !status.success() {
                // TODO: surface as LlmStreamEvent::Auth once that variant exists.
                // The blocking path (run_stream / run_stream_with_options) returns
                // LlmError::Auth; the stream path collapses to a plain Error with a
                // conventional "authentication required" message because
                // LlmStreamEvent has no Auth variant — same gap as codex_local and gemini_local.
                let src = if !error_msg.is_empty() {
                    error_msg
                } else if !stderr_text.is_empty() {
                    stderr_text
                } else {
                    format!("claude exited {:?}", status.code())
                };
                let hay = src.to_lowercase();
                let msg = if hay.contains("not logged in")
                    || hay.contains("please run /login")
                    || hay.contains("authentication required")
                    || hay.contains("unauthorized")
                    || hay.contains("invalid credentials")
                {
                    "authentication required".to_string()
                } else {
                    src
                };
                yield LlmStreamEvent::Error { message: msg };
                return;
            }

            // NOTE: auth-keyword normalization runs only in the non-zero-exit branch
            // above. This branch (zero exit + result.is_error=true) surfaces
            // non-auth application errors — Claude CLI signals auth failures by
            // exiting non-zero, not via result.is_error on a successful exit.
            // Matches the blocking path (src/claude_local.rs run_stream_with_options).
            if has_error {
                yield LlmStreamEvent::Error {
                    message: if error_msg.is_empty() {
                        "unknown error".into()
                    } else {
                        error_msg
                    },
                };
                return;
            }

            if model.is_empty() {
                model = "claude-auto".to_string();
            }

            yield LlmStreamEvent::End {
                total: LlmResponse {
                    rendered_text: accumulated,
                    thinking_text: if thinking_accumulator.is_empty() {
                        None
                    } else {
                        Some(thinking_accumulator)
                    },
                    model,
                    estimated_cost_usd: cost,
                    input_tokens,
                    output_tokens,
                    ..Default::default()
                },
            };
        })
    }

    fn run_stream(
        provider: &ClaudeLocalProvider,
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> Result<(String, String, f64), LlmError> {
        let args = Self::build_args(&provider.model, messages, system_prompt);
        let mut cmd = Command::new("claude")
            .args(&args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| LlmError::Other {
                provider: "claude_local".into(),
                message: format!("failed to spawn claude CLI: {e}"),
            })?;

        let stdout = cmd.stdout.take().ok_or_else(|| LlmError::Other {
            provider: "claude_local".into(),
            message: "failed to take stdout".into(),
        })?;

        let reader = BufReader::new(stdout);
        let mut result_text = String::new();
        let mut model = String::new();
        let mut cost = 0.0;
        let mut has_error = false;
        let mut error_msg = String::new();

        for line in reader.lines() {
            let line = line.map_err(|e| LlmError::Other {
                provider: "claude_local".into(),
                message: format!("failed to read stdout line: {e}"),
            })?;

            if line.is_empty() {
                continue;
            }

            if let Ok(event) = serde_json::from_str::<Value>(&line) {
                if let Some(event_type) = event.get("type").and_then(|v| v.as_str()) {
                    match event_type {
                        "result" => {
                            if let Some(text) = event.get("result").and_then(|t| t.as_str()) {
                                result_text = text.to_string();
                            }
                            cost = event
                                .get("total_cost_usd")
                                .and_then(|c| c.as_f64())
                                .unwrap_or(0.0);
                            if let Some(is_err) = event.get("is_error").and_then(|b| b.as_bool()) {
                                if is_err {
                                    has_error = true;
                                    if let Some(reason) =
                                        event.get("result").and_then(|r| r.as_str())
                                    {
                                        error_msg = reason.to_string();
                                    }
                                }
                            }
                        }
                        "assistant" => {
                            // In stream-json mode, assistant messages contain the text
                            if let Some(msg) = event.get("message") {
                                if let Some(content) = msg.get("content") {
                                    if let Some(arr) = content.as_array() {
                                        for part in arr {
                                            if let Some(text) =
                                                part.get("text").and_then(|t| t.as_str())
                                            {
                                                result_text.push_str(text);
                                            }
                                        }
                                    }
                                }
                                if let Some(model_name) = msg.get("model").and_then(|v| v.as_str())
                                {
                                    model = model_name.to_string();
                                }
                            }
                        }
                        "system" => {
                            // In stream-json mode, init event contains model info
                            if let Some(model_name) = event.get("model").and_then(|v| v.as_str()) {
                                model = model_name.to_string();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let exit_status = cmd.wait().map_err(|e| LlmError::Other {
            provider: "claude_local".into(),
            message: format!("failed to wait for claude CLI: {e}"),
        })?;

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
                format!("claude CLI exited with status: {exit_status}")
            };

            let haystack = err_source.to_lowercase();
            if haystack.contains("not logged in")
                || haystack.contains("please run /login")
                || haystack.contains("authentication required")
                || haystack.contains("unauthorized")
                || haystack.contains("invalid credentials")
            {
                return Err(LlmError::Auth {
                    provider: "claude_local".into(),
                });
            }

            return Err(LlmError::Other {
                provider: "claude_local".into(),
                message: err_source,
            });
        }

        if has_error {
            return Err(LlmError::Other {
                provider: "claude_local".into(),
                message: if error_msg.is_empty() {
                    "unknown error".into()
                } else {
                    error_msg
                },
            });
        }

        if model.is_empty() {
            model = "claude-auto".to_string();
        }

        Ok((result_text, model, cost))
    }

    fn run_stream_with_options(
        provider: &ClaudeLocalProvider,
        system_prompt: &str,
        messages: &[ChatMessage],
        options: &crate::LlmRequestOptions,
    ) -> RunStreamOptResult {
        let (args, _temp_images) =
            Self::build_args_with_options(&provider.model, messages, system_prompt, options);
        // _temp_images held alive until subprocess exits (end of this function).
        let binary = crate::cli_utils::resolve_binary("claude");
        let mut command = Command::new(&binary);
        command
            .args(&args)
            .env("PATH", crate::cli_utils::build_path_env())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        if let Some(dir) = options.working_dir.as_ref() {
            command.current_dir(dir);
        }

        let mut cmd = command.spawn().map_err(|e| LlmError::Other {
            provider: "claude_local".into(),
            message: format!("failed to spawn claude CLI: {e}"),
        })?;

        let stdout = cmd.stdout.take().ok_or_else(|| LlmError::Other {
            provider: "claude_local".into(),
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
        let mut result_text = String::new();
        let mut model = String::new();
        let mut cost = 0.0;
        let mut input_tokens: Option<u32> = None;
        let mut output_tokens: Option<u32> = None;
        let mut has_error = false;
        let mut error_msg = String::new();

        for line in reader.lines() {
            let line = line.map_err(|e| LlmError::Other {
                provider: "claude_local".into(),
                message: format!("failed to read stdout line: {e}"),
            })?;

            if line.is_empty() {
                continue;
            }

            if let Ok(event) = serde_json::from_str::<Value>(&line) {
                if let Some(event_type) = event.get("type").and_then(|v| v.as_str()) {
                    match event_type {
                        "result" => {
                            if let Some(text) = event.get("result").and_then(|t| t.as_str()) {
                                result_text = text.to_string();
                            }
                            cost = event
                                .get("total_cost_usd")
                                .and_then(|c| c.as_f64())
                                .unwrap_or(0.0);
                            if let Some(is_err) = event.get("is_error").and_then(|b| b.as_bool()) {
                                if is_err {
                                    has_error = true;
                                    if let Some(reason) =
                                        event.get("result").and_then(|r| r.as_str())
                                    {
                                        error_msg = reason.to_string();
                                    }
                                }
                            }
                            // Final usage totals in result event take precedence.
                            if let Some(usage) = event.get("usage") {
                                if let Some(n) = usage.get("input_tokens").and_then(|v| v.as_u64())
                                {
                                    input_tokens = u32::try_from(n).ok();
                                }
                                if let Some(n) = usage.get("output_tokens").and_then(|v| v.as_u64())
                                {
                                    output_tokens = u32::try_from(n).ok();
                                }
                            }
                        }
                        "assistant" => {
                            // In stream-json mode, assistant messages contain the text
                            if let Some(msg) = event.get("message") {
                                if let Some(content) = msg.get("content") {
                                    if let Some(arr) = content.as_array() {
                                        for part in arr {
                                            if let Some(text) =
                                                part.get("text").and_then(|t| t.as_str())
                                            {
                                                result_text.push_str(text);
                                            }
                                        }
                                    }
                                }
                                if let Some(model_name) = msg.get("model").and_then(|v| v.as_str())
                                {
                                    model = model_name.to_string();
                                }
                                if let Some(usage) = msg.get("usage") {
                                    if let Some(n) =
                                        usage.get("input_tokens").and_then(|v| v.as_u64())
                                    {
                                        input_tokens = u32::try_from(n).ok();
                                    }
                                    if let Some(n) =
                                        usage.get("output_tokens").and_then(|v| v.as_u64())
                                    {
                                        output_tokens = u32::try_from(n).ok();
                                    }
                                }
                            }
                        }
                        "system" => {
                            // In stream-json mode, init event contains model info
                            if let Some(model_name) = event.get("model").and_then(|v| v.as_str()) {
                                model = model_name.to_string();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let exit_status = cmd.wait().map_err(|e| LlmError::Other {
            provider: "claude_local".into(),
            message: format!("failed to wait for claude CLI: {e}"),
        })?;

        let stderr = stderr_thread
            .map(|h| h.join().unwrap_or_default())
            .unwrap_or_default();

        if !exit_status.success() {
            let err_source = if !error_msg.is_empty() {
                error_msg
            } else if !stderr.is_empty() {
                stderr
            } else {
                format!("claude CLI exited with status: {exit_status}")
            };

            let haystack = err_source.to_lowercase();
            if haystack.contains("not logged in")
                || haystack.contains("please run /login")
                || haystack.contains("authentication required")
                || haystack.contains("unauthorized")
                || haystack.contains("invalid credentials")
            {
                return Err(LlmError::Auth {
                    provider: "claude_local".into(),
                });
            }

            return Err(LlmError::Other {
                provider: "claude_local".into(),
                message: err_source,
            });
        }

        if has_error {
            return Err(LlmError::Other {
                provider: "claude_local".into(),
                message: if error_msg.is_empty() {
                    "unknown error".into()
                } else {
                    error_msg
                },
            });
        }

        if model.is_empty() {
            model = "claude-auto".to_string();
        }

        Ok((result_text, model, cost, input_tokens, output_tokens))
    }

    /// Build the base CLI args from messages, returning the prompt string and
    /// any temporary image files that must stay alive until the subprocess exits.
    ///
    /// Multipart [`ChatContent`] handling:
    /// - `Text`: appended to the prompt as before (backward-compatible).
    /// - `Image { Base64 }`: decoded → written to a temp file → path collected
    ///   into `temp_images`; callers pass those paths via `--image` flags.
    /// - `Image { Url }`: warn + skip (local CLIs cannot fetch URLs).
    /// - `ToolResult`: debug-log + skip (local CLIs manage their own tools).
    fn build_args_inner(
        model: &str,
        messages: &[ChatMessage],
        system_prompt: &str,
    ) -> (Vec<String>, Vec<crate::cli_utils::TempFile>) {
        let mut fixed_args = vec![
            "-p".to_string(),
            "--output-format".to_string(),
            "json".to_string(),
            "--dangerously-skip-permissions".to_string(),
            "--no-session-persistence".to_string(),
        ];

        if !model.is_empty() && model != DEFAULT_MODEL {
            fixed_args.push("--model".to_string());
            fixed_args.push(model.to_string());
        }

        let mut prompt = String::new();
        if !system_prompt.is_empty() {
            prompt.push_str(system_prompt);
            prompt.push_str("\n\n");
        }

        let mut temp_images: Vec<crate::cli_utils::TempFile> = Vec::new();

        for m in messages {
            // Role header — only emit when the message has at least one Text block.
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
                    crate::ChatContent::Image { source, mime_type } => {
                        match source {
                            crate::ImageSource::Base64(data) => {
                                match crate::cli_utils::decode_base64_image(data, mime_type) {
                                    Ok(tmp) => {
                                        // Path is referenced later via --image flags.
                                        temp_images.push(tmp);
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "claude_local: failed to decode base64 image; skipping: {}",
                                            e
                                        );
                                    }
                                }
                            }
                            crate::ImageSource::Url(url) => {
                                tracing::warn!(
                                    "claude_local: URL image source unsupported (would need pre-download); skipping: {}",
                                    url
                                );
                            }
                        }
                    }
                    crate::ChatContent::ToolResult { tool_use_id, .. } => {
                        tracing::debug!(
                            "claude_local: dropping tool_result block (CLI handles its own internal tools); tool_use_id={}",
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
    ///
    /// Text-only messages produce an identical CLI invocation to v0.3.x.
    fn build_args(model: &str, messages: &[ChatMessage], system_prompt: &str) -> Vec<String> {
        // Discard temp_images: the blocking path in run_stream holds them
        // alive indirectly since they are created before the subprocess
        // exits within the same function call.
        let (args, _temp_images) = Self::build_args_inner(model, messages, system_prompt);
        args
    }

    fn build_args_with_options(
        model: &str,
        messages: &[ChatMessage],
        system_prompt: &str,
        options: &crate::LlmRequestOptions,
    ) -> (Vec<String>, Vec<crate::cli_utils::TempFile>) {
        // Start from base args. build_args_inner puts the prompt as the LAST
        // arg. We need to inject option flags BEFORE the prompt.
        let (base, temp_images) = Self::build_args_inner(model, messages, system_prompt);
        let (prompt, head) = base
            .split_last()
            .expect("build_args_inner must return at least one element (prompt)");
        let mut args: Vec<String> = head.to_vec();
        let prompt = prompt.clone();

        // Tools allow-list — per-flag semantics (see LlmRequestOptions docs).
        let mut allowed_tools: Vec<&str> = Vec::new();
        if options.allow_web_search {
            allowed_tools.push("WebSearch");
            allowed_tools.push("WebFetch");
        }
        if options.allow_local_read {
            allowed_tools.push("Read");
        }
        if !allowed_tools.is_empty() {
            args.push("--allowedTools".to_string());
            args.push(allowed_tools.join(","));
        }

        if let Some(dir) = options.working_dir.as_ref() {
            args.push("--add-dir".to_string());
            args.push(dir.to_string_lossy().to_string());
        }

        if let Some(max) = options.max_turns {
            args.push("--max-turns".to_string());
            args.push(max.to_string());
        }

        // Thinking budget: pass --thinking-budget <n> when set.
        // Claude Code CLI may or may not support this flag depending on the
        // installed version. If the CLI rejects the flag it exits non-zero and
        // the caller receives an LlmStreamEvent::Error, which is acceptable
        // graceful degradation. Verified supported flag name as of Claude Code
        // CLI ≥ 1.x; if your CLI version differs, remove these two lines and
        // the warning will surface via the error event.
        if let Some(budget) = options.thinking_budget_tokens {
            args.push("--thinking-budget".to_string());
            args.push(budget.to_string());
        }

        // Image attachments: decoded-from-message temp files first, then
        // explicit caller-supplied paths. Claude CLI `--image` support is
        // unverified; if the CLI rejects unknown flags the error surfaces as
        // a subprocess error.
        for tmp in &temp_images {
            args.push("--image".to_string());
            args.push(tmp.0.to_string_lossy().to_string());
        }
        for img in &options.image_paths {
            args.push("--image".to_string());
            args.push(img.to_string_lossy().to_string());
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
    // claude_local does not require an API key — it uses the local Claude Code
    // CLI which authenticates via the user's Claude subscription (OAuth/keychain).
    let model = if model.is_empty() {
        DEFAULT_MODEL.to_string()
    } else {
        model.to_string()
    };
    Ok(Box::new(ClaudeLocalProvider::new(model)))
}

/// Read the configured model from `~/.claude/claude.json`.
///
/// The Claude Code CLI does not have a model listing command, so this
/// reads the currently configured `defaultModel` from the config file and
/// returns it as a single-element list. Returns an empty list if the config
/// is missing or has no model set.
pub async fn list_models() -> Result<Vec<crate::ListModel>, LlmError> {
    // Try ~/.claude/claude.json first
    let config_path = std::env::var("HOME").ok().map(|h| {
        std::path::PathBuf::from(h)
            .join(".claude")
            .join("claude.json")
    });

    let contents = match config_path {
        Some(p) => tokio::fs::read_to_string(&p).await.ok(),
        None => None,
    };

    if let Some(c) = contents {
        if !c.is_empty() {
            return Ok(parse_config_model(&c));
        }
    }

    // Fallback to XDG config dir
    if let Some(dir) = dirs::config_dir() {
        let xdg_path = dir.join("claude").join("claude.json");
        if let Ok(c) = tokio::fs::read_to_string(&xdg_path).await {
            if !c.is_empty() {
                return Ok(parse_config_model(&c));
            }
        }
    }

    // Fallback to CLAUDE_MODEL env var
    if let Ok(model) = std::env::var("CLAUDE_MODEL") {
        if !model.is_empty() {
            return Ok(vec![crate::ListModel {
                name: model,
                display_name: String::new(),
                description: None,
                input_token_limit: None,
                output_token_limit: None,
            }]);
        }
    }

    Ok(Vec::new())
}

fn parse_config_model(contents: &str) -> Vec<crate::ListModel> {
    if let Ok(json) = serde_json::from_str::<Value>(contents) {
        if let Some(model) = json
            .get("defaultModel")
            .or_else(|| json.get("default_model"))
            .or_else(|| json.get("settings").and_then(|s| s.get("defaultModel")))
            .or_else(|| json.get("settings").and_then(|s| s.get("default_model")))
            .and_then(|v| v.as_str())
        {
            if !model.is_empty() {
                return vec![crate::ListModel {
                    name: model.to_string(),
                    display_name: String::new(),
                    description: None,
                    input_token_limit: None,
                    output_token_limit: None,
                }];
            }
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialize tests that mutate `CLAUDE_CLI_PATH` so their env-var sets/unsets
    /// don't race when cargo-test runs them in parallel threads. Uses
    /// `tokio::sync::Mutex` so it is safe to hold the guard across `.await`
    /// while the stream is consumed (`std::sync::Mutex` would trip
    /// clippy::await_holding_lock).
    static CLAUDE_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[test]
    fn make_with_empty_model_returns_default() {
        let result = make("", "", "");
        assert!(result.is_ok(), "make should succeed with empty model");
    }

    #[test]
    fn make_with_model_returns_provider() {
        let result = make("", "sonnet", "");
        assert!(result.is_ok(), "make should succeed with model");
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "requires claude CLI installed locally with valid auth"]
    async fn full_chat_via_cli_returns_text() {
        let provider = ClaudeLocalProvider::new("auto".to_string());
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
            .expect("claude CLI should respond");

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
    async fn list_models_reads_configured_model() {
        let models = list_models().await.expect("list_models should succeed");
        // May return empty if no config file exists (first-run scenario)
        // but should not panic
        let _ = models;
    }

    #[test]
    fn build_args_includes_max_turns_when_set() {
        let opts = crate::LlmRequestOptions {
            max_turns: Some(7),
            ..Default::default()
        };
        let messages = vec![crate::ChatMessage::user("hi")];
        let (args, _) =
            ClaudeLocalProvider::build_args_with_options("auto", &messages, "sys", &opts);
        let idx = args
            .iter()
            .position(|a| a == "--max-turns")
            .expect("--max-turns present");
        assert_eq!(args[idx + 1], "7", "args: {args:?}");
        assert!(
            args.last().unwrap().contains("[user]"),
            "prompt must remain the final argument: {args:?}"
        );
    }

    #[test]
    fn build_args_includes_allowed_tools_when_search_enabled() {
        let opts = crate::LlmRequestOptions {
            allow_web_search: true,
            ..Default::default()
        };
        let messages = vec![crate::ChatMessage::user("hi")];
        let (args, _) =
            ClaudeLocalProvider::build_args_with_options("auto", &messages, "sys", &opts);
        let tools_idx = args
            .iter()
            .position(|a| a == "--allowedTools")
            .expect("--allowedTools present");
        let tools_value = &args[tools_idx + 1];
        assert!(tools_value.contains("WebSearch"), "tools: {tools_value}");
        assert!(tools_value.contains("WebFetch"), "tools: {tools_value}");
        assert!(
            args.last().unwrap().contains("[user]"),
            "prompt must remain the final argument: {args:?}"
        );
    }

    #[test]
    fn build_args_includes_add_dir_when_working_dir_set() {
        let opts = crate::LlmRequestOptions {
            working_dir: Some(std::path::PathBuf::from("/tmp/work")),
            ..Default::default()
        };
        let messages = vec![crate::ChatMessage::user("hi")];
        let (args, _) =
            ClaudeLocalProvider::build_args_with_options("auto", &messages, "sys", &opts);
        let idx = args
            .iter()
            .position(|a| a == "--add-dir")
            .expect("--add-dir present");
        assert_eq!(args[idx + 1], "/tmp/work", "args: {args:?}");
        assert!(
            args.last().unwrap().contains("[user]"),
            "prompt must remain the final argument: {args:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn claude_stream_emits_deltas_from_assistant_events() {
        use crate::{LlmProviderDyn, LlmRequestOptions, LlmStreamEvent};
        use futures::StreamExt;
        use serde_json::Value;

        let _guard = CLAUDE_ENV_LOCK.lock().await;

        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/fake_claude.sh");
        assert!(fixture.exists(), "fixture missing: {}", fixture.display());

        // UNSAFETY RATIONALE: `set_var` is not thread-safe; serialized by CLAUDE_ENV_LOCK.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("CLAUDE_CLI_PATH", fixture.to_str().unwrap());
        }

        let provider = super::ClaudeLocalProvider::new("claude-sonnet-4-5".to_string());
        let opts = LlmRequestOptions::default();
        let messages = vec![crate::ChatMessage::user("hi")];
        let mut stream =
            provider.chat_stream_with_options_dyn("sys", &messages, &opts, &Value::Null);

        let mut deltas: Vec<String> = Vec::new();
        let mut end_resp = None;
        while let Some(event) = stream.next().await {
            match event {
                LlmStreamEvent::Delta { text } => deltas.push(text),
                LlmStreamEvent::End { total } => end_resp = Some(total),
                LlmStreamEvent::Error { message } => panic!("error: {message}"),
                _ => {}
            }
        }

        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("CLAUDE_CLI_PATH");
        }

        assert_eq!(deltas.len(), 2, "expected 2 deltas, got: {deltas:?}");
        assert_eq!(deltas.concat(), "Hi there!");

        let end = end_resp.expect("End missing");
        assert_eq!(end.rendered_text, "Hi there!");
        assert!((end.estimated_cost_usd - 0.001).abs() < 1e-9);
        assert_eq!(end.input_tokens, Some(5));
        assert_eq!(end.output_tokens, Some(3));
        assert_eq!(end.model, "claude-sonnet-4-5");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn timeout_aborts_blocking_call() {
        use crate::{LlmError, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use std::time::Duration;

        let _guard = CLAUDE_ENV_LOCK.lock().await;

        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/fake_claude_slow.sh");
        assert!(fixture.exists(), "fixture missing: {}", fixture.display());

        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("CLAUDE_CLI_PATH", fixture.to_str().unwrap());
        }

        let provider = ClaudeLocalProvider::new("auto".to_string());
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
            std::env::remove_var("CLAUDE_CLI_PATH");
        }

        assert!(
            matches!(
                res,
                Err(LlmError::Network { ref provider, ref msg })
                    if provider == "claude_local" && msg.contains("timeout")
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

        let _guard = CLAUDE_ENV_LOCK.lock().await;

        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/fake_claude_slow.sh");
        assert!(fixture.exists(), "fixture missing: {}", fixture.display());

        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("CLAUDE_CLI_PATH", fixture.to_str().unwrap());
        }

        let provider = ClaudeLocalProvider::new("auto".to_string());
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
            std::env::remove_var("CLAUDE_CLI_PATH");
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

    #[tokio::test(flavor = "multi_thread")]
    async fn thinking_block_emits_thinking_event_and_populates_total() {
        use crate::{ChatMessage, LlmProviderDyn, LlmRequestOptions, LlmStreamEvent};
        use futures::StreamExt;
        use serde_json::Value;

        let _guard = CLAUDE_ENV_LOCK.lock().await;
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/fake_claude_thinking.sh");
        assert!(fixture.exists(), "fixture missing: {}", fixture.display());

        // UNSAFETY RATIONALE: `set_var` is not thread-safe; serialized by CLAUDE_ENV_LOCK.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("CLAUDE_CLI_PATH", fixture.to_str().unwrap());
        }

        let provider = ClaudeLocalProvider::new("sonnet".into());
        let opts = LlmRequestOptions::default();
        let messages = vec![ChatMessage::user("test")];
        let mut stream = provider.chat_stream_with_options_dyn("", &messages, &opts, &Value::Null);

        let mut thinking_count = 0;
        let mut delta_count = 0;
        let mut final_thinking_text: Option<String> = None;
        while let Some(event) = stream.next().await {
            match event {
                LlmStreamEvent::Thinking { .. } => thinking_count += 1,
                LlmStreamEvent::Delta { .. } => delta_count += 1,
                LlmStreamEvent::End { total } => {
                    final_thinking_text = total.thinking_text;
                    break;
                }
                LlmStreamEvent::Error { message } => panic!("unexpected error: {message}"),
                _ => {}
            }
        }

        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("CLAUDE_CLI_PATH");
        }

        assert_eq!(thinking_count, 1, "expected 1 Thinking event");
        assert!(
            delta_count >= 1,
            "expected at least 1 Delta event for the text"
        );
        assert_eq!(final_thinking_text.as_deref(), Some("Reasoning step..."));
    }

    #[test]
    fn build_args_includes_thinking_budget_when_set() {
        let opts = crate::LlmRequestOptions {
            thinking_budget_tokens: Some(8192),
            ..Default::default()
        };
        let messages = vec![crate::ChatMessage::user("hi")];
        let (args, _) =
            ClaudeLocalProvider::build_args_with_options("auto", &messages, "sys", &opts);
        let idx = args
            .iter()
            .position(|a| a == "--thinking-budget")
            .expect("--thinking-budget present");
        assert_eq!(args[idx + 1], "8192", "args: {args:?}");
        assert!(
            args.last().unwrap().contains("[user]"),
            "prompt must remain the final argument: {args:?}"
        );
    }

    #[test]
    fn build_args_no_thinking_budget_flag_when_none() {
        let opts = crate::LlmRequestOptions::default();
        let messages = vec![crate::ChatMessage::user("hi")];
        let (args, _) =
            ClaudeLocalProvider::build_args_with_options("auto", &messages, "sys", &opts);
        assert!(
            !args.contains(&"--thinking-budget".to_string()),
            "no --thinking-budget when budget is None: {args:?}"
        );
    }

    /// A 1x1 transparent PNG (67 bytes) in raw bytes — smallest valid PNG.
    const TINY_PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk length + type
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // width=1, height=1
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, // 8-bit RGB, CRC
        0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, // IDAT chunk
        0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, // compressed data
        0x00, 0x00, 0x02, 0x00, 0x01, 0xE2, 0x21, 0xBC, // IDAT CRC
        0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, // IEND chunk
        0x44, 0xAE, 0x42, 0x60, 0x82, // IEND CRC
    ];

    #[test]
    fn multipart_image_base64_block_adds_image_path_arg() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD.encode(TINY_PNG);
        let img_block = crate::ChatContent::Image {
            source: crate::ImageSource::Base64(b64),
            mime_type: "image/png".to_string(),
        };
        let messages = vec![crate::ChatMessage::user_with_image(
            "describe this",
            img_block,
        )];
        let opts = crate::LlmRequestOptions::default();
        let (args, temp_images) =
            ClaudeLocalProvider::build_args_with_options("auto", &messages, "", &opts);

        // At least one --image flag must be present.
        assert!(
            args.iter().any(|a| a == "--image"),
            "expected --image flag in args: {args:?}"
        );
        // The path arg after --image must point to an existing temp file.
        let img_idx = args.iter().position(|a| a == "--image").unwrap();
        let img_path = std::path::PathBuf::from(&args[img_idx + 1]);
        assert!(
            img_path.exists(),
            "temp image file should exist on disk: {}",
            img_path.display()
        );
        // Prompt must remain the last arg.
        assert!(
            args.last().unwrap().contains("describe this"),
            "prompt must be last: {args:?}"
        );
        // Drop guard — file removed after this.
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
        let (args, _) = ClaudeLocalProvider::build_args_with_options("auto", &messages, "", &opts);

        // The prompt (last arg) must NOT contain the tool_result content.
        let prompt = args.last().expect("args not empty");
        assert!(
            !prompt.contains("25C in Seoul"),
            "tool_result content must be dropped from prompt, but got: {prompt}"
        );
        // Regular text must still appear.
        assert!(
            prompt.contains("What is the weather?"),
            "text message must appear in prompt: {prompt}"
        );
    }
}
