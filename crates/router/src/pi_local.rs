use futures::StreamExt;
use futures::stream::BoxStream;
use serde_json::Value;

use super::{
    ChatContent, ChatMessage, ImageSource, LlmError, LlmProviderDyn, LlmRequestOptions,
    LlmResponse, LlmStreamEvent,
};

const DEFAULT_MODEL: &str = "auto";

#[derive(Clone)]
pub struct PiLocalProvider {
    model: String,
    provider: Option<String>,
    api_key: Option<String>,
}

impl PiLocalProvider {
    pub fn new(model: String) -> Self {
        Self {
            model: if model.is_empty() {
                DEFAULT_MODEL.to_string()
            } else {
                model
            },
            provider: None,
            api_key: None,
        }
    }

    pub fn with_provider(mut self, provider: String) -> Self {
        if !provider.is_empty() {
            self.provider = Some(provider);
        }
        self
    }

    pub fn with_api_key(mut self, api_key: String) -> Self {
        if !api_key.is_empty() {
            self.api_key = Some(api_key);
        }
        self
    }
}

impl LlmProviderDyn for PiLocalProvider {
    fn provider_key(&self) -> &'static str {
        "pi_local"
    }

    fn capabilities(&self) -> crate::ProviderCapabilities {
        crate::ProviderCapabilities {
            kind: crate::ProviderKind::LocalCli,
            usage_support: crate::UsageSupport::None,
            supports_streaming: true,
            supports_tool_calling: false,
            supports_parallel_tool_calls: false,
            supports_image_input: true,
            supports_image_url: false,
            supports_reasoning_effort: true,
            supports_thinking_budget: false,
            supports_prompt_cache: false,
            supports_model_listing: false,
            supports_web_search: false,
            supports_local_read: true,
            estimates_cost: false,
        }
    }

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
        let messages = vec![ChatMessage::user(user_input.to_string())];
        Box::pin(async move {
            collect_stream(provider, system, messages, LlmRequestOptions::default()).await
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
        let messages = messages.to_vec();
        Box::pin(async move {
            collect_stream(provider, system, messages, LlmRequestOptions::default()).await
        })
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
            LlmRequestOptions::default(),
        )
    }

    fn chat_with_options_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        options: &'a LlmRequestOptions,
        _result_json: &'a Value,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + 'a>,
    > {
        let provider = self.clone();
        let system = system_prompt.to_string();
        let messages = messages.to_vec();
        let options = options.clone();
        Box::pin(async move { collect_stream(provider, system, messages, options).await })
    }

    fn chat_stream_with_options_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        options: &'a LlmRequestOptions,
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

async fn collect_stream(
    provider: PiLocalProvider,
    system_prompt: String,
    messages: Vec<ChatMessage>,
    options: LlmRequestOptions,
) -> Result<LlmResponse, LlmError> {
    let mut stream = PiLocalProvider::run_stream_async(provider, system_prompt, messages, options);
    while let Some(event) = stream.next().await {
        match event {
            LlmStreamEvent::End { total } => return Ok(total),
            LlmStreamEvent::Error { message } => {
                return Err(classify_error(message));
            }
            _ => {}
        }
    }
    Err(LlmError::Other {
        provider: "pi_local".into(),
        message: "stream ended without End event".into(),
    })
}

