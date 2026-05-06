use futures::StreamExt;
use futures::stream::BoxStream;
use reqwest::Client;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::Duration;

use super::{
    ChatContent, ChatMessage, ImageSource, LlmError, LlmProviderDyn, LlmRequestOptions,
    LlmResponse, LlmStreamEvent, ToolCall, ToolChoice,
};

const DEFAULT_BASE_URL: &str = "http://localhost:11434/v1";
const REQUEST_TIMEOUT_SECS: u64 = 60;
const STREAM_TIMEOUT_SECS: u64 = 300;

pub struct OpenAICompatProvider {
    client: Client,
    stream_client: Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenAICompatProvider {
    pub fn new(base_url: String, api_key: String, model: String) -> Self {
        let base_url = if base_url.is_empty() {
            DEFAULT_BASE_URL.to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        let model = if model.is_empty() {
            "default".to_string()
        } else {
            model
        };
        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .expect("Failed to build HTTP client");
        let stream_client = Client::builder()
            .timeout(Duration::from_secs(STREAM_TIMEOUT_SECS))
            .build()
            .expect("Failed to build stream HTTP client");
        Self {
            client,
            stream_client,
            base_url,
            api_key,
            model,
        }
    }

    fn auth_header(&self) -> Option<String> {
        if self.api_key.is_empty() {
            None
        } else {
            Some(format!("Bearer {}", self.api_key))
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
            let mut tool_calls_array: Vec<Value> = Vec::new();
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
                    ChatContent::ToolUse {
                        id,
                        name,
                        arguments,
                    } => {
                        // OpenAI spec: arguments is a STRINGIFIED JSON,
                        // not a raw JSON object. Stringify here so the
                        // wire format is correct for LM Studio / OpenAI
                        // / vLLM / llama.cpp servers.
                        let args_str = serde_json::to_string(arguments)
                            .unwrap_or_else(|_| "{}".to_string());
                        tool_calls_array.push(json!({
                            "id": id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": args_str,
                            },
                        }));
                    }
                }
            }

            let text = text_parts.join("");
            let has_content = !text.is_empty() || has_image;
            let has_tool_calls = !tool_calls_array.is_empty();
            if has_content || has_tool_calls {
                let mut msg = if has_image {
                    // Array form: interleave text block (if any) then image blocks.
                    let mut content_arr: Vec<Value> = Vec::new();
                    if !text.is_empty() {
                        content_arr.push(json!({"type": "text", "text": text}));
                    }
                    content_arr.extend(image_blocks);
                    json!({"role": m.role, "content": content_arr})
                } else if has_tool_calls && text.is_empty() {
                    // Assistant tool-call-only turn: OpenAI accepts null content
                    // when tool_calls is present.
                    json!({"role": m.role, "content": Value::Null})
                } else {
                    // Plain string form for text-only messages.
                    json!({"role": m.role, "content": text})
                };
                if has_tool_calls {
                    msg["tool_calls"] = json!(tool_calls_array);
                }
                out.push(msg);
            }
            out.extend(tool_results);
        }
        json!(out)
    }

    async fn do_chat(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> Result<LlmResponse, LlmError> {
        self.do_chat_with_timeout(system_prompt, messages, None, &Default::default())
            .await
    }

    /// Core blocking implementation. Both `do_chat` (with `None`) and
    /// `chat_with_options_dyn` (with `options.timeout`) delegate here.
    ///
    /// When `request_timeout` is `Some(d)`, `d` replaces the `client`
    /// ceiling for this request only (per-call override).
    async fn do_chat_with_timeout(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
        request_timeout: Option<Duration>,
        options: &LlmRequestOptions,
    ) -> Result<LlmResponse, LlmError> {
        let url = format!("{}/chat/completions", self.base_url);
        let mut body = json!({
            "model": self.model,
            "messages": Self::build_messages(system_prompt, messages),
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

        // Encode tools when present.
        if !options.tools.is_empty() {
            body["tools"] = json!(
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
            );
        }

        // Encode tool_choice when present.
        if let Some(choice) = &options.tool_choice {
            body["tool_choice"] = match choice {
                ToolChoice::Auto => json!("auto"),
                ToolChoice::Required => json!("required"),
                ToolChoice::None => json!("none"),
                ToolChoice::Specific(name) => {
                    json!({"type": "function", "function": {"name": name}})
                }
            };
        }

        let mut req = self.client.post(&url).json(&body);
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", auth);
        }
        if let Some(d) = request_timeout {
            req = req.timeout(d);
        }

        let resp = req.send().await.map_err(|e| {
            let msg = if e.is_timeout() {
                "timeout".to_string()
            } else {
                e.to_string()
            };
            LlmError::Network {
                provider: "openai_compat".into(),
                msg,
            }
        })?;

        let status = resp.status();
        let body_text = resp.text().await.map_err(|e| LlmError::Network {
            provider: "openai_compat".into(),
            msg: e.to_string(),
        })?;

        if status == 401 || status == 403 {
            return Err(LlmError::Auth {
                provider: "openai_compat".into(),
            });
        }
        if status == 429 {
            return Err(LlmError::RateLimit {
                provider: "openai_compat".into(),
                retry_after_secs: None,
            });
        }
        if !status.is_success() {
            return Err(LlmError::Upstream {
                provider: "openai_compat".into(),
                status: status.as_u16(),
                body: body_text,
            });
        }

        let json: Value = serde_json::from_str(&body_text).map_err(|e| LlmError::Parse {
            provider: "openai_compat".into(),
            reason: e.to_string(),
        })?;

        let text = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let model = json["model"].as_str().unwrap_or(&self.model).to_string();
        let input_tokens = json["usage"]["prompt_tokens"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok());
        let output_tokens = json["usage"]["completion_tokens"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok());

        // Extract reasoning_content from o-series models (Chat Completions API).
        let thinking_text: Option<String> = json["choices"][0]["message"]["reasoning_content"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        // Parse tool_calls from the response message.
        let tool_calls: Vec<ToolCall> = json["choices"][0]["message"]["tool_calls"]
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
            .unwrap_or_default();

        Ok(LlmResponse {
            rendered_text: text,
            model,
            estimated_cost_usd: 0.0,
            input_tokens,
            output_tokens,
            tool_calls,
            thinking_text,
            ..Default::default()
        })
    }

    async fn do_stream(
        provider: std::sync::Arc<Self>,
        system_prompt: String,
        messages: Vec<ChatMessage>,
    ) -> BoxStream<'static, LlmStreamEvent> {
        Self::do_stream_with_timeout(provider, system_prompt, messages, None, Default::default())
            .await
    }

    /// Core streaming implementation. Both `do_stream` (with `None`) and
    /// `chat_stream_with_options_dyn` (with `options.timeout`) delegate here.
    ///
    /// When `request_timeout` is `Some(d)`, `d` replaces the `stream_client`
    /// ceiling for this request only (per-call override).
    async fn do_stream_with_timeout(
        provider: std::sync::Arc<Self>,
        system_prompt: String,
        messages: Vec<ChatMessage>,
        request_timeout: Option<Duration>,
        options: LlmRequestOptions,
    ) -> BoxStream<'static, LlmStreamEvent> {
        let url = format!("{}/chat/completions", provider.base_url);
        let mut body = json!({
            "model": provider.model,
            "messages": Self::build_messages(&system_prompt, &messages),
            "stream": true,
            "stream_options": {"include_usage": true},
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

        // Encode tools when present.
        if !options.tools.is_empty() {
            body["tools"] = json!(
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
            );
        }

        // Encode tool_choice when present.
        if let Some(ref choice) = options.tool_choice {
            body["tool_choice"] = match choice {
                ToolChoice::Auto => json!("auto"),
                ToolChoice::Required => json!("required"),
                ToolChoice::None => json!("none"),
                ToolChoice::Specific(name) => {
                    json!({"type": "function", "function": {"name": name}})
                }
            };
        }

        let mut req = provider.stream_client.post(&url).json(&body);
        if let Some(auth) = provider.auth_header() {
            req = req.header("Authorization", auth);
        }
        if let Some(d) = request_timeout {
            req = req.timeout(d);
        }

        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                let msg = if e.is_timeout() {
                    "timeout".to_string()
                } else {
                    e.to_string()
                };
                return Box::pin(futures::stream::once(async move {
                    LlmStreamEvent::Error { message: msg }
                }));
            }
        };

        let status = resp.status();
        if !status.is_success() {
            let msg = resp.text().await.unwrap_or_default();
            return Box::pin(futures::stream::once(async move {
                LlmStreamEvent::Error {
                    message: format!("HTTP {status}: {msg}"),
                }
            }));
        }

        let model_name = provider.model.clone();
        let mut stream = resp.bytes_stream();

        Box::pin(async_stream::stream! {
            let mut text = String::new();
            let mut model = model_name.clone();
            let mut buf = String::new();
            let mut input_tokens: Option<u32> = None;
            let mut output_tokens: Option<u32> = None;
            // Accumulates reasoning_content deltas from o-series models.
            let mut thinking_acc = String::new();

            // Per-index accumulator for tool_call deltas.
            // index → (id, name, accumulated_args_str)
            let mut tool_call_accum: BTreeMap<u32, (String, String, String)> = BTreeMap::new();
            // Completed tool calls, carried into the End event.
            let mut emitted_tool_calls: Vec<ToolCall> = Vec::new();

            while let Some(chunk) = stream.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        let msg = if e.is_timeout() { "timeout".to_string() } else { e.to_string() };
                        yield LlmStreamEvent::Error { message: msg };
                        return;
                    }
                };
                buf.push_str(&String::from_utf8_lossy(&chunk));

                // Process complete lines, keeping any incomplete tail.
                let mut start = 0;
                let mut end = 0;
                while end < buf.len() {
                    if buf.as_bytes()[end] == b'\n' {
                        let line = buf[start..end].trim();
                        if line == "data: [DONE]" {
                            // Drain any remaining accumulated tool calls in key-sorted order.
                            for (_, (id, name, args_str)) in std::mem::take(&mut tool_call_accum) {
                                if let Ok(arguments) = serde_json::from_str::<Value>(&args_str) {
                                    let call = ToolCall { id, name, arguments };
                                    emitted_tool_calls.push(call.clone());
                                    yield LlmStreamEvent::ToolCall { call };
                                }
                            }
                            break;
                        }
                        if let Some(data) = line.strip_prefix("data: ") {
                            if let Ok(json) = serde_json::from_str::<Value>(data) {
                                if let Some(m) = json["model"].as_str() {
                                    model = m.to_string();
                                }

                                // Extract reasoning_content delta from o-series models.
                                if let Some(reasoning) = json["choices"][0]["delta"]["reasoning_content"].as_str() {
                                    if !reasoning.is_empty() {
                                        thinking_acc.push_str(reasoning);
                                        yield LlmStreamEvent::Thinking { text: reasoning.to_string() };
                                    }
                                }

                                if let Some(delta) = json["choices"][0]["delta"]["content"].as_str() {
                                    text.push_str(delta);
                                    yield LlmStreamEvent::Delta { text: delta.to_string() };
                                }

                                // Accumulate tool_call deltas.
                                if let Some(tcs) = json["choices"][0]["delta"]["tool_calls"].as_array() {
                                    for tc in tcs {
                                        let idx = tc["index"].as_u64().unwrap_or(0) as u32;
                                        let entry = tool_call_accum
                                            .entry(idx)
                                            .or_insert_with(|| (String::new(), String::new(), String::new()));
                                        if let Some(s) = tc["id"].as_str() {
                                            entry.0 = s.to_string();
                                        }
                                        if let Some(s) = tc["function"]["name"].as_str() {
                                            entry.1 = s.to_string();
                                        }
                                        if let Some(s) = tc["function"]["arguments"].as_str() {
                                            entry.2.push_str(s);
                                        }
                                    }
                                }

                                // On finish_reason: "tool_calls", emit accumulated tool calls in key-sorted order.
                                if json["choices"][0]["finish_reason"].as_str() == Some("tool_calls") {
                                    for (_, (id, name, args_str)) in std::mem::take(&mut tool_call_accum) {
                                        if let Ok(arguments) = serde_json::from_str::<Value>(&args_str) {
                                            let call = ToolCall { id, name, arguments };
                                            emitted_tool_calls.push(call.clone());
                                            yield LlmStreamEvent::ToolCall { call };
                                        }
                                    }
                                }

                                // Capture usage chunk (final SSE message before [DONE]).
                                if let Some(usage) = json.get("usage") {
                                    if let Some(n) = usage.get("prompt_tokens").and_then(|v| v.as_u64()) {
                                        input_tokens = u32::try_from(n).ok();
                                    }
                                    if let Some(n) = usage.get("completion_tokens").and_then(|v| v.as_u64()) {
                                        output_tokens = u32::try_from(n).ok();
                                    }
                                    // Emit Usage event so callers can display live cost.
                                    // openai_compat is a generic endpoint — no built-in pricing table,
                                    // so estimated_cost_usd is always 0.0.
                                    yield LlmStreamEvent::Usage {
                                        input_tokens,
                                        output_tokens: output_tokens.unwrap_or(0),
                                        estimated_cost_usd: 0.0,
                                    };
                                }
                            }
                        }
                        start = end + 1;
                    }
                    end += 1;
                }
                buf = buf[start..].to_string();
            }

            let thinking_text = if thinking_acc.is_empty() { None } else { Some(thinking_acc) };
            yield LlmStreamEvent::End {
                total: LlmResponse {
                    rendered_text: text,
                    model,
                    estimated_cost_usd: 0.0,
                    input_tokens,
                    output_tokens,
                    tool_calls: emitted_tool_calls,
                    thinking_text,
                    ..Default::default()
                },
            };
        })
    }
}

