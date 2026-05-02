use reqwest::Client;
use serde_json::{Value, json};
use std::time::Duration;

use super::{
    ChatContent, ChatMessage, ImageSource, LlmError, LlmProviderDyn, LlmRequestOptions,
    LlmResponse, ToolCall, ToolChoice,
};

const CODEX_API_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
const REQUEST_TIMEOUT_SECS: u64 = 60;

pub struct CodexProvider {
    client: Client,
    token: String,
    model: String,
    base_url: String,
}

impl CodexProvider {
    pub fn new(token: String, model: String) -> Self {
        Self::new_with_base_url(token, model, CODEX_API_URL.to_string())
    }

    /// For testing only — allows pointing the provider at a mock HTTP server.
    /// Production callers should use [`Self::new`], which defaults to
    /// `CODEX_API_URL` (`https://chatgpt.com/backend-api/codex/responses`).
    pub fn new_with_base_url(token: String, model: String, base_url: String) -> Self {
        let model = if model.is_empty() {
            "gpt-5.4".to_string()
        } else {
            model
        };
        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .expect("Failed to build HTTP client");
        Self {
            client,
            token,
            model,
            base_url,
        }
    }

    /// Build the Responses API `input` array from `messages`.
    ///
    /// Each [`ChatMessage`] is iterated block-by-block:
    /// - [`ChatContent::Text`] blocks contribute a `{type: "input_text"|"output_text", text}`
    ///   sub-item to the message's `content` array.
    /// - [`ChatContent::Image`] blocks contribute a `{type: "input_image", image_url: <string>}`
    ///   sub-item. Responses API quirk: `image_url` is a plain string (data URL or HTTPS URL),
    ///   not an object like Chat Completions.
    ///   `ImageSource::Base64` → `"data:<mime>;base64,<data>"`;
    ///   `ImageSource::Url` → the URL string directly.
    /// - [`ChatContent::ToolResult`] blocks produce a separate
    ///   `{type: "function_call_output", call_id, output}` item in the `input`
    ///   array (Responses API wire format — not a `role: "tool"` message like
    ///   Chat Completions).
    fn build_input(messages: &[ChatMessage]) -> Vec<Value> {
        let mut out: Vec<Value> = Vec::new();
        for m in messages {
            let text_content_type = if m.role == "assistant" {
                "output_text"
            } else {
                "input_text"
            };

            let mut content_arr: Vec<Value> = Vec::new();

            for block in &m.content {
                match block {
                    ChatContent::Text(t) => {
                        if !t.is_empty() {
                            content_arr.push(json!({
                                "type": text_content_type,
                                "text": t,
                            }));
                        }
                    }
                    ChatContent::Image { source, mime_type } => {
                        // Responses API: image_url is a STRING (data URL or HTTPS URL),
                        // not an object like Chat Completions {url: "..."}.
                        let image_url = match source {
                            ImageSource::Base64(data) => {
                                format!("data:{mime_type};base64,{data}")
                            }
                            ImageSource::Url(url) => url.clone(),
                        };
                        content_arr.push(json!({
                            "type": "input_image",
                            "image_url": image_url,
                        }));
                    }
                    ChatContent::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        let output = if *is_error {
                            format!("Error: {content}")
                        } else {
                            content.clone()
                        };
                        // Responses API: tool results are top-level input items,
                        // NOT messages with role:"tool" (that is Chat Completions).
                        out.push(json!({
                            "type": "function_call_output",
                            "call_id": tool_use_id,
                            "output": output,
                        }));
                    }
                }
            }

            if !content_arr.is_empty() {
                out.push(json!({
                    "type": "message",
                    "role": m.role,
                    "content": content_arr,
                }));
            }
        }
        out
    }

    /// Parse the Responses API `output` array into `(rendered_text, thinking_text, tool_calls)`.
    ///
    /// - Items with `type: "reasoning"` contribute to `thinking_text`.
    ///   Two shapes are supported:
    ///   1. `reasoning_content` string field (some API versions).
    ///   2. `summary[].text` array (o-series standard shape) — texts joined with newlines.
    /// - Items with `type: "message"` contribute their `content[].text` (from
    ///   `output_text` sub-items) to `rendered_text`.
    /// - Items with `type: "function_call"` are converted to [`ToolCall`].
    fn parse_output(output: &[Value]) -> (String, Option<String>, Vec<ToolCall>) {
        let mut rendered_text = String::new();
        let mut thinking_acc = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();

        for item in output {
            match item.get("type").and_then(|t| t.as_str()) {
                Some("reasoning") => {
                    // Shape 1: reasoning_content as a direct string field.
                    if let Some(rc) = item.get("reasoning_content").and_then(|v| v.as_str()) {
                        if !thinking_acc.is_empty() {
                            thinking_acc.push('\n');
                        }
                        thinking_acc.push_str(rc);
                    } else if let Some(summary) = item.get("summary").and_then(|s| s.as_array()) {
                        // Shape 2: summary[].text array (o-series standard).
                        for entry in summary {
                            if let Some(text) = entry.get("text").and_then(|t| t.as_str()) {
                                if !thinking_acc.is_empty() {
                                    thinking_acc.push('\n');
                                }
                                thinking_acc.push_str(text);
                            }
                        }
                    }
                }
                Some("message") => {
                    if let Some(content) = item.get("content").and_then(|c| c.as_array()) {
                        for block in content {
                            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                rendered_text.push_str(text);
                            }
                        }
                    }
                }
                Some("function_call") => {
                    // Responses API: call_id (not id), name at top level (not nested under function).
                    if let (Some(id), Some(name), Some(args_str)) = (
                        item.get("call_id").and_then(|v| v.as_str()),
                        item.get("name").and_then(|v| v.as_str()),
                        item.get("arguments").and_then(|v| v.as_str()),
                    ) {
                        if let Ok(arguments) = serde_json::from_str::<Value>(args_str) {
                            tool_calls.push(ToolCall {
                                id: id.to_string(),
                                name: name.to_string(),
                                arguments,
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        let thinking_text = if thinking_acc.is_empty() {
            None
        } else {
            Some(thinking_acc)
        };
        (rendered_text, thinking_text, tool_calls)
    }
}

impl LlmProviderDyn for CodexProvider {
    fn render_dyn(
        &self,
        system_prompt: &str,
        user_input: &str,
        _result_json: &Value,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + '_>,
    > {
        let msgs = vec![ChatMessage::user(user_input)];
        let system = system_prompt.to_string();
        Box::pin(async move { self.chat_impl(&system, &msgs).await })
    }

    fn chat_dyn(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
        _result_json: &Value,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + '_>,
    > {
        let system = system_prompt.to_string();
        let msgs = messages.to_vec();
        Box::pin(async move { self.chat_impl(&system, &msgs).await })
    }

    fn chat_with_options_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        options: &'a crate::LlmRequestOptions,
        _result_json: &'a Value,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + 'a>,
    > {
        let timeout = options.timeout;
        let opts = options.clone();
        let system = system_prompt.to_string();
        let msgs = messages.to_vec();
        Box::pin(async move {
            self.chat_impl_with_timeout(&system, &msgs, timeout, &opts)
                .await
        })
    }

    fn chat_stream_with_options_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        options: &'a crate::LlmRequestOptions,
        result_json: &'a Value,
    ) -> futures::stream::BoxStream<'a, crate::LlmStreamEvent> {
        use crate::LlmStreamEvent;
        let fut = self.chat_with_options_dyn(system_prompt, messages, options, result_json);
        Box::pin(async_stream::stream! {
            match fut.await {
                Ok(resp) => {
                    if let Some(ref thinking) = resp.thinking_text {
                        yield LlmStreamEvent::Thinking { text: thinking.clone() };
                    }
                    if !resp.rendered_text.is_empty() {
                        yield LlmStreamEvent::Delta { text: resp.rendered_text.clone() };
                    }
                    // Emit Usage before End so callers can display live cost after
                    // the blocking Responses API call completes.
                    if let Some(out) = resp.output_tokens {
                        yield LlmStreamEvent::Usage {
                            input_tokens: resp.input_tokens,
                            output_tokens: out,
                            estimated_cost_usd: resp.estimated_cost_usd,
                        };
                    }
                    yield LlmStreamEvent::End { total: resp };
                }
                Err(e) => yield LlmStreamEvent::Error { message: e.to_string() },
            }
        })
    }
}

impl CodexProvider {
    async fn chat_impl(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> Result<LlmResponse, LlmError> {
        self.chat_impl_with_timeout(system_prompt, messages, None, &Default::default())
            .await
    }

    async fn chat_impl_with_timeout(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
        request_timeout: Option<Duration>,
        options: &LlmRequestOptions,
    ) -> Result<LlmResponse, LlmError> {
        let input = Self::build_input(messages);

        let mut body = json!({
            "model": self.model,
            "instructions": system_prompt,
            "input": input,
            "stream": true,
            "store": false,
        });

        if let Some(t) = options.temperature {
            body["temperature"] = json!(t);
        }
        if let Some(n) = options.max_tokens {
            body["max_tokens"] = json!(n);
        }
        if let Some(p) = options.top_p {
            body["top_p"] = json!(p);
        }
        if !options.stop_sequences.is_empty() {
            body["stop"] = json!(options.stop_sequences);
        }

        // Encode tools in Responses API format.
        // NOTE: differs from Chat Completions — `name` is at top level, NOT nested
        // under a `function: {}` wrapper.
        if !options.tools.is_empty() {
            body["tools"] = json!(
                options
                    .tools
                    .iter()
                    .map(|t| json!({
                        "type": "function",
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    }))
                    .collect::<Vec<_>>()
            );
        }

        // Encode tool_choice in Responses API format.
        // NOTE: differs from Chat Completions — `Specific(name)` uses
        // `{type: "function", name: "..."}` (name at top level), NOT
        // `{type: "function", function: {name: "..."}}`.
        if let Some(choice) = &options.tool_choice {
            body["tool_choice"] = match choice {
                ToolChoice::Auto => json!("auto"),
                ToolChoice::Required => json!("required"),
                ToolChoice::None => json!("none"),
                ToolChoice::Specific(name) => {
                    json!({"type": "function", "name": name})
                }
            };
        }

        // Map thinking_budget_tokens → reasoning effort when no explicit effort is set.
        // Explicit reasoning_effort always takes precedence; thinking_budget_tokens alone
        // implies "high" as a safe default for o-series reasoning models.
        let effective_reasoning_effort = options
            .reasoning_effort
            .clone()
            .or_else(|| options.thinking_budget_tokens.map(|_| "high".to_string()));
        if let Some(effort) = effective_reasoning_effort {
            body["reasoning"] = json!({"effort": effort});
        }

        tracing::debug!(
            model = %self.model,
            input_count = messages.len(),
            body = %body,
            "Codex API request"
        );

        let mut req = self
            .client
            .post(&self.base_url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("content-type", "application/json")
            .json(&body);
        if let Some(d) = request_timeout {
            req = req.timeout(d);
        }
        let resp = req.send().await.map_err(|e| {
            if e.is_timeout() {
                LlmError::Network {
                    provider: "codex".into(),
                    msg: "timeout".into(),
                }
            } else {
                LlmError::Network {
                    provider: "codex".into(),
                    msg: e.to_string(),
                }
            }
        })?;

        let status = resp.status();
        if !status.is_success() {
            let retry_after = crate::error::parse_retry_after(&resp);
            let err_body = resp.text().await.unwrap_or_default();
            tracing::error!(
                status = %status,
                error_body = %err_body,
                request_body = %body,
                model = %self.model,
                "Codex API error"
            );
            return Err(match status.as_u16() {
                401 | 403 => LlmError::Auth {
                    provider: "codex".into(),
                },
                429 => LlmError::RateLimit {
                    provider: "codex".into(),
                    retry_after_secs: retry_after,
                },
                s => LlmError::Upstream {
                    provider: "codex".into(),
                    status: s,
                    body: err_body,
                },
            });
        }

        // Parse SSE stream and collect text + tool calls.
        let body_text = resp.text().await.map_err(|e| LlmError::Network {
            provider: "codex".into(),
            msg: format!("failed to read response body: {e}"),
        })?;

        // First, attempt to parse as a non-streaming JSON response (Responses API
        // non-SSE format used in tests). Fall back to SSE line scanning.
        if let Ok(json_val) = serde_json::from_str::<Value>(&body_text) {
            if let Some(output_arr) = json_val.get("output").and_then(|o| o.as_array()) {
                let (rendered_text, thinking_text, tool_calls) = Self::parse_output(output_arr);
                let input_tokens = json_val["usage"]["input_tokens"]
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok());
                let output_tokens = json_val["usage"]["output_tokens"]
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok());
                return Ok(LlmResponse {
                    rendered_text,
                    model: self.model.clone(),
                    estimated_cost_usd: 0.0,
                    input_tokens,
                    output_tokens,
                    tool_calls,
                    thinking_text,
                    ..Default::default()
                });
            }
        }

        // SSE stream path: parse line-by-line.
        let mut result_text = String::new();
        for line in body_text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    break;
                }
                if let Ok(event) = serde_json::from_str::<Value>(data) {
                    // Responses API streaming: response.output_text.delta
                    if event.get("type").and_then(|t| t.as_str())
                        == Some("response.output_text.delta")
                    {
                        if let Some(delta) = event.get("delta").and_then(|d| d.as_str()) {
                            result_text.push_str(delta);
                        }
                    }
                }
            }
        }

        Ok(LlmResponse {
            rendered_text: result_text,
            model: self.model.clone(),
            estimated_cost_usd: 0.0,
            ..Default::default()
        })
    }
}

pub fn make(
    api_key: &str,
    model: &str,
    _base_url: &str,
) -> Result<Box<dyn crate::LlmProviderDyn>, crate::LlmError> {
    if api_key.is_empty() {
        return Err(crate::LlmError::MissingConfig {
            provider: "codex".into(),
            reason: "API key is empty".into(),
        });
    }
    Ok(Box::new(CodexProvider::new(
        api_key.to_string(),
        model.to_string(),
    )))
}

pub async fn list_models(_api_key: &str) -> Result<Vec<crate::ListModel>, LlmError> {
    // No public model listing API for OpenAI Codex
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{self, body_partial_json};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test(flavor = "multi_thread")]
    async fn sampling_params_appear_in_request_body() {
        use crate::{ChatMessage, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;

        let server = MockServer::start().await;
        // Use exact f32-representable values (0.5 = 2^-1, 0.25 = 2^-2) so that
        // f32 serialization matches the f64 literal in the partial-JSON matcher.
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(serde_json::json!({
                "temperature": 0.5,
                "max_tokens": 256,
                "top_p": 0.25,
                "stop": ["END"]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "data: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\ndata: [DONE]\n",
            ))
            .mount(&server)
            .await;

        let provider =
            super::CodexProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let opts = LlmRequestOptions {
            temperature: Some(0.5),
            max_tokens: Some(256),
            top_p: Some(0.25),
            stop_sequences: vec!["END".into()],
            ..Default::default()
        };
        let result = provider
            .chat_with_options_dyn("", &[ChatMessage::user("x")], &opts, &Value::Null)
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn timeout_option_aborts_slow_call() {
        use crate::{ChatMessage, LlmError, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use std::time::Duration;

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_secs(2))
                    .set_body_json(serde_json::json!({"output": []})),
            )
            .mount(&server)
            .await;

        let provider =
            super::CodexProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let messages = vec![ChatMessage::user("x")];
        let opts = LlmRequestOptions {
            timeout: Some(Duration::from_millis(500)),
            ..Default::default()
        };

        let started = std::time::Instant::now();
        let res = provider
            .chat_with_options_dyn("", &messages, &opts, &Value::Null)
            .await;
        let elapsed = started.elapsed();

        assert!(
            matches!(
                res,
                Err(LlmError::Network { ref provider, ref msg, .. })
                    if provider == "codex" && msg.to_lowercase().contains("timeout")
            ),
            "expected codex Network/timeout, got {res:?}"
        );
        assert!(
            elapsed < Duration::from_millis(1500),
            "should abort < 1.5s, took {elapsed:?}"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Task 7: Tool calling tests (Responses API schema)
    // ──────────────────────────────────────────────────────────────────────────

    /// Test A: Non-streaming Responses API `output` array with a `function_call`
    /// item is parsed into `LlmResponse.tool_calls`.
    #[tokio::test(flavor = "multi_thread")]
    async fn tool_call_in_blocking_response_returns_tool_calls() {
        use crate::{LlmProviderDyn, LlmRequestOptions, ToolDef};
        use serde_json::Value;

        let fixture = include_str!("../tests/fixtures/codex_tool_call_response.json");

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(fixture)
                    .insert_header("content-type", "application/json"),
            )
            .mount(&server)
            .await;

        let provider = CodexProvider::new_with_base_url("k".into(), "gpt-5".into(), server.uri());
        let opts = LlmRequestOptions {
            tools: vec![ToolDef {
                name: "get_weather".into(),
                description: "Get weather for a city".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {"city": {"type": "string"}},
                    "required": ["city"]
                }),
            }],
            ..Default::default()
        };

        let resp = provider
            .chat_with_options_dyn(
                "",
                &[ChatMessage::user("What's the weather in Seoul?")],
                &opts,
                &Value::Null,
            )
            .await
            .expect("expected Ok response");

        assert_eq!(resp.tool_calls.len(), 1, "expected 1 tool call");
        let call = &resp.tool_calls[0];
        assert_eq!(call.name, "get_weather");
        assert_eq!(call.id, "call_01");
        assert_eq!(call.arguments["city"], "seoul");
        assert_eq!(resp.rendered_text, "Checking the weather.");
    }

    /// Test B: Tools are serialised in Responses API format — `name` at top level,
    /// NOT nested under `function: {name}` like Chat Completions.
    #[tokio::test(flavor = "multi_thread")]
    async fn tools_appear_in_request_body() {
        use crate::{LlmProviderDyn, LlmRequestOptions, ToolDef};
        use serde_json::Value;

        let server = MockServer::start().await;
        // Responses API: {type: "function", name: "...", description: "..."} — name at top level.
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(serde_json::json!({
                "tools": [{
                    "type": "function",
                    "name": "get_weather",
                    "description": "Get weather for a city"
                }]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "data: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\ndata: [DONE]\n",
            ))
            .mount(&server)
            .await;

        let provider = CodexProvider::new_with_base_url("k".into(), "gpt-5".into(), server.uri());
        let opts = LlmRequestOptions {
            tools: vec![ToolDef {
                name: "get_weather".into(),
                description: "Get weather for a city".into(),
                input_schema: serde_json::json!({"type": "object", "properties": {}}),
            }],
            ..Default::default()
        };
        let result = provider
            .chat_with_options_dyn("", &[ChatMessage::user("hi")], &opts, &Value::Null)
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    /// Test C: `tool_choice: Required` serialises as the string `"required"`.
    #[tokio::test(flavor = "multi_thread")]
    async fn tool_choice_required_appears_as_string_required() {
        use crate::{LlmProviderDyn, LlmRequestOptions, ToolChoice, ToolDef};
        use serde_json::Value;

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(serde_json::json!({
                "tool_choice": "required"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "data: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\ndata: [DONE]\n",
            ))
            .mount(&server)
            .await;

        let provider = CodexProvider::new_with_base_url("k".into(), "gpt-5".into(), server.uri());
        let opts = LlmRequestOptions {
            tools: vec![ToolDef {
                name: "get_weather".into(),
                description: "dummy".into(),
                input_schema: serde_json::json!({"type": "object"}),
            }],
            tool_choice: Some(ToolChoice::Required),
            ..Default::default()
        };
        let result = provider
            .chat_with_options_dyn("", &[ChatMessage::user("hi")], &opts, &Value::Null)
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    /// Test D: `tool_choice: Specific(name)` serialises as
    /// `{type: "function", name: "..."}` — Responses API format where `name` is
    /// at the top level, NOT `{type: "function", function: {name: "..."}}` like
    /// Chat Completions.
    #[tokio::test(flavor = "multi_thread")]
    async fn tool_choice_specific_uses_responses_api_format() {
        use crate::{LlmProviderDyn, LlmRequestOptions, ToolChoice, ToolDef};
        use serde_json::Value;

        let server = MockServer::start().await;
        // Assert Responses API format: name at top level.
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(serde_json::json!({
                "tool_choice": {"type": "function", "name": "get_weather"}
            })))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "data: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\ndata: [DONE]\n",
            ))
            .mount(&server)
            .await;

        let provider = CodexProvider::new_with_base_url("k".into(), "gpt-5".into(), server.uri());
        let opts = LlmRequestOptions {
            tools: vec![ToolDef {
                name: "get_weather".into(),
                description: "dummy".into(),
                input_schema: serde_json::json!({"type": "object"}),
            }],
            tool_choice: Some(ToolChoice::Specific("get_weather".into())),
            ..Default::default()
        };
        let result = provider
            .chat_with_options_dyn("", &[ChatMessage::user("hi")], &opts, &Value::Null)
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    /// Test E: `ChatContent::ToolResult` is encoded as a top-level
    /// `{type: "function_call_output", call_id, output}` item in the `input`
    /// array — Responses API format, NOT a `role: "tool"` message like Chat
    /// Completions.
    #[tokio::test(flavor = "multi_thread")]
    async fn tool_result_becomes_function_call_output_in_input() {
        use crate::{LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;

        let server = MockServer::start().await;
        // Assert Responses API input encoding.
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(serde_json::json!({
                "input": [
                    {
                        "type": "function_call_output",
                        "call_id": "call_01",
                        "output": "25C"
                    }
                ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "data: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\ndata: [DONE]\n",
            ))
            .mount(&server)
            .await;

        let provider = CodexProvider::new_with_base_url("k".into(), "gpt-5".into(), server.uri());
        let messages = vec![ChatMessage::tool_result("call_01", "25C", false)];
        let result = provider
            .chat_with_options_dyn("", &messages, &LlmRequestOptions::default(), &Value::Null)
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Task 13: HTTP image input tests (Responses API)
    // ──────────────────────────────────────────────────────────────────────────

    /// Test A: base64 image block serializes as `input_image` with a data URL
    /// string (Responses API quirk: `image_url` is a STRING, not an object).
    #[tokio::test(flavor = "multi_thread")]
    async fn image_base64_block_serializes_correctly() {
        use crate::{ChatContent, ChatMessage, ImageSource, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(serde_json::json!({
                "input": [
                    {
                        "type": "message",
                        "role": "user",
                        "content": [
                            {"type": "input_text", "text": "describe"},
                            {"type": "input_image", "image_url": "data:image/png;base64,AAAA"}
                        ]
                    }
                ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "data: {\"type\":\"response.output_text.delta\",\"delta\":\"a square\"}\ndata: [DONE]\n",
            ))
            .mount(&server)
            .await;

        let provider = CodexProvider::new_with_base_url("k".into(), "gpt-5".into(), server.uri());
        let msg = ChatMessage::user_multipart(vec![
            ChatContent::Text("describe".into()),
            ChatContent::Image {
                source: ImageSource::Base64("AAAA".into()),
                mime_type: "image/png".into(),
            },
        ]);
        let result = provider
            .chat_with_options_dyn("", &[msg], &LlmRequestOptions::default(), &Value::Null)
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    /// Test B: URL image block serializes as `input_image` with the plain HTTPS
    /// URL string (Responses API: `image_url` is a string, not an object).
    #[tokio::test(flavor = "multi_thread")]
    async fn image_url_block_serializes_correctly() {
        use crate::{ChatContent, ChatMessage, ImageSource, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(serde_json::json!({
                "input": [
                    {
                        "type": "message",
                        "role": "user",
                        "content": [
                            {"type": "input_image", "image_url": "https://example.com/photo.jpg"}
                        ]
                    }
                ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "data: {\"type\":\"response.output_text.delta\",\"delta\":\"a photo\"}\ndata: [DONE]\n",
            ))
            .mount(&server)
            .await;

        let provider = CodexProvider::new_with_base_url("k".into(), "gpt-5".into(), server.uri());
        let msg = ChatMessage::user_multipart(vec![ChatContent::Image {
            source: ImageSource::Url("https://example.com/photo.jpg".into()),
            mime_type: "image/jpeg".into(),
        }]);
        let result = provider
            .chat_with_options_dyn("", &[msg], &LlmRequestOptions::default(), &Value::Null)
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Task 6: reasoning_content / thinking extraction tests
    // ──────────────────────────────────────────────────────────────────────────

    /// Test A: A Responses API JSON response containing a `reasoning` output item
    /// (summary array shape) populates `LlmResponse.thinking_text`.
    #[tokio::test(flavor = "multi_thread")]
    async fn reasoning_in_blocking_response_populates_thinking_text() {
        use crate::{LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;

        let fixture = include_str!("../tests/fixtures/codex_reasoning_response.json");

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(fixture)
                    .insert_header("content-type", "application/json"),
            )
            .mount(&server)
            .await;

        let provider = CodexProvider::new_with_base_url("k".into(), "gpt-o4".into(), server.uri());
        let resp = provider
            .chat_with_options_dyn(
                "",
                &[ChatMessage::user("think")],
                &LlmRequestOptions::default(),
                &Value::Null,
            )
            .await
            .expect("expected Ok response");

        assert_eq!(
            resp.thinking_text.as_deref(),
            Some("Step 1: think..."),
            "expected thinking_text from summary array"
        );
        assert_eq!(resp.rendered_text, "Final.");
        assert_eq!(resp.input_tokens, Some(50));
        assert_eq!(resp.output_tokens, Some(30));
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Task 11: Streaming usage event tests
    // ──────────────────────────────────────────────────────────────────────────

    /// When the Responses API returns a blocking JSON response with a `usage`
    /// field, the stream wrapper must emit a `LlmStreamEvent::Usage` event
    /// before the terminal `End` event.
    #[tokio::test(flavor = "multi_thread")]
    async fn streaming_usage_event_emitted_before_end() {
        use crate::{LlmProviderDyn, LlmRequestOptions, LlmStreamEvent};
        use futures::StreamExt;
        use serde_json::Value;

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({
                        "output": [
                            {
                                "type": "message",
                                "role": "assistant",
                                "content": [{"type": "output_text", "text": "hello"}]
                            }
                        ],
                        "usage": {
                            "input_tokens": 10,
                            "output_tokens": 5
                        }
                    }))
                    .insert_header("content-type", "application/json"),
            )
            .mount(&server)
            .await;

        let provider = CodexProvider::new_with_base_url("k".into(), "gpt-5".into(), server.uri());
        let opts = LlmRequestOptions::default();
        let result_json = Value::Null;
        let msgs = vec![ChatMessage::user("hi")];
        let mut stream = provider.chat_stream_with_options_dyn("", &msgs, &opts, &result_json);

        let mut saw_usage = false;
        let mut usage_before_end = false;
        let mut end_total: Option<crate::LlmResponse> = None;

        while let Some(event) = stream.next().await {
            match event {
                LlmStreamEvent::Usage {
                    input_tokens,
                    output_tokens,
                    ..
                } => {
                    assert_eq!(input_tokens, Some(10), "Usage.input_tokens must be 10");
                    assert_eq!(output_tokens, 5, "Usage.output_tokens must be 5");
                    saw_usage = true;
                }
                LlmStreamEvent::End { total } => {
                    if saw_usage {
                        usage_before_end = true;
                    }
                    end_total = Some(total);
                    break;
                }
                LlmStreamEvent::Error { message } => panic!("unexpected error: {message}"),
                #[allow(unreachable_patterns)]
                _ => {}
            }
        }

        assert!(saw_usage, "expected a Usage event");
        assert!(usage_before_end, "Usage event must appear before End");
        let total = end_total.expect("expected End event");
        assert_eq!(total.input_tokens, Some(10));
        assert_eq!(total.output_tokens, Some(5));
    }

    /// Test B: When `thinking_budget_tokens` is set and `reasoning_effort` is
    /// absent, the request body must contain `"reasoning": {"effort": "high"}`.
    /// An explicit `reasoning_effort` must never be overridden by this inference.
    #[tokio::test(flavor = "multi_thread")]
    async fn thinking_budget_implies_high_reasoning_effort_when_unset() {
        use crate::{LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(serde_json::json!({
                "reasoning": {"effort": "high"}
            })))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "data: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\ndata: [DONE]\n",
            ))
            .mount(&server)
            .await;

        let provider = CodexProvider::new_with_base_url("k".into(), "gpt-o4".into(), server.uri());
        let opts = LlmRequestOptions {
            thinking_budget_tokens: Some(1024),
            reasoning_effort: None,
            ..Default::default()
        };
        let result = provider
            .chat_with_options_dyn("", &[ChatMessage::user("think hard")], &opts, &Value::Null)
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }
}