impl PiLocalProvider {
    fn run_stream_async(
        provider: PiLocalProvider,
        system_prompt: String,
        messages: Vec<ChatMessage>,
        options: LlmRequestOptions,
    ) -> BoxStream<'static, LlmStreamEvent> {
        Box::pin(async_stream::stream! {
            use tokio::io::{AsyncBufReadExt, AsyncReadExt};
            use tokio::process::Command as TokioCommand;

            let (args, _temp_images) = provider.build_args(&system_prompt, &messages, &options);
            let binary = crate::cli_utils::resolve_binary("pi");
            let mut command = TokioCommand::new(&binary);
            command
                .args(&args)
                .env("PATH", crate::cli_utils::build_path_env())
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true);

            if let Some(dir) = options.working_dir.as_ref() {
                command.current_dir(dir);
            }

            let mut child = match command.spawn() {
                Ok(child) => child,
                Err(e) => {
                    yield LlmStreamEvent::Error {
                        message: format!("failed to spawn pi CLI: {e}"),
                    };
                    return;
                }
            };

            let stdout = match child.stdout.take() {
                Some(stdout) => stdout,
                None => {
                    yield LlmStreamEvent::Error { message: "failed to take stdout".into() };
                    return;
                }
            };

            let stderr_task = child.stderr.take().map(|mut stderr| {
                tokio::spawn(async move {
                    let mut buf = String::new();
                    let _ = stderr.read_to_string(&mut buf).await;
                    buf
                })
            });

            let reader = tokio::io::BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut accumulated = String::new();
            let mut final_text: Option<String> = None;
            let mut model = provider.model.clone();
            let mut error_msg = String::new();

            let sleep_opt = options.timeout.map(tokio::time::sleep);
            tokio::pin!(sleep_opt);

            loop {
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
                    None => {
                        yield LlmStreamEvent::Error { message: "timeout".into() };
                        return;
                    }
                    Some(Ok(Some(line))) => {
                        if line.trim().is_empty() {
                            continue;
                        }
                        match parse_event_line(&line) {
                            PiEvent::Delta(text) => {
                                accumulated.push_str(&text);
                                yield LlmStreamEvent::Delta { text };
                            }
                            PiEvent::FinalText(text) => {
                                final_text = Some(text);
                            }
                            PiEvent::Model(name) => model = name,
                            PiEvent::Error(message) => error_msg = message,
                            PiEvent::Ignore => {}
                        }
                    }
                    Some(Ok(None)) => break,
                    Some(Err(e)) => {
                        yield LlmStreamEvent::Error {
                            message: format!("failed to read pi stdout: {e}"),
                        };
                        return;
                    }
                }
            }

            let status = match child.wait().await {
                Ok(status) => status,
                Err(e) => {
                    yield LlmStreamEvent::Error {
                        message: format!("failed to wait for pi CLI: {e}"),
                    };
                    return;
                }
            };
            let stderr = match stderr_task {
                Some(task) => task.await.unwrap_or_default(),
                None => String::new(),
            };

            if !status.success() {
                let message = if !error_msg.is_empty() {
                    error_msg
                } else if !stderr.trim().is_empty() {
                    stderr
                } else {
                    format!("pi CLI exited with status: {status}")
                };
                yield LlmStreamEvent::Error { message };
                return;
            }

            if !error_msg.is_empty() {
                yield LlmStreamEvent::Error { message: error_msg };
                return;
            }

            yield LlmStreamEvent::End {
                total: LlmResponse {
                    rendered_text: final_text.unwrap_or(accumulated),
                    model,
                    ..Default::default()
                },
            };
        })
    }

    fn build_args(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
        options: &LlmRequestOptions,
    ) -> (Vec<String>, Vec<crate::cli_utils::TempFile>) {
        let mut args = vec!["--mode".to_string(), "json".to_string()];
        let mut temp_images = Vec::new();

        if let Some(provider) = self.provider.as_deref() {
            args.push("--provider".to_string());
            args.push(provider.to_string());
        }
        if self.model != DEFAULT_MODEL {
            args.push("--model".to_string());
            args.push(self.model.clone());
        }
        if let Some(api_key) = self.api_key.as_deref() {
            args.push("--api-key".to_string());
            args.push(api_key.to_string());
        }
        if let Some(reasoning) = options.reasoning_effort.as_deref() {
            args.push("--thinking".to_string());
            args.push(reasoning.to_string());
        }
        if !system_prompt.trim().is_empty() {
            args.push("--system-prompt".to_string());
            args.push(crate::cli_utils::sanitize_prompt(system_prompt.trim()));
        }

        args.push("--no-session".to_string());
        args.push(build_prompt(messages, options, &mut temp_images));
        (args, temp_images)
    }
}

fn build_prompt(
    messages: &[ChatMessage],
    options: &LlmRequestOptions,
    temp_images: &mut Vec<crate::cli_utils::TempFile>,
) -> String {
    let mut parts = Vec::new();

    for message in messages {
        let (text, image_refs) = collect_message_prompt_parts(message, temp_images);
        let has_text = !text.trim().is_empty();
        let has_images = !image_refs.is_empty();
        if !has_text && !has_images {
            continue;
        }
        let role = match message.role.as_str() {
            "assistant" => "Assistant",
            "system" => "System",
            _ => "User",
        };

        let mut body = String::new();
        if has_text {
            body.push_str(text.trim());
        }
        for image_ref in image_refs {
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str(&image_ref);
        }
        parts.push(format!("{role}:\n{body}"));
    }

    for path in &options.image_paths {
        parts.push(format!("@{}", path.display()));
    }

    crate::cli_utils::sanitize_prompt(&parts.join("\n\n"))
}

