//! GitHub Copilot Chat provider.
//!
//! **Tool calling support is EXPERIMENTAL.** Implemented against the
//! OpenAI Chat Completions schema (which Copilot's API closely mirrors),
//! but real-world behavior with the live Copilot endpoint is not
//! guaranteed. Tool support varies by Copilot tier and is not publicly
//! documented. Use at your own risk; tests only cover mock responses.
//!
//! **Usage event emission is EXPERIMENTAL.** Copilot does not publish
//! its pricing, so `estimated_cost_usd` is always `0.0`. Token counts
//! are derived from the OpenAI-compatible `usage` field in the response.
//! A single `LlmStreamEvent::Usage` event is emitted before `End` when
//! token data is present.

use reqwest::Client;
use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

use super::{
    ChatContent, ChatMessage, ImageSource, LlmError, LlmProviderDyn, LlmRequestOptions,
    LlmResponse, ToolCall, ToolChoice,
};

const COPILOT_API_URL: &str = "https://api.githubcopilot.com/chat/completions";
const COPILOT_TOKEN_URL: &str = "https://api.github.com/copilot_internal/v2/token";
const REQUEST_TIMEOUT_SECS: u64 = 60;
const MAX_TOKENS: u32 = 1024;

pub struct CopilotProvider {
    client: Client,
    github_token: String,
    copilot_token: std::sync::Mutex<String>,
    model: String,
    base_url: String,
}

/// Serialised request body sent to the Copilot Chat Completions endpoint.
///
/// Uses raw [`Value`] for the `messages`, `tools`, and `tool_choice` fields so
/// that the encoding logic in [`CopilotProvider::build_messages`] and the tools
/// helpers can construct OpenAI-compatible wire format directly.
#[derive(serde::Serialize)]
struct CopilotRequestBody {
    model: String,
    messages: Value,
    stream: bool,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<Value>,
}

#[derive(serde::Deserialize)]
struct CopilotTokenResponse {
    token: Option<String>,
}

impl CopilotProvider {
    /// token에 "ghu_" 또는 "gho_"로 시작하면 GitHub token, 아니면 Copilot token
    pub fn new(token: String, model: String) -> Self {
        Self::new_with_base_url(token, model, COPILOT_API_URL.to_string())
    }

    /// For testing only — allows pointing the chat-completion endpoint at a
    /// mock HTTP server. Production callers should use [`Self::new`], which
    /// defaults to `COPILOT_API_URL` (`https://api.githubcopilot.com/chat/completions`).
    /// The OAT token refresh endpoint `COPILOT_TOKEN_URL`
    /// (`https://api.github.com/copilot_internal/v2/token`) is not affected by
    /// this override.
    pub fn new_with_base_url(token: String, model: String, base_url: String) -> Self {
        let model = if model.is_empty() {
            "claude-sonnet-4.6".to_string()
        } else {
            model
        };
        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .expect("Failed to build HTTP client");

        let (github_token, copilot_token) =
            if token.starts_with("ghu_") || token.starts_with("gho_") {
                (token, String::new())
            } else {
                // Assume it's already a copilot token — store empty github token
                (String::new(), token)
            };

        Self {
            client,
            github_token,
            copilot_token: std::sync::Mutex::new(copilot_token),
            model,
            base_url,
        }
    }