impl LlmProviderDyn for OpenAICompatProvider {
    fn render_dyn(
        &self,
        system_prompt: &str,
        user_input: &str,
        _result_json: &Value,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + '_>,
    > {
        let system = system_prompt.to_string();
        let msgs = vec![ChatMessage::user(user_input)];
        Box::pin(async move { self.do_chat(&system, &msgs).await })
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
        Box::pin(async move { self.do_chat(&system, &msgs).await })
    }

    fn chat_stream_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        _result_json: &'a Value,
    ) -> BoxStream<'a, LlmStreamEvent> {
        let provider = std::sync::Arc::new(OpenAICompatProvider::new(
            self.base_url.clone(),
            self.api_key.clone(),
            self.model.clone(),
        ));
        let system = system_prompt.to_string();
        let msgs = messages.to_vec();
        Box::pin(async_stream::stream! {
            let inner = Self::do_stream(provider, system, msgs).await;
            futures::pin_mut!(inner);
            while let Some(ev) = inner.next().await {
                yield ev;
            }
        })
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
        let timeout = options.timeout;
        let opts = options.clone();
        let system = system_prompt.to_string();
        let msgs = messages.to_vec();
        Box::pin(async move {
            self.do_chat_with_timeout(&system, &msgs, timeout, &opts)
                .await
        })
    }

    fn chat_stream_with_options_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        options: &'a LlmRequestOptions,
        _result_json: &'a Value,
    ) -> BoxStream<'a, LlmStreamEvent> {
        let timeout = options.timeout;
        let opts = options.clone();
        let provider = std::sync::Arc::new(OpenAICompatProvider::new(
            self.base_url.clone(),
            self.api_key.clone(),
            self.model.clone(),
        ));
        let system = system_prompt.to_string();
        let msgs = messages.to_vec();
        Box::pin(async_stream::stream! {
            let inner = Self::do_stream_with_timeout(provider, system, msgs, timeout, opts).await;
            futures::pin_mut!(inner);
            while let Some(ev) = inner.next().await {
                yield ev;
            }
        })
    }
}