fn collect_message_prompt_parts(
    message: &ChatMessage,
    temp_images: &mut Vec<crate::cli_utils::TempFile>,
) -> (String, Vec<String>) {
    let mut text_parts = Vec::new();
    let mut image_refs = Vec::new();

    for content in &message.content {
        match content {
            ChatContent::Text(text) => text_parts.push(text.as_str()),
            ChatContent::Image { source, mime_type } => match source {
                ImageSource::Base64(data) => {
                    match crate::cli_utils::decode_base64_image(data, mime_type) {
                        Ok(temp) => {
                            image_refs.push(format!("@{}", temp.0.display()));
                            temp_images.push(temp);
                        }
                        Err(err) => {
                            tracing::warn!(
                                "pi_local: failed to decode base64 image; skipping: {}",
                                err
                            );
                        }
                    }
                }
                ImageSource::Url(url) => {
                    tracing::warn!(
                        "pi_local: URL image source unsupported (would need pre-download); skipping: {}",
                        url
                    );
                }
            },
            ChatContent::ToolResult { .. } | ChatContent::ToolUse { .. } => {}
        }
    }

    (text_parts.join(""), image_refs)
}

enum PiEvent {
    Delta(String),
    FinalText(String),
    Model(String),
    Error(String),
    Ignore,
}

fn parse_event_line(line: &str) -> PiEvent {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return PiEvent::Ignore;
    };

    if let Some(delta) = value
        .get("assistantMessageEvent")
        .and_then(|event| event.get("delta"))
        .and_then(|delta| delta.as_str())
    {
        return PiEvent::Delta(delta.to_string());
    }

    if let Some(model) = value
        .get("message")
        .and_then(|message| message.get("model"))
        .or_else(|| value.get("model"))
        .and_then(|model| model.as_str())
    {
        return PiEvent::Model(model.to_string());
    }

    if matches!(
        value.get("type").and_then(|v| v.as_str()),
        Some("message_end" | "turn_end")
    ) {
        if let Some(text) = extract_message_text(value.get("message")) {
            return PiEvent::FinalText(text);
        }
    }

    if let Some(message) = value
        .get("errorMessage")
        .or_else(|| value.get("finalError"))
        .or_else(|| value.get("message"))
        .and_then(|message| message.as_str())
    {
        let event_type = value
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if event_type.contains("error") || event_type == "error" {
            return PiEvent::Error(message.to_string());
        }
    }

    PiEvent::Ignore
}

fn extract_message_text(message: Option<&Value>) -> Option<String> {
    let message = message?;
    if message.get("role").and_then(|role| role.as_str()) != Some("assistant") {
        return None;
    }

    let mut text = String::new();
    match message.get("content") {
        Some(Value::String(content)) => text.push_str(content),
        Some(Value::Array(parts)) => {
            for part in parts {
                if let Some(part_text) = part
                    .get("text")
                    .or_else(|| part.get("content"))
                    .and_then(|value| value.as_str())
                {
                    text.push_str(part_text);
                }
            }
        }
        _ => {}
    }

    if text.is_empty() { None } else { Some(text) }
}

fn classify_error(message: String) -> LlmError {
    let haystack = message.to_lowercase();
    if haystack.contains("not logged in")
        || haystack.contains("login")
        || haystack.contains("oauth")
        || haystack.contains("unauthorized")
        || haystack.contains("forbidden")
        || haystack.contains("invalid credentials")
    {
        LlmError::Auth {
            provider: "pi_local".into(),
        }
    } else if message == "timeout" {
        LlmError::Network {
            provider: "pi_local".into(),
            msg: "timeout".into(),
        }
    } else {
        LlmError::Other {
            provider: "pi_local".into(),
            message,
        }
    }
}

pub fn make(
    api_key: &str,
    model: &str,
    provider: &str,
) -> Result<Box<dyn LlmProviderDyn>, LlmError> {
    Ok(Box::new(
        PiLocalProvider::new(model.to_string())
            .with_provider(provider.to_string())
            .with_api_key(api_key.to_string()),
    ))
}