    /// Build the OpenAI-format `messages` array.
    ///
    /// For each [`ChatMessage`]:
    /// - Text-only messages emit `{role, content: <string>}` (plain string form).
    /// - Messages with at least one [`ChatContent::Image`] block emit
    ///   `{role, content: [{type:"text",text}, {type:"image_url",image_url:{url}},...]}`.
    ///   `ImageSource::Base64` encodes as `data:<mime>;base64,<data>`;
    ///   `ImageSource::Url` passes the URL directly.
    /// - `ToolResult` blocks each become a **separate** `{role: "tool",
    ///   tool_call_id, content}` message (OpenAI wire-format quirk — not a
    ///   content block inside a user message like Anthropic).
    ///
    /// If a single [`ChatMessage`] has both text/image and tool-result blocks, a
    /// content message is emitted first (if non-empty), followed by one tool
    /// message per result block.
    fn build_messages(system_prompt: &str, messages: &[ChatMessage]) -> Value {
        let mut out: Vec<Value> = Vec::new();
        if !system_prompt.is_empty() {
            out.push(json!({"role": "system", "content": system_prompt}));
        }
        for m in messages {
            let mut text_parts: Vec<&str> = Vec::new();
            let mut image_blocks: Vec<Value> = Vec::new();
            let mut tool_results: Vec<Value> = Vec::new();
            let mut has_image = false;

            for block in &m.content {
                match block {
                    ChatContent::Text(t) => text_parts.push(t.as_str()),
                    ChatContent::Image { source, mime_type } => {
                        has_image = true;
                        let url = match source {
                            ImageSource::Base64(data) => {
                                format!("data:{mime_type};base64,{data}")
                            }
                            ImageSource::Url(url) => url.clone(),
                        };
                        image_blocks.push(json!({
                            "type": "image_url",
                            "image_url": {"url": url},
                        }));
                    }
                    ChatContent::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        let wire_content = if *is_error {
                            format!("Error: {content}")
                        } else {
                            content.clone()
                        };
                        tool_results.push(json!({
                            "role": "tool",
                            "tool_call_id": tool_use_id,
                            "content": wire_content,
                        }));
                    }
                }
            }

            let text = text_parts.join("");
            let has_content = !text.is_empty() || has_image;
            if has_content {
                if has_image {
                    // Array form: interleave text block (if any) then image blocks.
                    let mut content_arr: Vec<Value> = Vec::new();
                    if !text.is_empty() {
                        content_arr.push(json!({"type": "text", "text": text}));
                    }
                    content_arr.extend(image_blocks);
                    out.push(json!({"role": m.role, "content": content_arr}));
                } else {
                    // Plain string form for text-only messages.
                    out.push(json!({"role": m.role, "content": text}));
                }
            }
            out.extend(tool_results);
        }
        json!(out)
    }

    /// Encode `tools` into the OpenAI Chat Completions wire format.
    fn encode_tools(options: &LlmRequestOptions) -> Option<Value> {
        if options.tools.is_empty() {
            return None;
        }
        Some(json!(
            options
                .tools
                .iter()
                .map(|t| json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    }
                }))
                .collect::<Vec<_>>()
        ))
    }

    /// Encode `tool_choice` into the OpenAI Chat Completions wire format.
    fn encode_tool_choice(options: &LlmRequestOptions) -> Option<Value> {
        options.tool_choice.as_ref().map(|choice| match choice {
            ToolChoice::Auto => json!("auto"),
            ToolChoice::Required => json!("required"),
            ToolChoice::None => json!("none"),
            ToolChoice::Specific(name) => {
                json!({"type": "function", "function": {"name": name}})
            }
        })
    }

    /// Parse `tool_calls` from an OpenAI-format response JSON value.
    fn parse_tool_calls(json: &Value) -> Vec<ToolCall> {
        json["choices"][0]["message"]["tool_calls"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|tc| {
                        let id = tc["id"].as_str()?.to_string();
                        let name = tc["function"]["name"].as_str()?.to_string();
                        let args_str = tc["function"]["arguments"].as_str()?;
                        let arguments: Value = serde_json::from_str(args_str).ok()?;
                        Some(ToolCall {
                            id,
                            name,
                            arguments,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Copilot 토큰 갱신 (GitHub token → Copilot token 교환).
    /// Keeps `Result<_, String>` — internal helper; conversion happens at the trait boundary.
    async fn refresh_copilot_token(&self) -> Result<String, String> {
        if self.github_token.is_empty() {
            return Err("No GitHub token for refresh".to_string());
        }

        let resp = self
            .client
            .get(COPILOT_TOKEN_URL)
            .header("Authorization", format!("token {}", self.github_token))
            .header("Accept", "application/json")
            .header("Editor-Version", "vscode/1.85.1")
            .header("Editor-Plugin-Version", "copilot/1.155.0")
            .header("User-Agent", "dalgyeol-backend")
            .send()
            .await
            .map_err(|e| format!("Copilot token refresh failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Copilot token refresh HTTP {}", resp.status()));
        }

        let body: CopilotTokenResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse token response: {e}"))?;

        let token = body.token.ok_or("No token in response")?;
        if let Ok(mut t) = self.copilot_token.lock() {
            *t = token.clone();
        }
        tracing::info!("Copilot token refreshed successfully");
        Ok(token)
    }

    /// 현재 Copilot 토큰 가져오기 (없거나 비어있으면 갱신).
    /// Keeps `Result<_, String>` — internal helper.
    async fn get_token(&self) -> Result<String, String> {
        let current = self
            .copilot_token
            .lock()
            .map(|t| t.clone())
            .unwrap_or_default();
        if current.is_empty() {
            return self.refresh_copilot_token().await;
        }
        Ok(current)
    }
}

impl LlmProviderDyn for CopilotProvider {
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

    fn chat_stream_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        result_json: &'a Value,
    ) -> futures::stream::BoxStream<'a, crate::LlmStreamEvent> {
        use crate::LlmStreamEvent;
        let fut = self.chat_dyn(system_prompt, messages, result_json);
        Box::pin(async_stream::stream! {
            match fut.await {
                Ok(resp) => {
                    if !resp.rendered_text.is_empty() {
                        yield LlmStreamEvent::Delta { text: resp.rendered_text.clone() };
                    }
                    for call in resp.tool_calls.iter().cloned() {
                        yield LlmStreamEvent::ToolCall { call };
                    }
                    // EXPERIMENTAL: emit a single Usage event before End when
                    // token data is present. Copilot does not publish pricing,
                    // so estimated_cost_usd is always 0.0.
                    if let Some(output_tokens) = resp.output_tokens {
                        yield LlmStreamEvent::Usage {
                            input_tokens: resp.input_tokens,
                            output_tokens,
                            estimated_cost_usd: 0.0,
                        };
                    }
                    yield LlmStreamEvent::End { total: resp };
                }
                Err(e) => yield LlmStreamEvent::Error { message: e.to_string() },
            }
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
                    if !resp.rendered_text.is_empty() {
                        yield LlmStreamEvent::Delta { text: resp.rendered_text.clone() };
                    }
                    // Emit any tool calls collected during the blocking response.
                    for call in resp.tool_calls.iter().cloned() {
                        yield LlmStreamEvent::ToolCall { call };
                    }
                    // EXPERIMENTAL: emit a single Usage event before End when
                    // token data is present. Copilot does not publish pricing,
                    // so estimated_cost_usd is always 0.0.
                    if let Some(output_tokens) = resp.output_tokens {
                        yield LlmStreamEvent::Usage {
                            input_tokens: resp.input_tokens,
                            output_tokens,
                            estimated_cost_usd: 0.0,
                        };
                    }
                    yield LlmStreamEvent::End { total: resp };
                }
                Err(e) => yield LlmStreamEvent::Error { message: e.to_string() },
            }
        })
    }
}