pub fn make(
    api_key: &str,
    model: &str,
    base_url: &str,
) -> Result<Box<dyn LlmProviderDyn>, LlmError> {
    if base_url.is_empty() {
        return Err(LlmError::MissingConfig {
            provider: "openai_compat".into(),
            reason: "base_url is required".into(),
        });
    }
    Ok(Box::new(OpenAICompatProvider::new(
        base_url.to_string(),
        api_key.to_string(),
        model.to_string(),
    )))
}

pub async fn list_models(base_url: &str, api_key: &str) -> Result<Vec<crate::ListModel>, LlmError> {
    if base_url.is_empty() {
        return Err(LlmError::MissingConfig {
            provider: "openai_compat".into(),
            reason: "base_url is required".into(),
        });
    }
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| LlmError::Network {
            provider: "openai_compat".into(),
            msg: e.to_string(),
        })?;

    let mut req = client.get(&url);
    if !api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {api_key}"));
    }

    let resp = req.send().await.map_err(|e| LlmError::Network {
        provider: "openai_compat".into(),
        msg: e.to_string(),
    })?;

    let status = resp.status();
    let body = resp.text().await.map_err(|e| LlmError::Network {
        provider: "openai_compat".into(),
        msg: e.to_string(),
    })?;

    if status == 401 || status == 403 {
        return Err(LlmError::Auth {
            provider: "openai_compat".into(),
        });
    }
    if !status.is_success() {
        return Err(LlmError::Upstream {
            provider: "openai_compat".into(),
            status: status.as_u16(),
            body,
        });
    }

    let json: Value = serde_json::from_str(&body).map_err(|e| LlmError::Parse {
        provider: "openai_compat".into(),
        reason: e.to_string(),
    })?;

    let models = json["data"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let name = m["id"].as_str()?.to_string();
                    Some(crate::ListModel {
                        display_name: name.clone(),
                        name,
                        description: None,
                        input_token_limit: None,
                        output_token_limit: None,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn make_requires_base_url() {
        assert!(make("", "gpt-4", "").is_err());
    }

    #[test]
    fn make_with_base_url_succeeds() {
        assert!(make("", "gpt-4", "http://localhost:11434/v1").is_ok());
    }

    #[test]
    fn make_empty_model_uses_default() {
        let p = OpenAICompatProvider::new("http://localhost/v1".into(), "".into(), "".into());
        assert_eq!(p.model, "default");
    }

    #[test]
    fn trailing_slash_stripped_from_base_url() {
        let p = OpenAICompatProvider::new("http://localhost/v1/".into(), "".into(), "m".into());
        assert_eq!(p.base_url, "http://localhost/v1");
    }

    #[tokio::test]
    async fn chat_returns_parsed_text() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "model": "test-model",
                "choices": [{"message": {"role": "assistant", "content": "hello"}}],
                "usage": {"prompt_tokens": 10, "completion_tokens": 5},
            })))
            .mount(&server)
            .await;

        let p = OpenAICompatProvider::new(server.uri(), "".into(), "test-model".into());
        let msgs = vec![ChatMessage::user("hi")];
        let resp = p.do_chat("", &msgs).await.unwrap();
        assert_eq!(resp.rendered_text, "hello");
        assert_eq!(resp.model, "test-model");
        assert_eq!(resp.input_tokens, Some(10));
        assert_eq!(resp.output_tokens, Some(5));
    }

    #[tokio::test]
    async fn chat_401_maps_to_auth_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let p = OpenAICompatProvider::new(server.uri(), "bad-key".into(), "m".into());
        let msgs = vec![ChatMessage::user("hi")];
        assert!(matches!(
            p.do_chat("", &msgs).await.unwrap_err(),
            LlmError::Auth { .. }
        ));
    }

    #[tokio::test]
    async fn chat_429_maps_to_rate_limit() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;

        let p = OpenAICompatProvider::new(server.uri(), "".into(), "m".into());
        let msgs = vec![ChatMessage::user("hi")];
        assert!(matches!(
            p.do_chat("", &msgs).await.unwrap_err(),
            LlmError::RateLimit { .. }
        ));
    }

    #[tokio::test]
    async fn list_models_returns_parsed_ids() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {"id": "llama3", "object": "model"},
                    {"id": "mistral", "object": "model"},
                ]
            })))
            .mount(&server)
            .await;

        let models = list_models(&server.uri(), "").await.unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].name, "llama3");
        assert_eq!(models[1].name, "mistral");
    }

    #[tokio::test]
    async fn list_models_sends_auth_header() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .and(header("Authorization", "Bearer my-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{"id": "gpt-4"}]
            })))
            .mount(&server)
            .await;

        let models = list_models(&server.uri(), "my-key").await.unwrap();
        assert_eq!(models.len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sampling_params_appear_in_request_body() {
        use wiremock::matchers::body_partial_json;

        let server = MockServer::start().await;
        // Use exact f32-representable values (0.5 = 2^-1, 0.25 = 2^-2) so that
        // f32 serialization matches the f64 literal in the partial-JSON matcher.
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(body_partial_json(serde_json::json!({
                "temperature": 0.5,
                "max_tokens": 256,
                "top_p": 0.25,
                "stop": ["END"]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "model": "test-model",
                "choices": [{"message": {"role": "assistant", "content": "ok"}}],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1}
            })))
            .mount(&server)
            .await;

        let p = OpenAICompatProvider::new(server.uri(), "k".into(), "test-model".into());
        let options = LlmRequestOptions {
            temperature: Some(0.5),
            max_tokens: Some(256),
            top_p: Some(0.25),
            stop_sequences: vec!["END".into()],
            ..Default::default()
        };
        let msgs = vec![crate::ChatMessage::user("x")];
        use crate::LlmProviderDyn;
        let result = p
            .chat_with_options_dyn("", &msgs, &options, &serde_json::json!({}))
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    #[tokio::test]
    async fn timeout_option_aborts_slow_blocking_call() {
        let server = MockServer::start().await;
        // Respond with a 2-second delay — longer than our 500ms timeout.
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(std::time::Duration::from_secs(2))
                    .set_body_json(serde_json::json!({
                        "model": "test-model",
                        "choices": [{"message": {"role": "assistant", "content": "late"}}],
                        "usage": {"prompt_tokens": 1, "completion_tokens": 1},
                    })),
            )
            .mount(&server)
            .await;

        let p = make("k", "test-model", &server.uri()).expect("make");
        let msgs = vec![ChatMessage::user("hi")];
        let options = LlmRequestOptions {
            timeout: Some(Duration::from_millis(500)),
            ..Default::default()
        };

        let t0 = Instant::now();
        let err = p
            .chat_with_options_dyn("", &msgs, &options, &serde_json::json!({}))
            .await
            .unwrap_err();
        let elapsed = t0.elapsed();

        assert!(
            elapsed < Duration::from_millis(1500),
            "expected abort < 1.5s, took {elapsed:?}"
        );
        match err {
            LlmError::Network { provider, msg } => {
                assert_eq!(provider, "openai_compat");
                assert!(
                    msg.contains("timeout"),
                    "expected msg to contain 'timeout', got: {msg}"
                );
            }
            other => panic!("expected Network error, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn timeout_option_aborts_slow_stream() {
        let server = MockServer::start().await;
        // Respond with a 2-second delay before the first byte.
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(std::time::Duration::from_secs(2))
                    .set_body_string("data: [DONE]\n\n"),
            )
            .mount(&server)
            .await;

        let p = make("k", "test-model", &server.uri()).expect("make");
        let msgs = vec![ChatMessage::user("hi")];
        let options = LlmRequestOptions {
            timeout: Some(Duration::from_millis(500)),
            ..Default::default()
        };

        let t0 = Instant::now();
        let result_json = serde_json::json!({});
        let mut stream = p.chat_stream_with_options_dyn("", &msgs, &options, &result_json);

        let first = stream.next().await.expect("expected at least one event");
        let elapsed = t0.elapsed();

        assert!(
            elapsed < Duration::from_millis(1500),
            "expected abort < 1.5s, took {elapsed:?}"
        );
        match first {
            LlmStreamEvent::Error { message } => {
                assert!(
                    message.contains("timeout"),
                    "expected message to contain 'timeout', got: {message}"
                );
            }
            other => {
                let label = match other {
                    LlmStreamEvent::Delta { .. } => "Delta",
                    LlmStreamEvent::End { .. } => "End",
                    LlmStreamEvent::Error { .. } => "Error",
                    _ => "Unknown",
                };
                panic!("expected Error event, got: {label}");
            }
        }
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Task 6: Tool calling tests
    // ──────────────────────────────────────────────────────────────────────────

    /// Test A: blocking response with `tool_calls` is parsed into
    /// `LlmResponse.tool_calls`.
    #[tokio::test(flavor = "multi_thread")]
    async fn tool_call_in_blocking_response_returns_tool_calls() {
        use crate::{LlmProviderDyn, LlmRequestOptions, ToolDef};

        let fixture = include_str!("../tests/fixtures/openai_tool_call_response.json");

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(fixture)
                    .insert_header("content-type", "application/json"),
            )
            .mount(&server)
            .await;

        let p = OpenAICompatProvider::new(server.uri(), "".into(), "gpt-4".into());
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

        let resp = p
            .chat_with_options_dyn(
                "",
                &[ChatMessage::user("What's the weather in Seoul?")],
                &opts,
                &serde_json::json!({}),
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

    /// Test B: `tools` array appears in the serialised request body in
    /// OpenAI Chat Completions format.
    #[tokio::test(flavor = "multi_thread")]
    async fn tools_appear_in_request_body() {
        use crate::{LlmProviderDyn, LlmRequestOptions, ToolDef};
        use wiremock::matchers::body_partial_json;

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
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
                "model": "gpt-4",
                "choices": [{"message": {"role": "assistant", "content": "ok"}}],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1}
            })))
            .mount(&server)
            .await;

        let p = OpenAICompatProvider::new(server.uri(), "".into(), "gpt-4".into());
        let opts = LlmRequestOptions {
            tools: vec![ToolDef {
                name: "get_weather".into(),
                description: "Get weather for a city".into(),
                input_schema: serde_json::json!({"type": "object", "properties": {}}),
            }],
            ..Default::default()
        };
        let result = p
            .chat_with_options_dyn(
                "",
                &[ChatMessage::user("hi")],
                &opts,
                &serde_json::json!({}),
            )
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    /// Test C: `tool_choice: Required` serialises as the string `"required"`.
    #[tokio::test(flavor = "multi_thread")]
    async fn tool_choice_required_appears_as_string_required() {
        use crate::{LlmProviderDyn, LlmRequestOptions, ToolChoice, ToolDef};
        use wiremock::matchers::body_partial_json;

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(body_partial_json(serde_json::json!({
                "tool_choice": "required"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "model": "gpt-4",
                "choices": [{"message": {"role": "assistant", "content": "ok"}}],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1}
            })))
            .mount(&server)
            .await;

        let p = OpenAICompatProvider::new(server.uri(), "".into(), "gpt-4".into());
        let opts = LlmRequestOptions {
            tools: vec![ToolDef {
                name: "get_weather".into(),
                description: "dummy".into(),
                input_schema: serde_json::json!({"type": "object"}),
            }],
            tool_choice: Some(ToolChoice::Required),
            ..Default::default()
        };
        let result = p
            .chat_with_options_dyn(
                "",
                &[ChatMessage::user("hi")],
                &opts,
                &serde_json::json!({}),
            )
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    /// Round 20 regression lock: `ChatMessage::assistant_with_tool_calls`
    /// encodes the assistant turn with a `tool_calls` array, and arguments
    /// are stringified per the OpenAI/LM Studio wire spec. The subsequent
    /// `ChatMessage::tool_result` then renders as a separate `role:"tool"`
    /// message whose `tool_call_id` matches — completing the round-trip
    /// LM Studio multi-turn loops require.
    #[tokio::test(flavor = "multi_thread")]
    async fn assistant_tool_calls_arguments_are_stringified_round_trip() {
        use crate::{ChatContent, LlmProviderDyn, LlmRequestOptions, ToolCall as TC};
        use wiremock::matchers::body_partial_json;

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(body_partial_json(serde_json::json!({
                "messages": [
                    {"role": "user", "content": "weather?"},
                    {
                        "role": "assistant",
                        "content": "checking",
                        "tool_calls": [{
                            "id": "call_01",
                            "type": "function",
                            "function": {
                                "name": "get_weather",
                                "arguments": "{\"city\":\"seoul\"}"
                            }
                        }]
                    },
                    {"role": "tool", "tool_call_id": "call_01", "content": "25C"},
                ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "model": "gpt-4",
                "choices": [{"message": {"role": "assistant", "content": "It is 25C."}}],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1}
            })))
            .mount(&server)
            .await;

        let p = OpenAICompatProvider::new(server.uri(), "".into(), "gpt-4".into());
        let messages = vec![
            ChatMessage::user("weather?"),
            ChatMessage::assistant_with_tool_calls(
                "checking",
                vec![TC {
                    id: "call_01".into(),
                    name: "get_weather".into(),
                    arguments: serde_json::json!({"city": "seoul"}),
                }],
            ),
            ChatMessage::tool_result("call_01", "25C", false),
        ];
        let result = p
            .chat_with_options_dyn(
                "",
                &messages,
                &LlmRequestOptions::default(),
                &serde_json::json!({}),
            )
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");

        // Sanity: the variant is publicly constructable for downstream
        // multi-turn loops. (Direct construction is allowed inside this
        // crate; external crates use ChatMessage::assistant_with_tool_calls.)
        let _ = ChatContent::ToolUse {
            id: "x".into(),
            name: "y".into(),
            arguments: serde_json::json!({}),
        };
    }

    /// Test D: `ChatMessage::tool_result` encodes as a separate `{role: "tool",
    /// tool_call_id, content}` message (OpenAI wire-format, not a content block).
    #[tokio::test(flavor = "multi_thread")]
    async fn tool_result_message_role_serializes_correctly() {
        use crate::{LlmProviderDyn, LlmRequestOptions};
        use wiremock::matchers::body_partial_json;

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
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
                "model": "gpt-4",
                "choices": [{"message": {"role": "assistant", "content": "ok"}}],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1}
            })))
            .mount(&server)
            .await;

        let p = OpenAICompatProvider::new(server.uri(), "".into(), "gpt-4".into());
        let messages = vec![ChatMessage::tool_result("call_01", "25C", false)];
        let result = p
            .chat_with_options_dyn(
                "",
                &messages,
                &LlmRequestOptions::default(),
                &serde_json::json!({}),
            )
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    /// Test E: streaming SSE with `tool_calls` deltas emits
    /// `LlmStreamEvent::ToolCall` and the terminal `End` event carries the
    /// accumulated tool call.
    #[tokio::test(flavor = "multi_thread")]
    async fn tool_call_in_streaming_emits_toolcall_event() {
        use crate::{LlmProviderDyn, LlmRequestOptions, LlmStreamEvent, ToolCall as TC, ToolDef};
        use futures::StreamExt;

        // Build OpenAI-style SSE stream with tool_calls deltas.
        let sse_body = concat!(
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"Checking the weather.\"}}]}\n\n",
            // Initialize tool call slot: first delta has id, type, name + empty arguments.
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_01\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"\"}}]}}]}\n\n",
            // Accumulate arguments across two chunks.
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"city\\\":\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"seoul\\\"}\"}}]}}]}\n\n",
            // finish_reason triggers drain.
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: {\"usage\":{\"prompt_tokens\":50,\"completion_tokens\":20}}\n\n",
            "data: [DONE]\n\n",
        );

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(sse_body)
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;

        let p = OpenAICompatProvider::new(server.uri(), "".into(), "gpt-4".into());
        let opts = LlmRequestOptions {
            tools: vec![ToolDef {
                name: "get_weather".into(),
                description: "Get weather for a city".into(),
                input_schema: serde_json::json!({"type": "object"}),
            }],
            ..Default::default()
        };

        let msgs = vec![ChatMessage::user("weather?")];
        let result_json = serde_json::json!({});
        let mut stream = p.chat_stream_with_options_dyn("", &msgs, &opts, &result_json);

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
            "expected exactly one ToolCall event"
        );
        let call = &tool_call_events[0];
        assert_eq!(call.name, "get_weather");
        assert_eq!(call.id, "call_01");
        assert_eq!(
            call.arguments["city"], "seoul",
            "parsed arguments should include city=seoul"
        );

        let total = end_total.expect("expected End event");
        assert_eq!(
            total.tool_calls.len(),
            1,
            "End.total should carry accumulated tool calls"
        );
        assert_eq!(total.tool_calls[0].name, "get_weather");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn parallel_tool_calls_emit_in_index_order() {
        // Three parallel tool calls at indices 0, 1, 2 named "a", "b", "c".
        // SSE delta chunks arrive out of order: index 2 first, then 0, then 1.
        // After BTreeMap drain on finish_reason, ToolCall events and End.total.tool_calls
        // must come out in index order: a, b, c.
        use crate::{LlmProviderDyn, LlmRequestOptions, LlmStreamEvent, ToolDef};
        use futures::StreamExt;

        let sse_body = concat!(
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n",
            // Index 2 delta arrives first (out of order)
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":2,\"id\":\"id_c\",\"type\":\"function\",\"function\":{\"name\":\"c\",\"arguments\":\"\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":2,\"function\":{\"arguments\":\"{}\"}}]}}]}\n\n",
            // Index 0 delta arrives second
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"id_a\",\"type\":\"function\",\"function\":{\"name\":\"a\",\"arguments\":\"\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{}\"}}]}}]}\n\n",
            // Index 1 delta arrives last
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":1,\"id\":\"id_b\",\"type\":\"function\",\"function\":{\"name\":\"b\",\"arguments\":\"\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":1,\"function\":{\"arguments\":\"{}\"}}]}}]}\n\n",
            // finish_reason triggers drain — BTreeMap yields 0, 1, 2 order
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: {\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5}}\n\n",
            "data: [DONE]\n\n",
        );

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(sse_body)
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;

        let p = OpenAICompatProvider::new(server.uri(), "".into(), "gpt-4".into());
        let opts = LlmRequestOptions {
            tools: vec![
                ToolDef {
                    name: "a".into(),
                    description: "".into(),
                    input_schema: serde_json::json!({"type":"object"}),
                },
                ToolDef {
                    name: "b".into(),
                    description: "".into(),
                    input_schema: serde_json::json!({"type":"object"}),
                },
                ToolDef {
                    name: "c".into(),
                    description: "".into(),
                    input_schema: serde_json::json!({"type":"object"}),
                },
            ],
            ..Default::default()
        };

        let msgs = vec![ChatMessage::user("call all three")];
        let result_json = serde_json::json!({});
        let mut stream = p.chat_stream_with_options_dyn("", &msgs, &opts, &result_json);

        let mut tool_call_events: Vec<String> = Vec::new();
        let mut end_tool_calls: Vec<String> = Vec::new();

        while let Some(event) = stream.next().await {
            match event {
                LlmStreamEvent::ToolCall { call } => tool_call_events.push(call.name.clone()),
                LlmStreamEvent::End { total } => {
                    end_tool_calls = total.tool_calls.iter().map(|c| c.name.clone()).collect();
                    break;
                }
                LlmStreamEvent::Error { message } => panic!("unexpected error: {message}"),
                #[allow(unreachable_patterns)]
                _ => {}
            }
        }

        assert_eq!(
            tool_call_events,
            vec!["a", "b", "c"],
            "ToolCall events must be emitted in index order regardless of SSE arrival order"
        );
        assert_eq!(
            end_tool_calls,
            vec!["a", "b", "c"],
            "End.total.tool_calls must be in index order"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Task 11: HTTP image input tests
    // ──────────────────────────────────────────────────────────────────────────

    /// Test A: base64 image block serializes as `image_url` content block with a
    /// data URL (`data:<mime>;base64,<data>`) in array-form message content.
    #[tokio::test(flavor = "multi_thread")]
    async fn image_base64_block_serializes_correctly() {
        use crate::{ChatContent, ChatMessage, ImageSource, LlmProviderDyn, LlmRequestOptions};
        use wiremock::matchers::body_partial_json;

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
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
                "model": "gpt-4o",
                "choices": [{"message": {"role": "assistant", "content": "a red square"}}],
                "usage": {"prompt_tokens": 10, "completion_tokens": 5}
            })))
            .mount(&server)
            .await;

        let p = OpenAICompatProvider::new(server.uri(), "".into(), "gpt-4o".into());
        let msg = ChatMessage::user_multipart(vec![
            ChatContent::Text("describe".into()),
            ChatContent::Image {
                source: ImageSource::Base64("AAAA".into()),
                mime_type: "image/png".into(),
            },
        ]);
        let result = p
            .chat_with_options_dyn(
                "",
                &[msg],
                &LlmRequestOptions::default(),
                &serde_json::json!({}),
            )
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    /// Test B: URL image block serializes as `image_url` content block with the
    /// plain HTTPS URL (no base64 encoding, no mime prefix).
    #[tokio::test(flavor = "multi_thread")]
    async fn image_url_block_serializes_correctly() {
        use crate::{ChatContent, ChatMessage, ImageSource, LlmProviderDyn, LlmRequestOptions};
        use wiremock::matchers::body_partial_json;

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
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
                "model": "gpt-4o",
                "choices": [{"message": {"role": "assistant", "content": "a photo"}}],
                "usage": {"prompt_tokens": 10, "completion_tokens": 3}
            })))
            .mount(&server)
            .await;

        let p = OpenAICompatProvider::new(server.uri(), "".into(), "gpt-4o".into());
        let msg = ChatMessage::user_multipart(vec![ChatContent::Image {
            source: ImageSource::Url("https://example.com/photo.jpg".into()),
            mime_type: "image/jpeg".into(),
        }]);
        let result = p
            .chat_with_options_dyn(
                "",
                &[msg],
                &LlmRequestOptions::default(),
                &serde_json::json!({}),
            )
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Task 7: o-series reasoning_content extraction tests
    // ──────────────────────────────────────────────────────────────────────────

    /// Test A: blocking response with `reasoning_content` populates
    /// `LlmResponse.thinking_text`. Text-only responses leave it `None`.
    #[tokio::test]
    async fn reasoning_content_in_blocking_response_populates_thinking_text() {
        let fixture = include_str!("../tests/fixtures/openai_reasoning_response.json");

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(fixture)
                    .insert_header("content-type", "application/json"),
            )
            .mount(&server)
            .await;

        let p = OpenAICompatProvider::new(server.uri(), "".into(), "o4-mini".into());
        let msgs = vec![ChatMessage::user("Explain photosynthesis.")];
        let resp = p.do_chat("", &msgs).await.expect("expected Ok response");

        assert_eq!(resp.rendered_text, "Final answer.");
        assert_eq!(
            resp.thinking_text,
            Some("Step 1: analyze. Step 2: conclude.".to_string()),
            "reasoning_content should be mapped to thinking_text"
        );
        assert_eq!(resp.input_tokens, Some(50));
        assert_eq!(resp.output_tokens, Some(30));

        // Sanity-check: a response without reasoning_content leaves thinking_text as None.
        let server2 = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "model": "gpt-4o",
                "choices": [{"message": {"role": "assistant", "content": "hello"}}],
                "usage": {"prompt_tokens": 5, "completion_tokens": 2}
            })))
            .mount(&server2)
            .await;

        let p2 = OpenAICompatProvider::new(server2.uri(), "".into(), "gpt-4o".into());
        let resp2 = p2.do_chat("", &msgs).await.expect("expected Ok response");
        assert!(
            resp2.thinking_text.is_none(),
            "text-only model should have thinking_text = None"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Task 11: Streaming usage event tests
    // ──────────────────────────────────────────────────────────────────────────

    /// When the SSE stream contains a usage chunk (empty `choices`, top-level
    /// `usage` field) followed by `[DONE]`, the provider must emit exactly one
    /// `LlmStreamEvent::Usage` event before the terminal `End`, and
    /// `End.total.input_tokens` / `output_tokens` must reflect those counts.
    #[tokio::test(flavor = "multi_thread")]
    async fn streaming_usage_event_emitted_before_end() {
        use crate::{LlmProviderDyn, LlmRequestOptions, LlmStreamEvent};
        use futures::StreamExt;

        let sse_body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\" there\"}}]}\n\n",
            // Usage chunk: empty choices array + top-level usage field.
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"total_tokens\":15}}\n\n",
            "data: [DONE]\n\n",
        );

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(sse_body)
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;

        let p = OpenAICompatProvider::new(server.uri(), "".into(), "gpt-4".into());
        let msgs = vec![ChatMessage::user("hi")];
        let result_json = serde_json::json!({});
        let opts = LlmRequestOptions::default();
        let mut stream = p.chat_stream_with_options_dyn("", &msgs, &opts, &result_json);

        let mut usage_events: Vec<(Option<u32>, u32)> = Vec::new();
        let mut end_total: Option<LlmResponse> = None;
        // Track emission order so we can assert Usage precedes End.
        let mut saw_usage = false;
        let mut usage_before_end = false;

        while let Some(event) = stream.next().await {
            match event {
                LlmStreamEvent::Usage {
                    input_tokens,
                    output_tokens,
                    ..
                } => {
                    usage_events.push((input_tokens, output_tokens));
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

        assert_eq!(usage_events.len(), 1, "expected exactly one Usage event");
        assert_eq!(
            usage_events[0],
            (Some(10), 5),
            "Usage must carry prompt=10, completion=5"
        );
        assert!(usage_before_end, "Usage event must be emitted before End");

        let total = end_total.expect("expected End event");
        assert_eq!(
            total.input_tokens,
            Some(10),
            "End.total.input_tokens must be 10"
        );
        assert_eq!(
            total.output_tokens,
            Some(5),
            "End.total.output_tokens must be 5"
        );
    }

    /// Test B: streaming SSE with `reasoning_content` deltas emits
    /// `LlmStreamEvent::Thinking` events before `Delta` events, and the
    /// terminal `End` event has `total.thinking_text.is_some()`.
    #[tokio::test(flavor = "multi_thread")]
    async fn reasoning_content_in_streaming_emits_thinking_event_then_delta() {
        use crate::{LlmProviderDyn, LlmRequestOptions, LlmStreamEvent};
        use futures::StreamExt;

        let sse_body = concat!(
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"Step 1...\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"Final.\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        );

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(sse_body)
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;

        let p = OpenAICompatProvider::new(server.uri(), "".into(), "o4-mini".into());
        let msgs = vec![ChatMessage::user("Think step by step.")];
        let result_json = serde_json::json!({});
        let opts = LlmRequestOptions::default();
        let mut stream = p.chat_stream_with_options_dyn("", &msgs, &opts, &result_json);

        let mut thinking_events: Vec<String> = Vec::new();
        let mut delta_events: Vec<String> = Vec::new();
        let mut end_total: Option<LlmResponse> = None;
        // Track emission order: 'T' = Thinking, 'D' = Delta.
        let mut order: Vec<char> = Vec::new();

        while let Some(event) = stream.next().await {
            match event {
                LlmStreamEvent::Thinking { text } => {
                    thinking_events.push(text);
                    order.push('T');
                }
                LlmStreamEvent::Delta { text } => {
                    delta_events.push(text);
                    order.push('D');
                }
                LlmStreamEvent::End { total } => {
                    end_total = Some(total);
                    break;
                }
                LlmStreamEvent::Error { message } => panic!("unexpected error: {message}"),
                #[allow(unreachable_patterns)]
                _ => {}
            }
        }

        assert!(
            !thinking_events.is_empty(),
            "expected at least one Thinking event"
        );
        assert!(
            !delta_events.is_empty(),
            "expected at least one Delta event after reasoning"
        );

        // All Thinking events must precede any Delta event (as per the SSE order).
        let first_delta_pos = order
            .iter()
            .position(|&c| c == 'D')
            .expect("Delta must appear");
        let last_thinking_pos = order
            .iter()
            .rposition(|&c| c == 'T')
            .expect("Thinking must appear");
        assert!(
            last_thinking_pos < first_delta_pos,
            "all Thinking events must be emitted before any Delta event; order: {order:?}"
        );

        let total = end_total.expect("expected End event");
        assert!(
            total.thinking_text.is_some(),
            "End.total.thinking_text must be Some for o-series streaming"
        );
        assert_eq!(
            total.thinking_text.as_deref(),
            Some("Step 1..."),
            "accumulated thinking_text should match the reasoning_content chunks"
        );
        assert_eq!(
            total.rendered_text, "Final.",
            "rendered_text should contain the answer"
        );
    }
}
