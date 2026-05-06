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
pub struct GeminiLocalProvider {
    model: String,
    sandbox: bool,
    resume_session: Option<String>,
}

impl GeminiLocalProvider {
    pub fn new(model: String) -> Self {
        Self {
            model,
            sandbox: false,
            resume_session: None,
        }
    }

    pub fn new_with_sandbox(model: String, sandbox: bool) -> Self {
        Self {
            model,
            sandbox,
            resume_session: None,
        }
    }

    pub fn with_resume_session(mut self, session_id: String) -> Self {
        self.resume_session = Some(session_id);
        self
    }
}

impl LlmProviderDyn for GeminiLocalProvider {
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
        let provider = self.clone();
        let system = system_prompt.to_string();
        let msgs = messages.to_vec();
        let opts = crate::LlmRequestOptions::default();
        GeminiLocalProvider::run_stream_async(provider, system, msgs, opts)
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
                                    provider: "gemini_local".into(),
                                    msg: "timeout".into(),
                                }
                            } else {
                                LlmError::Other {
                                    provider: "gemini_local".into(),
                                    message,
                                }
                            });
                        }
                        _ => {}
                    }
                }
                final_response.ok_or_else(|| LlmError::Other {
                    provider: "gemini_local".into(),
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
                    GeminiLocalProvider::run_stream_with_options(&provider, &system, &msgs, &opts)
                })
                .await
                .unwrap_or_else(|e| {
                    Err(LlmError::Other {
                        provider: "gemini_local".into(),
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
        let provider = self.clone();
        let system = system_prompt.to_string();
        let msgs = messages.to_vec();
        let opts = options.clone();
        GeminiLocalProvider::run_stream_async(provider, system, msgs, opts)
    }
}

impl GeminiLocalProvider {
    async fn chat_impl(
        provider: &GeminiLocalProvider,
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> Result<LlmResponse, LlmError> {
        let provider = provider.clone();
        let system = system_prompt.to_string();
        let msgs = messages.to_vec();
        let result = tokio::task::spawn_blocking(move || {
            GeminiLocalProvider::run_stream(&provider, &system, &msgs)
        })
        .await
        .unwrap_or_else(|e| {
            Err(LlmError::Other {
                provider: "gemini_local".into(),
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

    fn run_stream(
        provider: &GeminiLocalProvider,
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> Result<(String, String, f64), LlmError> {
        let args = Self::build_args(
            &provider.model,
            provider.sandbox,
            provider.resume_session.as_deref(),
            system_prompt,
            messages,
        );
        let mut cmd = Command::new("gemini")
            .args(&args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| LlmError::Other {
                provider: "gemini_local".into(),
                message: format!("failed to spawn gemini CLI: {e}"),
            })?;

        let stdout = cmd.stdout.take().ok_or_else(|| LlmError::Other {
            provider: "gemini_local".into(),
            message: "failed to take stdout".into(),
        })?;

        let reader = BufReader::new(stdout);
        let mut full_text = String::new();
        let mut model = String::new();
        let mut cost = 0.0;
        let mut has_error = false;
        let mut error_msg = String::new();

        for line in reader.lines() {
            let line = line.map_err(|e| LlmError::Other {
                provider: "gemini_local".into(),
                message: format!("failed to read stdout line: {e}"),
            })?;

            if line.is_empty() {
                continue;
            }

            if let Ok(event) = serde_json::from_str::<Value>(&line) {
                if let Some(event_type) = event.get("type").and_then(|v| v.as_str()) {
                    match event_type {
                        "init" | "system" => {
                            if let Some(m) = event.get("model").and_then(|v| v.as_str()) {
                                model = m.to_string();
                            }
                        }
                        "message" => {
                            if let Some(role) = event.get("role").and_then(|v| v.as_str()) {
                                if role == "assistant" {
                                    if let Some(content) =
                                        event.get("content").and_then(|v| v.as_str())
                                    {
                                        full_text.push_str(content);
                                    }
                                }
                            }
                        }
                        "assistant" => {
                            if let Some(content) = event.get("content").and_then(|v| v.as_str()) {
                                full_text.push_str(content);
                            } else if let Some(msg) = event.get("message") {
                                if let Some(parts) = msg.get("content").and_then(|v| v.as_array()) {
                                    for part in parts {
                                        if let Some(text) =
                                            part.get("text").and_then(|v| v.as_str())
                                        {
                                            full_text.push_str(text);
                                        } else if let Some(text) =
                                            part.get("content").and_then(|v| v.as_str())
                                        {
                                            full_text.push_str(text);
                                        }
                                    }
                                }
                            }
                        }
                        "result" => {
                            if let Some(status) = event.get("status").and_then(|v| v.as_str()) {
                                if status == "error" || status == "failure" {
                                    has_error = true;
                                    if let Some(msg) = event.get("message").and_then(|v| v.as_str())
                                    {
                                        error_msg = msg.to_string();
                                    }
                                }
                            }
                            if let Some(c) = event.get("total_cost_usd").and_then(|v| v.as_f64()) {
                                cost = c;
                            } else if let Some(usage) = event.get("usage") {
                                if let Some(p) = usage
                                    .get("promptTokenCount")
                                    .or_else(|| usage.get("input_tokens"))
                                {
                                    cost += p.as_f64().unwrap_or(0.0) * 0.000001;
                                }
                                if let Some(c) = usage
                                    .get("candidatesTokenCount")
                                    .or_else(|| usage.get("output_tokens"))
                                {
                                    cost += c.as_f64().unwrap_or(0.0) * 0.000003;
                                }
                            } else if let Some(stats) = event.get("stats") {
                                if let Some(p) = stats.get("input_tokens") {
                                    cost += p.as_f64().unwrap_or(0.0) * 0.000001;
                                }
                                if let Some(c) = stats.get("output_tokens") {
                                    cost += c.as_f64().unwrap_or(0.0) * 0.000003;
                                }
                            }
                        }
                        "error" => {
                            has_error = true;
                            if let Some(msg) = event.get("message").and_then(|v| v.as_str()) {
                                error_msg = msg.to_string();
                            } else if let Some(text) = event.get("text").and_then(|v| v.as_str()) {
                                error_msg = text.to_string();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let exit_status = cmd.wait().map_err(|e| LlmError::Other {
            provider: "gemini_local".into(),
            message: format!("failed to wait for gemini CLI: {e}"),
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
                format!("gemini CLI exited with status: {exit_status}")
            };

            // Detect auth-required patterns
            let haystack = err_source.to_lowercase();
            if haystack.contains("not authenticated")
                || haystack.contains("please authenticate")
                || haystack.contains("api_key")
                || haystack.contains("authentication required")
                || haystack.contains("unauthorized")
                || haystack.contains("invalid credentials")
                || haystack.contains("not logged in")
                || haystack.contains("login required")
                || haystack.contains("run `gemini auth")
                || haystack.contains("gemini auth")
            {
                return Err(LlmError::Auth {
                    provider: "gemini_local".into(),
                });
            }

            return Err(LlmError::Other {
                provider: "gemini_local".into(),
                message: err_source,
            });
        }

        if has_error {
            return Err(LlmError::Other {
                provider: "gemini_local".into(),
                message: if error_msg.is_empty() {
                    "unknown error".into()
                } else {
                    error_msg
                },
            });
        }

        if model.is_empty() {
            model = "gemini-auto".to_string();
        }

        Ok((full_text, model, cost))
    }

    fn run_stream_with_options(
        provider: &GeminiLocalProvider,
        system_prompt: &str,
        messages: &[ChatMessage],
        options: &crate::LlmRequestOptions,
    ) -> RunStreamOptResult {
        let (args, _temp_images) = Self::build_args_with_options(
            &provider.model,
            provider.sandbox,
            provider.resume_session.as_deref(),
            system_prompt,
            messages,
            options,
        );
        // _temp_images held alive until subprocess exits (end of this function).
        let binary = crate::cli_utils::resolve_binary("gemini");
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
            provider: "gemini_local".into(),
            message: format!("failed to spawn gemini CLI: {e}"),
        })?;

        let stdout = cmd.stdout.take().ok_or_else(|| LlmError::Other {
            provider: "gemini_local".into(),
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
        let mut has_error = false;
        let mut error_msg = String::new();

        for line in reader.lines() {
            let line = line.map_err(|e| LlmError::Other {
                provider: "gemini_local".into(),
                message: format!("failed to read stdout line: {e}"),
            })?;

            if line.is_empty() {
                continue;
            }

            if let Ok(event) = serde_json::from_str::<Value>(&line) {
                if let Some(event_type) = event.get("type").and_then(|v| v.as_str()) {
                    match event_type {
                        "init" | "system" => {
                            if let Some(m) = event.get("model").and_then(|v| v.as_str()) {
                                model = m.to_string();
                            }
                        }
                        "message" => {
                            if let Some(role) = event.get("role").and_then(|v| v.as_str()) {
                                if role == "assistant" {
                                    if let Some(content) =
                                        event.get("content").and_then(|v| v.as_str())
                                    {
                                        full_text.push_str(content);
                                    }
                                }
                            }
                        }
                        "assistant" => {
                            if let Some(content) = event.get("content").and_then(|v| v.as_str()) {
                                full_text.push_str(content);
                            } else if let Some(msg) = event.get("message") {
                                if let Some(parts) = msg.get("content").and_then(|v| v.as_array()) {
                                    for part in parts {
                                        if let Some(text) =
                                            part.get("text").and_then(|v| v.as_str())
                                        {
                                            full_text.push_str(text);
                                        } else if let Some(text) =
                                            part.get("content").and_then(|v| v.as_str())
                                        {
                                            full_text.push_str(text);
                                        }
                                    }
                                }
                            }
                        }
                        "result" => {
                            if let Some(status) = event.get("status").and_then(|v| v.as_str()) {
                                if status == "error" || status == "failure" {
                                    has_error = true;
                                    if let Some(msg) = event.get("message").and_then(|v| v.as_str())
                                    {
                                        error_msg = msg.to_string();
                                    }
                                }
                            }
                            if let Some(c) = event.get("total_cost_usd").and_then(|v| v.as_f64()) {
                                cost = c;
                            } else if let Some(usage) = event.get("usage") {
                                if let Some(p) = usage
                                    .get("promptTokenCount")
                                    .or_else(|| usage.get("input_tokens"))
                                {
                                    cost += p.as_f64().unwrap_or(0.0) * 0.000001;
                                }
                                if let Some(c) = usage
                                    .get("candidatesTokenCount")
                                    .or_else(|| usage.get("output_tokens"))
                                {
                                    cost += c.as_f64().unwrap_or(0.0) * 0.000003;
                                }
                            } else if let Some(stats) = event.get("stats") {
                                if let Some(p) = stats.get("input_tokens") {
                                    cost += p.as_f64().unwrap_or(0.0) * 0.000001;
                                }
                                if let Some(c) = stats.get("output_tokens") {
                                    cost += c.as_f64().unwrap_or(0.0) * 0.000003;
                                }
                            }
                        }
                        "error" => {
                            has_error = true;
                            if let Some(msg) = event.get("message").and_then(|v| v.as_str()) {
                                error_msg = msg.to_string();
                            } else if let Some(text) = event.get("text").and_then(|v| v.as_str()) {
                                error_msg = text.to_string();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let exit_status = cmd.wait().map_err(|e| LlmError::Other {
            provider: "gemini_local".into(),
            message: format!("failed to wait for gemini CLI: {e}"),
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
                format!("gemini CLI exited with status: {exit_status}")
            };

            // Detect auth-required patterns
            let haystack = err_source.to_lowercase();
            if haystack.contains("not authenticated")
                || haystack.contains("please authenticate")
                || haystack.contains("api_key")
                || haystack.contains("authentication required")
                || haystack.contains("unauthorized")
                || haystack.contains("invalid credentials")
                || haystack.contains("not logged in")
                || haystack.contains("login required")
                || haystack.contains("run `gemini auth")
                || haystack.contains("gemini auth")
            {
                return Err(LlmError::Auth {
                    provider: "gemini_local".into(),
                });
            }

            return Err(LlmError::Other {
                provider: "gemini_local".into(),
                message: err_source,
            });
        }

        if has_error {
            return Err(LlmError::Other {
                provider: "gemini_local".into(),
                message: if error_msg.is_empty() {
                    "unknown error".into()
                } else {
                    error_msg
                },
            });
        }

        if model.is_empty() {
            model = "gemini-auto".to_string();
        }

        // Gemini CLI does not report token usage reliably via stream-json, so
        // input_tokens and output_tokens are always None here (see plan line
        // 1132 in docs/plans/2026-04-23-llm-request-options.plan.md).
        Ok((full_text, model, cost, None, None))
    }

    /// Async streaming helper: spawns the Gemini CLI via `tokio::process::Command`,
    /// reads stdout line by line with `AsyncBufReadExt::lines()`, parses each line
    /// as stream-json, and yields `LlmStreamEvent::Delta` for each assistant text
    /// chunk. Emits a final `LlmStreamEvent::End` with the full accumulated text.
    ///
    /// Both `chat_stream_dyn` and `chat_stream_with_options_dyn` delegate here to
    /// avoid ~150 LOC duplication. Accepts fully owned inputs so the returned
    /// `BoxStream` is `'static` and borrow-checker friction is minimal.
    fn run_stream_async(
        provider: GeminiLocalProvider,
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
                provider.sandbox,
                provider.resume_session.as_deref(),
                &system,
                &msgs,
                &opts,
            );
            // _temp_images held alive until stream generator drops (stream end).
            let binary = crate::cli_utils::resolve_binary("gemini");

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
            let mut model = String::new();
            let mut cost: f64 = 0.0;
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
                        if line.is_empty() {
                            continue;
                        }
                        let event: Value = match serde_json::from_str(&line) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        let event_type = event.get("type").and_then(Value::as_str).unwrap_or("");
                        match event_type {
                            "init" | "system" => {
                                if let Some(m) = event.get("model").and_then(Value::as_str) {
                                    model = m.to_string();
                                }
                            }
                            "message" => {
                                if event.get("role").and_then(Value::as_str) == Some("assistant") {
                                    if let Some(content) =
                                        event.get("content").and_then(Value::as_str)
                                    {
                                        if !content.is_empty() {
                                            accumulated.push_str(content);
                                            yield LlmStreamEvent::Delta {
                                                text: content.to_string(),
                                            };
                                        }
                                    }
                                }
                            }
                            "assistant" => {
                                if let Some(content) =
                                    event.get("content").and_then(Value::as_str)
                                {
                                    if !content.is_empty() {
                                        accumulated.push_str(content);
                                        yield LlmStreamEvent::Delta {
                                            text: content.to_string(),
                                        };
                                    }
                                } else if let Some(parts) = event
                                    .get("message")
                                    .and_then(|m| m.get("content"))
                                    .and_then(Value::as_array)
                                {
                                    for part in parts {
                                        let maybe_text = part
                                            .get("text")
                                            .and_then(Value::as_str)
                                            .or_else(|| {
                                                part.get("content").and_then(Value::as_str)
                                            });
                                        if let Some(t) = maybe_text {
                                            if !t.is_empty() {
                                                accumulated.push_str(t);
                                                yield LlmStreamEvent::Delta {
                                                    text: t.to_string(),
                                                };
                                            }
                                        }
                                    }
                                }
                            }
                            "result" => {
                                if let Some(status) =
                                    event.get("status").and_then(Value::as_str)
                                {
                                    if status == "error" || status == "failure" {
                                        has_error = true;
                                        if let Some(m) =
                                            event.get("message").and_then(Value::as_str)
                                        {
                                            error_msg = m.to_string();
                                        }
                                    }
                                }
                                if let Some(c) =
                                    event.get("total_cost_usd").and_then(Value::as_f64)
                                {
                                    cost = c;
                                } else if let Some(usage) = event.get("usage") {
                                    if let Some(p) = usage
                                        .get("promptTokenCount")
                                        .or_else(|| usage.get("input_tokens"))
                                    {
                                        cost += p.as_f64().unwrap_or(0.0) * 0.000001;
                                    }
                                    if let Some(c) = usage
                                        .get("candidatesTokenCount")
                                        .or_else(|| usage.get("output_tokens"))
                                    {
                                        cost += c.as_f64().unwrap_or(0.0) * 0.000003;
                                    }
                                } else if let Some(stats) = event.get("stats") {
                                    if let Some(p) = stats.get("input_tokens") {
                                        cost += p.as_f64().unwrap_or(0.0) * 0.000001;
                                    }
                                    if let Some(c) = stats.get("output_tokens") {
                                        cost += c.as_f64().unwrap_or(0.0) * 0.000003;
                                    }
                                }
                            }
                            "error" => {
                                has_error = true;
                                if let Some(m) = event.get("message").and_then(Value::as_str) {
                                    error_msg = m.to_string();
                                } else if let Some(t) = event.get("text").and_then(Value::as_str)
                                {
                                    error_msg = t.to_string();
                                }
                            }
                            _ => {}
                        }
                    }
                    Some(Ok(None)) => break,
                    Some(Err(e)) => {
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
                // TODO: surface as LlmStreamEvent::Auth once that variant
                // exists. The blocking path (run_stream / run_stream_with_options)
                // returns LlmError::Auth; the stream path collapses to a plain
                // Error with a conventional "authentication required" message
                // because LlmStreamEvent has no Auth variant.
                let src = if !error_msg.is_empty() {
                    error_msg
                } else if !stderr_text.is_empty() {
                    stderr_text
                } else {
                    format!("gemini CLI exited with status: {exit_status}")
                };
                let hay = src.to_lowercase();
                let msg = if hay.contains("not authenticated")
                    || hay.contains("please authenticate")
                    || hay.contains("api_key")
                    || hay.contains("authentication required")
                    || hay.contains("unauthorized")
                    || hay.contains("invalid credentials")
                    || hay.contains("not logged in")
                    || hay.contains("login required")
                    || hay.contains("run `gemini auth")
                    || hay.contains("gemini auth")
                {
                    "authentication required".to_string()
                } else {
                    src
                };
                yield LlmStreamEvent::Error { message: msg };
                return;
            }

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
                model = "gemini-auto".to_string();
            }

            yield LlmStreamEvent::End {
                total: LlmResponse {
                    rendered_text: accumulated,
                    model,
                    estimated_cost_usd: cost,
                    ..Default::default()
                },
            };
        })
    }

    /// Build base CLI args from messages. Returns the arg list and any
    /// temporary image files that must stay alive until the subprocess exits.
    ///
    /// Multipart [`ChatContent`] handling:
    /// - `Text`: appended to prompt (backward-compatible).
    /// - `Image { Base64 }`: decoded → temp file; path appended as `@<path>`
    ///   in the prompt body (Gemini CLI's `read_many_files` mechanism).
    /// - `Image { Url }`: warn + skip.
    /// - `ToolResult`: debug-log + skip.
    fn build_args_inner(
        model: &str,
        sandbox: bool,
        resume_session: Option<&str>,
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> (Vec<String>, Vec<crate::cli_utils::TempFile>) {
        let mut fixed_args = vec![
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--approval-mode".to_string(),
            "yolo".to_string(),
        ];

        if sandbox {
            fixed_args.push("--sandbox".to_string());
        } else {
            fixed_args.push("--sandbox=none".to_string());
        }

        if let Some(session_id) = resume_session {
            fixed_args.push("--resume".to_string());
            fixed_args.push(session_id.to_string());
        }

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
                                        // Gemini CLI reads images via `@<path>` tokens in
                                        // the prompt body, not via a separate --image flag.
                                        if !prompt.ends_with('\n') {
                                            prompt.push('\n');
                                        }
                                        prompt.push('@');
                                        prompt.push_str(&tmp.0.to_string_lossy());
                                        prompt.push('\n');
                                        temp_images.push(tmp);
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "gemini_local: failed to decode base64 image; skipping: {}",
                                            e
                                        );
                                    }
                                }
                            }
                            crate::ImageSource::Url(url) => {
                                tracing::warn!(
                                    "gemini_local: URL image source unsupported (would need pre-download); skipping: {}",
                                    url
                                );
                            }
                        }
                    }
                    crate::ChatContent::ToolResult { tool_use_id, .. } => {
                        tracing::debug!(
                            "gemini_local: dropping tool_result block (CLI handles its own internal tools); tool_use_id={}",
                            tool_use_id
                        );
                    }
                    crate::ChatContent::ToolUse { name, .. } => {
                        tracing::debug!(
                            "gemini_local: dropping tool_use block (CLI handles its own internal tools); name={}",
                            name
                        );
                    }
                }
            }
            if has_text {
                prompt.push_str("\n\n");
            }
        }

        fixed_args.push("--prompt".to_string());
        fixed_args.push(prompt);
        (fixed_args, temp_images)
    }

    fn build_args_with_options(
        model: &str,
        sandbox: bool,
        resume_session: Option<&str>,
        system_prompt: &str,
        messages: &[ChatMessage],
        options: &crate::LlmRequestOptions,
    ) -> (Vec<String>, Vec<crate::cli_utils::TempFile>) {
        // build_args_inner appends `--prompt <prompt>` as the last two
        // elements. We need to: (a) inject `@<abs_path>` lines into the prompt
        // body for caller-supplied image_paths, (b) add `--include-directories
        // <path>` for working_dir BEFORE the `--prompt`/prompt-body pair so
        // the flag stays adjacent to its value.
        let (base, temp_images) =
            Self::build_args_inner(model, sandbox, resume_session, system_prompt, messages);
        assert!(
            base.len() >= 2 && base[base.len() - 2] == "--prompt",
            "build_args_inner invariant violated: expected [..., \"--prompt\", <body>] tail, got {base:?}",
        );
        let prompt_body = base[base.len() - 1].clone();
        let mut args: Vec<String> = base[..base.len() - 2].to_vec();

        if let Some(dir) = options.working_dir.as_ref() {
            args.push("--include-directories".to_string());
            args.push(dir.to_string_lossy().to_string());
        }

        // Inject caller-supplied image paths into the prompt body (same
        // `@<path>` mechanism used for decoded-from-message images above).
        let mut augmented_prompt = prompt_body;
        if !options.image_paths.is_empty() {
            if !augmented_prompt.ends_with('\n') {
                augmented_prompt.push('\n');
            }
            for img in &options.image_paths {
                augmented_prompt.push('@');
                augmented_prompt.push_str(&img.to_string_lossy());
                augmented_prompt.push('\n');
            }
        }

        args.push("--prompt".to_string());
        args.push(augmented_prompt);
        (args, temp_images)
    }

    fn build_args(
        model: &str,
        sandbox: bool,
        resume_session: Option<&str>,
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> Vec<String> {
        let (args, _temp_images) =
            Self::build_args_inner(model, sandbox, resume_session, system_prompt, messages);
        args
    }
}

pub fn make(
    _api_key: &str,
    model: &str,
    _base_url: &str,
) -> Result<Box<dyn LlmProviderDyn>, LlmError> {
    // gemini_local does not require an API key — it uses the local gemini CLI
    // which authenticates via the user's Google account.
    let model = if model.is_empty() {
        DEFAULT_MODEL.to_string()
    } else {
        model.to_string()
    };
    Ok(Box::new(GeminiLocalProvider::new(model)))
}

/// List available models via the Google Gemini API.
///
/// The Gemini CLI itself does not have a model listing command, so this
/// delegates to the Google Gemini API. An API key is required (same as the
/// `gemini` provider). If the API key is not available, returns an empty list.
pub async fn list_models() -> Result<Vec<crate::ListModel>, LlmError> {
    // gemini_local does not require an API key for chat (uses CLI's Google
    // account auth), but model listing requires a Google API key.
    // Check if GEMINI_API_KEY env var is set.
    let api_key = std::env::var("GEMINI_API_KEY").ok();
    match api_key {
        Some(key) => crate::gemini::list_models(&key).await,
        None => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialize tests that mutate `GEMINI_CLI_PATH` so their env-var sets/unsets
    /// don't race when cargo-test runs them in parallel threads. Uses
    /// `tokio::sync::Mutex` so it is safe to hold the guard across `.await`
    /// while the stream is consumed (`std::sync::Mutex` would trip
    /// clippy::await_holding_lock).
    static GEMINI_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[test]
    fn make_with_empty_model_returns_default() {
        let result = make("", "", "");
        assert!(result.is_ok());
    }

    #[test]
    fn make_with_model_returns_provider() {
        let result = make("", "gemini-2.5-pro", "");
        assert!(result.is_ok());
    }

    #[test]
    fn build_args_appends_image_paths_to_prompt() {
        let opts = crate::LlmRequestOptions {
            image_paths: vec![
                std::path::PathBuf::from("/tmp/a.png"),
                std::path::PathBuf::from("/tmp/b.png"),
            ],
            ..Default::default()
        };
        let messages = vec![crate::ChatMessage::user("hi")];
        let (args, _) = GeminiLocalProvider::build_args_with_options(
            "auto", false, None, "sys", &messages, &opts,
        );
        let prompt_idx = args
            .iter()
            .position(|a| a == "--prompt")
            .expect("--prompt present");
        let prompt = &args[prompt_idx + 1];
        assert!(
            prompt.contains("@/tmp/a.png"),
            "prompt should contain @/tmp/a.png: {prompt}"
        );
        assert!(
            prompt.contains("@/tmp/b.png"),
            "prompt should contain @/tmp/b.png: {prompt}"
        );
    }

    #[test]
    fn build_args_includes_include_directories_when_working_dir_set() {
        let opts = crate::LlmRequestOptions {
            working_dir: Some(std::path::PathBuf::from("/tmp/work")),
            ..Default::default()
        };
        let messages = vec![crate::ChatMessage::user("hi")];
        let (args, _) = GeminiLocalProvider::build_args_with_options(
            "auto", false, None, "sys", &messages, &opts,
        );
        let idx = args
            .iter()
            .position(|a| a == "--include-directories")
            .expect("--include-directories present");
        assert_eq!(args[idx + 1], "/tmp/work");

        // Regression guard: `--prompt` must be immediately followed by the
        // prompt body (not `--include-directories`). A previous revision
        // inserted --include-directories between --prompt and its value.
        let prompt_idx = args
            .iter()
            .position(|a| a == "--prompt")
            .expect("--prompt present");
        assert_eq!(
            prompt_idx,
            args.len() - 2,
            "--prompt must be second-to-last: {args:?}"
        );
        assert!(
            args[prompt_idx + 1].contains("[user]"),
            "--prompt must be followed by the prompt body, got: {}",
            args[prompt_idx + 1]
        );
    }

    #[test]
    fn build_args_without_options_matches_build_args() {
        let opts = crate::LlmRequestOptions::default();
        let messages = vec![crate::ChatMessage::user("hi")];
        let (with_opts, _) = GeminiLocalProvider::build_args_with_options(
            "auto", false, None, "sys", &messages, &opts,
        );
        let without = GeminiLocalProvider::build_args("auto", false, None, "sys", &messages);
        assert_eq!(with_opts, without, "default options should be a no-op");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn gemini_stream_emits_deltas_then_end() {
        use crate::{LlmProviderDyn, LlmRequestOptions, LlmStreamEvent};
        use futures::StreamExt;
        use serde_json::Value;

        let _guard = GEMINI_ENV_LOCK.lock().await;

        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/fake_gemini.sh");
        assert!(fixture.exists(), "fixture missing: {}", fixture.display());

        // UNSAFETY RATIONALE: `set_var` is not thread-safe; serialized by GEMINI_ENV_LOCK.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("GEMINI_CLI_PATH", fixture.to_str().unwrap());
        }

        let provider = super::GeminiLocalProvider::new("gemini-1.5-flash".to_string());
        let opts = LlmRequestOptions::default();
        let messages = vec![crate::ChatMessage::user("irrelevant")];
        let mut stream = provider.chat_stream_with_options_dyn("", &messages, &opts, &Value::Null);

        let mut deltas: Vec<String> = Vec::new();
        let mut end_total: Option<String> = None;
        while let Some(event) = stream.next().await {
            match event {
                LlmStreamEvent::Delta { text } => deltas.push(text),
                LlmStreamEvent::End { total } => end_total = Some(total.rendered_text),
                LlmStreamEvent::Error { message } => panic!("unexpected error: {message}"),
                _ => {}
            }
        }

        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("GEMINI_CLI_PATH");
        }

        assert!(
            deltas.len() >= 2,
            "expected >=2 deltas, got {}: {:?}",
            deltas.len(),
            deltas
        );
        let concat: String = deltas.concat();
        assert_eq!(
            concat,
            end_total.as_ref().expect("End event missing").as_str(),
            "concat of Delta text must equal End.total.rendered_text",
        );
        assert!(
            concat.contains("Hello"),
            "fixture should produce 'Hello' in output, got: {concat:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn timeout_aborts_blocking_call() {
        use crate::{LlmError, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use std::time::Duration;

        let _guard = GEMINI_ENV_LOCK.lock().await;

        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/fake_gemini_slow.sh");
        assert!(fixture.exists(), "fixture missing: {}", fixture.display());

        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("GEMINI_CLI_PATH", fixture.to_str().unwrap());
        }

        let provider = GeminiLocalProvider::new("auto".to_string());
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
            std::env::remove_var("GEMINI_CLI_PATH");
        }

        assert!(
            matches!(
                res,
                Err(LlmError::Network { ref provider, ref msg })
                    if provider == "gemini_local" && msg.contains("timeout")
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

        let _guard = GEMINI_ENV_LOCK.lock().await;

        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/fake_gemini_slow.sh");
        assert!(fixture.exists(), "fixture missing: {}", fixture.display());

        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("GEMINI_CLI_PATH", fixture.to_str().unwrap());
        }

        let provider = GeminiLocalProvider::new("auto".to_string());
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
            std::env::remove_var("GEMINI_CLI_PATH");
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
    #[ignore = "requires gemini CLI installed locally"]
    async fn full_chat_via_cli_returns_text_and_model() {
        let provider = GeminiLocalProvider::new("auto".to_string());
        let msgs = vec![ChatMessage::user(
            "Say 'test-ok' in exactly two words. Nothing else.",
        )];
        let resp = provider
            .chat_dyn(
                "You are a test assistant. Respond briefly.",
                &msgs,
                &serde_json::json!({}),
            )
            .await
            .expect("gemini CLI should respond");

        println!(
            "model={}, cost={:.6}, text=[{}]",
            resp.model,
            resp.estimated_cost_usd,
            &resp.rendered_text[..resp.rendered_text.len().min(200)]
        );

        let text_lower = resp.rendered_text.to_lowercase();
        assert!(
            text_lower.contains("test") || text_lower.contains("ok"),
            "content should be parsed from CLI output: [{}]",
            resp.rendered_text
        );
        assert!(
            !resp.model.is_empty(),
            "model should be extracted from CLI 'init' event"
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
    fn multipart_image_base64_block_injects_at_path_into_prompt() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD.encode(TINY_PNG);
        let img_block = crate::ChatContent::Image {
            source: crate::ImageSource::Base64(b64),
            mime_type: "image/png".to_string(),
        };
        let messages = vec![crate::ChatMessage::user_with_image("describe", img_block)];
        let opts = crate::LlmRequestOptions::default();
        let (args, temp_images) =
            GeminiLocalProvider::build_args_with_options("auto", false, None, "", &messages, &opts);

        // Prompt body must contain an @<path> token pointing to the temp file.
        let prompt_idx = args
            .iter()
            .position(|a| a == "--prompt")
            .expect("--prompt present");
        let prompt = &args[prompt_idx + 1];
        assert!(
            prompt.contains('@'),
            "prompt should contain @<path> for image: {prompt}"
        );

        // Confirm the referenced path currently exists (TempFile still alive).
        let at_line = prompt
            .lines()
            .find(|l| l.starts_with('@'))
            .expect("@<path> line in prompt");
        let img_path = std::path::PathBuf::from(at_line.trim_start_matches('@'));
        assert!(
            img_path.exists(),
            "temp image file should exist: {}",
            img_path.display()
        );

        // Prompt must remain last arg.
        assert_eq!(
            args.iter().rposition(|a| a == "--prompt").unwrap(),
            args.len() - 2,
            "--prompt must be second-to-last: {args:?}"
        );

        // Dropping temp_images must remove the file.
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
            GeminiLocalProvider::build_args_with_options("auto", false, None, "", &messages, &opts);

        let prompt_idx = args
            .iter()
            .position(|a| a == "--prompt")
            .expect("--prompt present");
        let prompt = &args[prompt_idx + 1];
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