impl CopilotProvider {
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
        options: &crate::LlmRequestOptions,
    ) -> Result<LlmResponse, LlmError> {
        let wire_messages = Self::build_messages(system_prompt, messages);
        let tools = Self::encode_tools(options);
        let tool_choice = Self::encode_tool_choice(options);

        let body = CopilotRequestBody {
            model: self.model.clone(),
            messages: wire_messages,
            stream: false,
            max_tokens: options.max_tokens.unwrap_or(MAX_TOKENS),
            temperature: options.temperature,
            top_p: options.top_p,
            stop: options.stop_sequences.clone(),
            tools,
            tool_choice,
        };

        // Acquire token — failed refresh surfaces as Auth failure (no credential available)
        let token = self.get_token().await.map_err(|_| LlmError::Auth {
            provider: "copilot".into(),
        })?;

        let resp = self
            .send_request(&body, &token, request_timeout)
            .await
            .map_err(|e| LlmError::Network {
                provider: "copilot".into(),
                msg: e,
            })?;

        let status = resp.status();
        if status.as_u16() == 401 && !self.github_token.is_empty() {
            // Token expired — refresh and retry (internal retry, keep String errors)
            tracing::info!("Copilot token expired, refreshing...");
            let new_token = self
                .refresh_copilot_token()
                .await
                .map_err(|_| LlmError::Auth {
                    provider: "copilot".into(),
                })?;
            let resp = self
                .send_request(&body, &new_token, request_timeout)
                .await
                .map_err(|e| LlmError::Network {
                    provider: "copilot".into(),
                    msg: e,
                })?;
            return self.parse_response(resp).await;
        }

        if !status.is_success() {
            let retry_after = crate::error::parse_retry_after(&resp);
            let err_body = resp.text().await.unwrap_or_default();
            tracing::error!(status = %status, body = %err_body, model = %self.model, "Copilot API error");
            return Err(copilot_http_error(status, err_body, retry_after));
        }

        self.parse_response(resp).await
    }

    async fn send_request(
        &self,
        body: &CopilotRequestBody,
        token: &str,
        request_timeout: Option<Duration>,
    ) -> Result<reqwest::Response, String> {
        let mut req = self
            .client
            .post(&self.base_url)
            .header("Authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .header("Editor-Version", "vscode/1.85.1")
            .header("Editor-Plugin-Version", "copilot/1.155.0")
            .header("Copilot-Integration-Id", "vscode-chat")
            .header("X-Request-Id", Uuid::new_v4().to_string())
            .json(body);
        if let Some(d) = request_timeout {
            req = req.timeout(d);
        }
        req.send().await.map_err(|e| {
            if e.is_timeout() {
                "timeout".to_string()
            } else {
                "Copilot API request failed".to_string()
            }
        })
    }

    async fn parse_response(&self, resp: reqwest::Response) -> Result<LlmResponse, LlmError> {
        let status = resp.status();
        if !status.is_success() {
            let retry_after = crate::error::parse_retry_after(&resp);
            let err_body = resp.text().await.unwrap_or_default();
            tracing::error!(status = %status, body = %err_body, model = %self.model, "Copilot API error");
            return Err(copilot_http_error(status, err_body, retry_after));
        }

        let body_text = resp.text().await.map_err(|e| LlmError::Network {
            provider: "copilot".into(),
            msg: format!("failed to read response body: {e}"),
        })?;

        let json: Value = serde_json::from_str(&body_text).map_err(|e| LlmError::Parse {
            provider: "copilot".into(),
            reason: e.to_string(),
        })?;

        let text = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let model = json["model"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.model.clone());

        let input_tokens = json["usage"]["prompt_tokens"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok());
        let output_tokens = json["usage"]["completion_tokens"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok());

        // Parse tool_calls from the response message (EXPERIMENTAL).
        let tool_calls = Self::parse_tool_calls(&json);

        Ok(LlmResponse {
            rendered_text: text,
            model,
            estimated_cost_usd: 0.0,
            input_tokens,
            output_tokens,
            tool_calls,
            ..Default::default()
        })
    }
}