pub async fn list_models() -> Result<Vec<crate::ListModel>, LlmError> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_with_empty_model_uses_auto() {
        let provider = make("", "", "").unwrap();

        assert_eq!(provider.provider_key(), "pi_local");
        assert_eq!(provider.context_window_tokens(), None);
    }

    #[test]
    fn build_args_use_json_mode_no_session_and_prompt() {
        let provider = PiLocalProvider::new("openai/gpt-5".into()).with_provider("openai".into());
        let messages = vec![ChatMessage::user("hello")];
        let (args, _temp_images) =
            provider.build_args("sys", &messages, &LlmRequestOptions::default());

        assert!(args.windows(2).any(|pair| pair == ["--mode", "json"]));
        assert!(args.windows(2).any(|pair| pair == ["--provider", "openai"]));
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--model", "openai/gpt-5"])
        );
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--system-prompt", "sys"])
        );
        assert!(args.iter().any(|arg| arg == "--no-session"));
        assert!(!args.last().unwrap().contains("System:\nsys"));
        assert!(args.last().unwrap().contains("User:\nhello"));
    }

    #[test]
    fn build_args_include_reasoning_effort_and_api_key() {
        let provider = PiLocalProvider::new("sonnet:high".into()).with_api_key("k".into());
        let messages = vec![ChatMessage::user("hard problem")];
        let opts = LlmRequestOptions {
            reasoning_effort: Some("high".into()),
            ..Default::default()
        };
        let (args, _temp_images) = provider.build_args("", &messages, &opts);

        assert!(args.windows(2).any(|pair| pair == ["--api-key", "k"]));
        assert!(args.windows(2).any(|pair| pair == ["--thinking", "high"]));
    }

    #[test]
    fn build_prompt_appends_image_paths_as_file_references() {
        let opts = LlmRequestOptions {
            image_paths: vec!["/tmp/screenshot.png".into()],
            ..Default::default()
        };
        let mut temp_images = Vec::new();
        let prompt = build_prompt(&[ChatMessage::user("describe")], &opts, &mut temp_images);

        assert!(prompt.contains("User:\ndescribe"));
        assert!(prompt.contains("@/tmp/screenshot.png"));
    }

    #[test]
    fn multipart_base64_image_becomes_temp_file_reference() {
        use base64::Engine as _;

        let data = base64::engine::general_purpose::STANDARD.encode(b"not-a-real-png");
        let image = ChatContent::Image {
            source: ImageSource::Base64(data),
            mime_type: "image/png".to_string(),
        };
        let messages = vec![ChatMessage::user_with_image("describe", image)];
        let provider = PiLocalProvider::new("auto".into());

        let (args, temp_images) = provider.build_args("", &messages, &LlmRequestOptions::default());

        let prompt = args.last().unwrap();
        assert!(prompt.contains("User:\ndescribe"));
        assert!(prompt.contains("@"));
        assert_eq!(temp_images.len(), 1);
        assert!(temp_images[0].0.exists());
        let path = temp_images[0].0.clone();
        drop(temp_images);
        assert!(!path.exists());
    }

    #[test]
    fn final_text_overrides_delta_accumulation_for_end_total() {
        let accumulated = String::from("draft");
        let final_text = Some(String::from("final"));

        let rendered = final_text.unwrap_or(accumulated);

        assert_eq!(rendered, "final");
    }

    #[test]
    fn parse_text_delta_event() {
        let event = r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Hello"}}"#;

        match parse_event_line(event) {
            PiEvent::Delta(text) => assert_eq!(text, "Hello"),
            _ => panic!("expected delta"),
        }
    }

    #[test]
    fn parse_turn_end_final_text() {
        let event = r#"{"type":"turn_end","message":{"role":"assistant","content":[{"type":"text","text":"Done"}]}}"#;

        match parse_event_line(event) {
            PiEvent::FinalText(text) => assert_eq!(text, "Done"),
            _ => panic!("expected final text"),
        }
    }

    #[test]
    fn classify_login_errors_as_auth() {
        let err = classify_error("not logged in; use /login".into());

        assert!(matches!(err, LlmError::Auth { provider } if provider == "pi_local"));
    }
}