/// `retry_after` must be pre-computed via `parse_retry_after(&resp)` before the body is consumed.
fn copilot_http_error(
    status: reqwest::StatusCode,
    body: String,
    retry_after: Option<u32>,
) -> LlmError {
    match status.as_u16() {
        401 | 403 => LlmError::Auth {
            provider: "copilot".into(),
        },
        429 => LlmError::RateLimit {
            provider: "copilot".into(),
            retry_after_secs: retry_after,
        },
        s => LlmError::Upstream {
            provider: "copilot".into(),
            status: s,
            body,
        },
    }
}

pub fn make(
    api_key: &str,
    model: &str,
    _base_url: &str,
) -> Result<Box<dyn crate::LlmProviderDyn>, crate::LlmError> {
    if api_key.is_empty() {
        return Err(crate::LlmError::MissingConfig {
            provider: "copilot".into(),
            reason: "API key is empty".into(),
        });
    }
    Ok(Box::new(CopilotProvider::new(
        api_key.to_string(),
        model.to_string(),
    )))
}

pub async fn list_models(_api_key: &str) -> Result<Vec<crate::ListModel>, LlmError> {
    // No public model listing API for GitHub Copilot
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ──────────────────────────────────────────────────────────────────────────
    // Pre-existing tests
    // ──────────────────────────────────────────────────────────────────────────

    #[tokio::test(flavor = "multi_thread")]
    async fn sampling_params_appear_in_request_body() {
        use crate::{ChatMessage, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use wiremock::matchers::{self, body_partial_json};
        use wiremock::{Mock, MockServer, ResponseTemplate};

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
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"role": "assistant", "content": "ok"}}],
                "model": "copilot-test"
            })))
            .mount(&server)
            .await;

        // Pass a direct Copilot token (not ghu_/gho_) to skip the GitHub token exchange
        let provider = CopilotProvider::new_with_base_url(
            "copilot-direct-token".into(),
            "test-model".into(),
            server.uri(),
        );
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
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_secs(2))
                    .set_body_json(serde_json::json!({"choices":[{"message":{"content":"late"}}]})),
            )
            .mount(&server)
            .await;

        let provider = CopilotProvider::new_with_base_url(
            "copilot-token-direct".into(),
            "test-model".into(),
            server.uri(),
        );
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
                    if provider == "copilot" && msg.to_lowercase().contains("timeout")
            ),
            "expected copilot Network/timeout, got {res:?}"
        );
        assert!(
            elapsed < Duration::from_millis(1500),
            "should abort < 1.5s, took {elapsed:?}"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Task 9: EXPERIMENTAL tool calling tests
    // ──────────────────────────────────────────────────────────────────────────

    /// Test A (EXPERIMENTAL): blocking response with `tool_calls` is parsed into
    /// `LlmResponse.tool_calls`.
    ///
    /// Note: copilot tool calling is not publicly documented. This test uses a
    /// mock server that returns an OpenAI-compatible fixture.
    #[tokio::test(flavor = "multi_thread")]
    async fn tool_call_in_blocking_response_returns_tool_calls() {
        use crate::{LlmProviderDyn, LlmRequestOptions, ToolDef};
        use serde_json::Value;
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let fixture = include_str!("../tests/fixtures/copilot_tool_call_response.json");

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(fixture)
                    .insert_header("content-type", "application/json"),
            )
            .mount(&server)
            .await;

        let provider = CopilotProvider::new_with_base_url(
            "copilot-direct-token".into(),
            "test-model".into(),
            server.uri(),
        );
        let opts = LlmRequestOptions {
            tools: vec![ToolDef {
                name: "get_weather".into(),
                description: "Get weather for a city".into(),
                input_schema: serde_json::json!({"type": "object", "properties": {}}),
            }],
            ..Default::default()
        };

        let resp = provider
            .chat_with_options_dyn("", &[ChatMessage::user("weather?")], &opts, &Value::Null)
            .await
            .expect("expected ok response");

        assert_eq!(resp.tool_calls.len(), 1, "expected 1 tool call");
        let call = &resp.tool_calls[0];
        assert_eq!(call.name, "get_weather");
        assert_eq!(call.id, "call_01");
        assert_eq!(call.arguments["city"], "seoul");
        assert_eq!(resp.rendered_text, "Checking the weather.");
    }

    /// Test B (EXPERIMENTAL): `tools` array appears in the serialised request body
    /// in OpenAI Chat Completions format.
    #[tokio::test(flavor = "multi_thread")]
    async fn tools_appear_in_request_body() {
        use crate::{LlmProviderDyn, LlmRequestOptions, ToolDef};
        use serde_json::Value;
        use wiremock::matchers::{body_partial_json, method};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({
                "tools": [{
                    "type": "function",
                    "function": {
                        "name": "get_weather",
                        "description": "Get weather for a city"
                    }
                }]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "model": "copilot-test",
                "choices": [{"message": {"role": "assistant", "content": "ok"}}],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1}
            })))
            .mount(&server)
            .await;

        let provider = CopilotProvider::new_with_base_url(
            "copilot-direct-token".into(),
            "test-model".into(),
            server.uri(),
        );
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

    /// Test C (EXPERIMENTAL): `tool_choice: Required` serialises as the string
    /// `"required"` (OpenAI wire format).
    #[tokio::test(flavor = "multi_thread")]
    async fn tool_choice_required_appears_as_string_required() {
        use crate::{LlmProviderDyn, LlmRequestOptions, ToolChoice, ToolDef};
        use serde_json::Value;
        use wiremock::matchers::{body_partial_json, method};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({
                "tool_choice": "required"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "model": "copilot-test",
                "choices": [{"message": {"role": "assistant", "content": "ok"}}],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1}
            })))
            .mount(&server)
            .await;

        let provider = CopilotProvider::new_with_base_url(
            "copilot-direct-token".into(),
            "test-model".into(),
            server.uri(),
        );
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

    /// Test D (EXPERIMENTAL): `ChatMessage::tool_result` encodes as a separate
    /// `{role: "tool", tool_call_id, content}` message (OpenAI wire-format).
    #[tokio::test(flavor = "multi_thread")]
    async fn tool_result_message_role_serializes_correctly() {
        use crate::{LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use wiremock::matchers::{body_partial_json, method};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({
                "messages": [
                    {
                        "role": "tool",
                        "tool_call_id": "call_01",
                        "content": "25C"
                    }
                ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "model": "copilot-test",
                "choices": [{"message": {"role": "assistant", "content": "ok"}}],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1}
            })))
            .mount(&server)
            .await;

        let provider = CopilotProvider::new_with_base_url(
            "copilot-direct-token".into(),
            "test-model".into(),
            server.uri(),
        );
        let messages = vec![ChatMessage::tool_result("call_01", "25C", false)];
        let result = provider
            .chat_with_options_dyn("", &messages, &LlmRequestOptions::default(), &Value::Null)
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    /// Test E (EXPERIMENTAL): streaming wrapper emits `LlmStreamEvent::ToolCall`
    /// events when the underlying blocking response contains tool calls.
    ///
    /// Copilot has no native SSE streaming impl — the stream adapter wraps the
    /// blocking `chat_with_options_dyn` result, so tool calls from the blocking
    /// response must propagate into `ToolCall` stream events before the `End`
    /// event.
    #[tokio::test(flavor = "multi_thread")]
    async fn tool_call_in_streaming_emits_toolcall_event() {
        use crate::{LlmProviderDyn, LlmRequestOptions, LlmStreamEvent, ToolCall as TC, ToolDef};
        use futures::StreamExt;
        use serde_json::Value;
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let fixture = include_str!("../tests/fixtures/copilot_tool_call_response.json");

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(fixture)
                    .insert_header("content-type", "application/json"),
            )
            .mount(&server)
            .await;

        let provider = CopilotProvider::new_with_base_url(
            "copilot-direct-token".into(),
            "test-model".into(),
            server.uri(),
        );
        let opts = LlmRequestOptions {
            tools: vec![ToolDef {
                name: "get_weather".into(),
                description: "Get weather for a city".into(),
                input_schema: serde_json::json!({"type": "object"}),
            }],
            ..Default::default()
        };

        let msgs = vec![ChatMessage::user("weather?")];
        let mut stream = provider.chat_stream_with_options_dyn("", &msgs, &opts, &Value::Null);

        let mut got_delta = false;
        let mut tool_call_events: Vec<TC> = Vec::new();
        let mut end_total: Option<LlmResponse> = None;

        while let Some(event) = stream.next().await {
            match event {
                LlmStreamEvent::Delta { .. } => got_delta = true,
                LlmStreamEvent::ToolCall { call } => tool_call_events.push(call),
                LlmStreamEvent::End { total } => {
                    end_total = Some(total);
                    break;
                }
                LlmStreamEvent::Error { message } => panic!("unexpected error: {message}"),
                #[allow(unreachable_patterns)]
                _ => {}
            }
        }

        assert!(got_delta, "expected at least one Delta event");
        assert_eq!(
            tool_call_events.len(),
            1,
            "expected exactly one ToolCall event from streaming wrapper"
        );
        let call = &tool_call_events[0];
        assert_eq!(call.name, "get_weather");
        assert_eq!(call.id, "call_01");
        assert_eq!(call.arguments["city"], "seoul");

        let total = end_total.expect("expected End event");
        assert_eq!(
            total.tool_calls.len(),
            1,
            "End.total should carry accumulated tool calls"
        );
        assert_eq!(total.tool_calls[0].name, "get_weather");
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Task 13: HTTP image input tests (Copilot = OpenAI-compatible)
    // ──────────────────────────────────────────────────────────────────────────

    /// Test A: base64 image block serializes as `image_url` content block with a
    /// data URL (`data:<mime>;base64,<data>`) in array-form message content.
    #[tokio::test(flavor = "multi_thread")]
    async fn image_base64_block_serializes_correctly() {
        use crate::{ChatContent, ChatMessage, ImageSource, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use wiremock::matchers::{body_partial_json, method};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({
                "messages": [
                    {
                        "role": "user",
                        "content": [
                            {"type": "text", "text": "describe"},
                            {"type": "image_url", "image_url": {"url": "data:image/png;base64,AAAA"}}
                        ]
                    }
                ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "model": "copilot-test",
                "choices": [{"message": {"role": "assistant", "content": "a red square"}}],
                "usage": {"prompt_tokens": 10, "completion_tokens": 5}
            })))
            .mount(&server)
            .await;

        let provider = CopilotProvider::new_with_base_url(
            "copilot-direct-token".into(),
            "test-model".into(),
            server.uri(),
        );
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

    /// Test B: URL image block serializes as `image_url` content block with the
    /// plain HTTPS URL (no base64 encoding, no mime prefix).
    #[tokio::test(flavor = "multi_thread")]
    async fn image_url_block_serializes_correctly() {
        use crate::{ChatContent, ChatMessage, ImageSource, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use wiremock::matchers::{body_partial_json, method};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({
                "messages": [
                    {
                        "role": "user",
                        "content": [
                            {"type": "image_url", "image_url": {"url": "https://example.com/photo.jpg"}}
                        ]
                    }
                ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "model": "copilot-test",
                "choices": [{"message": {"role": "assistant", "content": "a photo"}}],
                "usage": {"prompt_tokens": 10, "completion_tokens": 3}
            })))
            .mount(&server)
            .await;

        let provider = CopilotProvider::new_with_base_url(
            "copilot-direct-token".into(),
            "test-model".into(),
            server.uri(),
        );
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
    // Task 13: EXPERIMENTAL streaming Usage event emission
    // ──────────────────────────────────────────────────────────────────────────

    /// EXPERIMENTAL: the stream adapter emits a single `LlmStreamEvent::Usage`
    /// event before the terminal `End` event when the blocking response contains
    /// token counts. Because copilot has no native SSE streaming, usage data
    /// comes from the blocking response's `usage` field rather than a final SSE
    /// chunk, but the observable stream contract is identical to openai_compat.
    #[tokio::test(flavor = "multi_thread")]
    async fn streaming_usage_event_emitted_before_end() {
        use crate::{ChatMessage, LlmProviderDyn, LlmStreamEvent};
        use futures::StreamExt;
        use serde_json::Value;
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

        let server = MockServer::start().await;
        // Return a plain JSON (non-streaming) response with usage data.
        // Copilot's stream adapter calls the blocking endpoint regardless of
        // whether the caller uses chat_stream_dyn or chat_stream_with_options_dyn.
        Mock::given(matchers::method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_json(serde_json::json!({
                        "model": "copilot-test",
                        "choices": [{"message": {"role": "assistant", "content": "hi there"}}],
                        "usage": {
                            "prompt_tokens": 10,
                            "completion_tokens": 5,
                            "total_tokens": 15
                        }
                    })),
            )
            .mount(&server)
            .await;

        let provider = CopilotProvider::new_with_base_url(
            "copilot-direct-token".into(),
            "test-model".into(),
            server.uri(),
        );
        let messages = vec![ChatMessage::user("test")];
        let mut stream = provider.chat_stream_dyn("", &messages, &Value::Null);

        let mut usage_seen = false;
        let mut end_seen = false;
        let mut usage_before_end = true;

        while let Some(event) = stream.next().await {
            match event {
                LlmStreamEvent::Usage {
                    input_tokens,
                    output_tokens,
                    estimated_cost_usd,
                } => {
                    assert_eq!(output_tokens, 5, "output_tokens should be 5");
                    assert_eq!(input_tokens, Some(10), "input_tokens should be Some(10)");
                    assert_eq!(
                        estimated_cost_usd, 0.0,
                        "cost is 0.0 (copilot pricing undisclosed)"
                    );
                    usage_seen = true;
                    if end_seen {
                        usage_before_end = false;
                    }
                }
                LlmStreamEvent::End { total } => {
                    end_seen = true;
                    assert_eq!(
                        total.output_tokens,
                        Some(5),
                        "End.total.output_tokens should be 5"
                    );
                    assert_eq!(
                        total.input_tokens,
                        Some(10),
                        "End.total.input_tokens should be 10"
                    );
                }
                LlmStreamEvent::Error { message } => panic!("unexpected error: {message}"),
                _ => {}
            }
        }

        assert!(usage_seen, "expected a Usage event");
        assert!(end_seen, "expected an End event");
        assert!(usage_before_end, "Usage event must precede End event");
    }
}
