use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Duration;

use super::{
    ChatContent, ChatMessage, LlmError, LlmProviderDyn, LlmResponse, LlmStreamEvent, ToolCall,
};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const REQUEST_TIMEOUT_SECS: u64 = 30;
const STREAM_TIMEOUT_SECS: u64 = 120;
const MAX_TOKENS: u32 = 1024;

pub struct ClaudeProvider {
    client: Client,
    stream_client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

/// Extended thinking configuration sent in the Anthropic request body.
/// Emitted only when `LlmRequestOptions.thinking_budget_tokens` is set.
#[derive(Serialize)]
struct ClaudeThinkingConfig {
    #[serde(rename = "type")]
    kind: String, // always "enabled"
    budget_tokens: u32,
}

#[derive(Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    system: Vec<SystemBlock>,
    messages: Vec<ClaudeMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop_sequences: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ClaudeToolDef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<ClaudeToolChoiceWire>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ClaudeThinkingConfig>,
}

/// Anthropic wire format for a tool definition.
#[derive(Serialize)]
struct ClaudeToolDef {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

/// Anthropic wire format for `tool_choice`.
///
/// Uses serde `tag = "type"` to emit `{"type": "auto"}`, `{"type": "any"}`,
/// or `{"type": "tool", "name": "..."}`.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ClaudeToolChoiceWire {
    Auto,
    Any,
    Tool { name: String },
}

/// Anthropic cache control marker — `{type: "ephemeral"}`.
/// Attached to system blocks and content blocks to enable prompt caching.
#[derive(Serialize, Deserialize, Clone)]
struct CacheControl {
    #[serde(rename = "type")]
    kind: String, // always "ephemeral"
}

impl CacheControl {
    fn ephemeral() -> Self {
        Self {
            kind: "ephemeral".into(),
        }
    }
}

#[derive(Serialize)]
struct SystemBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<CacheControl>,
}

#[derive(Serialize)]
struct ClaudeMessage {
    role: String,
    content: Vec<ContentBlock>,
}

/// Serializable content block — supports text, tool_result, image, and thinking.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlock {
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    Image {
        source: ClaudeImageSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    #[allow(dead_code)]
    Thinking { thinking: String },
}

/// Anthropic wire format for image source — either base64-encoded data or a URL.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClaudeImageSource {
    Base64 { media_type: String, data: String },
    Url { url: String },
}

#[derive(Deserialize)]
struct ClaudeResponse {
    content: Vec<ResponseBlock>,
    model: String,
    usage: Usage,
    stop_reason: String,
}

/// One block in an Anthropic Messages API response.
///
/// All typed fields are optional with serde defaults so the same struct handles
/// text, tool_use, and thinking block types. In practice:
/// - `type == "text"`: `text` is populated.
/// - `type == "tool_use"`: `id`, `name`, `input` are populated.
/// - `type == "thinking"`: `thinking` is populated.
#[derive(Deserialize)]
struct ResponseBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    text: String,
    // tool_use fields
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    input: Option<serde_json::Value>,
    // thinking field
    #[serde(default)]
    thinking: String,
}

#[derive(Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
    #[serde(default)]
    cache_creation_input_tokens: Option<u32>,
    #[serde(default)]
    cache_read_input_tokens: Option<u32>,
}

impl ClaudeProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self::new_with_base_url(api_key, model, ANTHROPIC_API_URL.to_string())
    }

    /// For testing only — allows pointing the provider at a mock HTTP server.
    /// Production callers should use [`Self::new`], which defaults to
    /// `ANTHROPIC_API_URL` (`https://api.anthropic.com/v1/messages`).
    pub fn new_with_base_url(api_key: String, model: String, base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .expect("Failed to build HTTP client");
        let stream_client = Client::builder()
            .timeout(Duration::from_secs(STREAM_TIMEOUT_SECS))
            .build()
            .expect("Failed to build streaming HTTP client");
        Self {
            client,
            stream_client,
            api_key,
            model,
            base_url,
        }
    }

    fn estimate_cost(model: &str, input_tokens: u32, output_tokens: u32) -> f64 {
        let (input_rate, output_rate) = match model {
            m if m.contains("haiku") => (0.25, 1.25),
            m if m.contains("sonnet") => (3.0, 15.0),
            m if m.contains("opus") => (15.0, 75.0),
            _ => (3.0, 15.0),
        };
        (input_tokens as f64 * input_rate + output_tokens as f64 * output_rate) / 1_000_000.0
    }

    fn build_messages(messages: &[ChatMessage]) -> Vec<ClaudeMessage> {
        messages
            .iter()
            .map(|m| {
                let blocks: Vec<ContentBlock> = m
                    .content
                    .iter()
                    .map(|block| match block {
                        ChatContent::Text(t) => ContentBlock::Text {
                            text: t.clone(),
                            cache_control: None,
                        },
                        ChatContent::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } => ContentBlock::ToolResult {
                            tool_use_id: tool_use_id.clone(),
                            content: content.clone(),
                            is_error: *is_error,
                            cache_control: None,
                        },
                        ChatContent::Image { source, mime_type } => {
                            use super::ImageSource;
                            let claude_source = match source {
                                ImageSource::Base64(data) => ClaudeImageSource::Base64 {
                                    media_type: mime_type.clone(),
                                    data: data.clone(),
                                },
                                ImageSource::Url(url) => {
                                    ClaudeImageSource::Url { url: url.clone() }
                                }
                            };
                            ContentBlock::Image {
                                source: claude_source,
                                cache_control: None,
                            }
                        }
                    })
                    .collect();
                ClaudeMessage {
                    role: m.role.clone(),
                    content: blocks,
                }
            })
            .collect()
    }

    /// Build the `tools` array for the Anthropic request from `LlmRequestOptions`.
    fn build_tools(options: &crate::LlmRequestOptions) -> Vec<ClaudeToolDef> {
        options
            .tools
            .iter()
            .map(|t| ClaudeToolDef {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
            })
            .collect()
    }

    /// Translate `ToolChoice` to the Anthropic wire representation.
    /// Returns `None` for `ToolChoice::None` — omit the field entirely so
    /// Anthropic falls back to its own default (auto when tools present).
    fn build_tool_choice(options: &crate::LlmRequestOptions) -> Option<ClaudeToolChoiceWire> {
        use crate::ToolChoice;
        match options.tool_choice.as_ref()? {
            ToolChoice::Auto => Some(ClaudeToolChoiceWire::Auto),
            ToolChoice::Required => Some(ClaudeToolChoiceWire::Any),
            ToolChoice::Specific(name) => Some(ClaudeToolChoiceWire::Tool { name: name.clone() }),
            ToolChoice::None => Option::None,
        }
    }

    /// Apply `cache_control: {type: "ephemeral"}` markers to the request body per
    /// `breakpoints`. Anthropic limit is 4; extras are truncated with a warning.
    /// Out-of-range `MessageIndex` values are skipped with a warning.
    fn apply_cache_breakpoints(req: &mut ClaudeRequest, breakpoints: &[crate::CacheBreakpoint]) {
        if breakpoints.is_empty() {
            return;
        }

        let effective: Vec<&crate::CacheBreakpoint> = if breakpoints.len() > 4 {
            tracing::warn!(
                "claude: {} cache_breakpoints provided; Anthropic limit is 4. Truncating to first 4.",
                breakpoints.len()
            );
            breakpoints.iter().take(4).collect()
        } else {
            breakpoints.iter().collect()
        };

        for bp in effective {
            match bp {
                crate::CacheBreakpoint::System => {
                    if req.system.is_empty() {
                        tracing::warn!(
                            "claude: CacheBreakpoint::System requested but system array is empty; skipping"
                        );
                        continue;
                    }
                    if let Some(last) = req.system.last_mut() {
                        last.cache_control = Some(CacheControl::ephemeral());
                    }
                }
                crate::CacheBreakpoint::MessageIndex(idx) => {
                    if *idx >= req.messages.len() {
                        tracing::warn!(
                            "claude: cache_breakpoint MessageIndex({}) out of range (messages.len = {}); skipping",
                            idx,
                            req.messages.len()
                        );
                        continue;
                    }
                    if let Some(last_block) = req.messages[*idx].content.last_mut() {
                        match last_block {
                            ContentBlock::Text { cache_control, .. } => {
                                *cache_control = Some(CacheControl::ephemeral());
                            }
                            ContentBlock::ToolResult { cache_control, .. } => {
                                *cache_control = Some(CacheControl::ephemeral());
                            }
                            ContentBlock::Image { cache_control, .. } => {
                                *cache_control = Some(CacheControl::ephemeral());
                            }
                            ContentBlock::Thinking { .. } => {
                                // Thinking blocks don't support cache_control.
                                tracing::warn!(
                                    "claude: cache_breakpoint MessageIndex({}) targets a message whose last content block is a Thinking block; cache_control not applicable; skipping",
                                    idx
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// Parse `tool_use` blocks from a non-streaming Anthropic response.
    fn parse_tool_calls(blocks: &[ResponseBlock]) -> Vec<ToolCall> {
        blocks
            .iter()
            .filter(|b| b.block_type == "tool_use")
            .map(|b| ToolCall {
                id: b.id.clone(),
                name: b.name.clone(),
                arguments: b.input.clone().unwrap_or(serde_json::Value::Null),
            })
            .collect()
    }

    /// Sends a streaming request, handling OAT 400 → haiku fallback.
    /// Returns (response_with_streaming_body, model_used).
    ///
    /// Non-success HTTP responses are mapped to typed `LlmError` variants
    /// (`Auth` for 401/403, `RateLimit` with `retry-after` for 429,
    /// `Upstream` for others) via `claude_http_error`, preserving the same
    /// error taxonomy as the non-streaming `chat_impl`.
    ///
    /// When `request_timeout` is `Some(d)`, `d` replaces the `stream_client`
    /// ceiling for this request only (per-call override).
    async fn send_streaming_request(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
        request_timeout: Option<Duration>,
        options: &crate::LlmRequestOptions,
    ) -> Result<(reqwest::Response, String), LlmError> {
        let mut body = ClaudeRequest {
            model: self.model.clone(),
            max_tokens: options.max_tokens.unwrap_or(MAX_TOKENS),
            system: vec![SystemBlock {
                block_type: "text".into(),
                text: system_prompt.to_string(),
                cache_control: None,
            }],
            messages: Self::build_messages(messages),
            stream: Some(true),
            temperature: options.temperature,
            top_p: options.top_p,
            stop_sequences: options.stop_sequences.clone(),
            tools: Self::build_tools(options),
            tool_choice: Self::build_tool_choice(options),
            thinking: options
                .thinking_budget_tokens
                .map(|n| ClaudeThinkingConfig {
                    kind: "enabled".into(),
                    budget_tokens: n,
                }),
        };
        Self::apply_cache_breakpoints(&mut body, &options.cache_breakpoints);

        let req_builder = self
            .stream_client
            .post(&self.base_url)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json");

        let req_builder = if self.api_key.starts_with("sk-ant-oat") {
            req_builder
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("anthropic-beta", "claude-code-20250219,oauth-2025-04-20")
                .header("user-agent", "claude-cli/2.1.77")
                .header("x-app", "cli")
        } else {
            req_builder.header("x-api-key", &self.api_key)
        };

        let req_builder = if options.thinking_budget_tokens.is_some() {
            req_builder.header("anthropic-beta", "interleaved-thinking-2025-05-14")
        } else {
            req_builder
        };

        let req_builder = req_builder.json(&body);
        let req_builder = if let Some(d) = request_timeout {
            req_builder.timeout(d)
        } else {
            req_builder
        };

        let resp = req_builder.send().await.map_err(|e| {
            let msg = if e.is_timeout() {
                "timeout".to_string()
            } else {
                e.to_string()
            };
            LlmError::Network {
                provider: "claude".into(),
                msg,
            }
        })?;

        let status = resp.status();

        // OAT 400 → Haiku fallback
        if status.as_u16() == 400 && self.api_key.starts_with("sk-ant-oat") {
            const HAIKU_FALLBACK: &str = "claude-haiku-4-5";
            tracing::warn!(
                original_model = %self.model,
                fallback_model = HAIKU_FALLBACK,
                "OAT 400 error (streaming), retrying with Haiku fallback"
            );
            let err_body = resp.text().await.unwrap_or_default();
            tracing::error!(body = %err_body, "Original 400 error body");

            let mut fallback_body = ClaudeRequest {
                model: HAIKU_FALLBACK.to_string(),
                max_tokens: options.max_tokens.unwrap_or(MAX_TOKENS),
                system: vec![SystemBlock {
                    block_type: "text".into(),
                    text: system_prompt.to_string(),
                    cache_control: None,
                }],
                messages: Self::build_messages(messages),
                stream: Some(true),
                temperature: options.temperature,
                top_p: options.top_p,
                stop_sequences: options.stop_sequences.clone(),
                tools: Self::build_tools(options),
                tool_choice: Self::build_tool_choice(options),
                thinking: options
                    .thinking_budget_tokens
                    .map(|n| ClaudeThinkingConfig {
                        kind: "enabled".into(),
                        budget_tokens: n,
                    }),
            };
            Self::apply_cache_breakpoints(&mut fallback_body, &options.cache_breakpoints);

            let fallback_req = self
                .stream_client
                .post(&self.base_url)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .header("content-type", "application/json")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("anthropic-beta", "claude-code-20250219,oauth-2025-04-20")
                .header("user-agent", "claude-cli/2.1.77")
                .header("x-app", "cli");

            let fallback_req = if options.thinking_budget_tokens.is_some() {
                fallback_req.header("anthropic-beta", "interleaved-thinking-2025-05-14")
            } else {
                fallback_req
            };

            let mut fallback_req = fallback_req.json(&fallback_body);
            if let Some(d) = request_timeout {
                fallback_req = fallback_req.timeout(d);
            }
            let fallback_resp = fallback_req.send().await.map_err(|e| LlmError::Network {
                provider: "claude".into(),
                msg: if e.is_timeout() {
                    "timeout".into()
                } else {
                    format!("haiku fallback: {e}")
                },
            })?;

            let fallback_status = fallback_resp.status();
            if !fallback_status.is_success() {
                let retry_after = crate::error::parse_retry_after(&fallback_resp);
                let fallback_err = fallback_resp.text().await.unwrap_or_default();
                tracing::error!(
                    status = %fallback_status,
                    body = %fallback_err,
                    "Claude API (haiku fallback streaming) returned error"
                );
                return Err(claude_http_error(
                    fallback_status,
                    fallback_err,
                    retry_after,
                ));
            }

            return Ok((fallback_resp, HAIKU_FALLBACK.to_string()));
        }

        if !status.is_success() {
            let retry_after = crate::error::parse_retry_after(&resp);
            let err_body = resp.text().await.unwrap_or_default();
            tracing::error!(
                status = %status,
                body = %err_body,
                "Claude streaming API returned error"
            );
            return Err(claude_http_error(status, err_body, retry_after));
        }

        Ok((resp, self.model.clone()))
    }

    /// Core streaming implementation. Both `chat_stream_dyn` (with default options)
    /// and `chat_stream_with_options_dyn` delegate here.
    fn stream_impl(
        &self,
        system_prompt: String,
        messages: Vec<ChatMessage>,
        request_timeout: Option<Duration>,
        options: crate::LlmRequestOptions,
    ) -> futures::stream::BoxStream<'_, LlmStreamEvent> {
        Box::pin(async_stream::stream! {
            let stream_result = self.send_streaming_request(&system_prompt, &messages, request_timeout, &options).await;
            let (response, actual_model) = match stream_result {
                Err(e) => {
                    yield LlmStreamEvent::Error {
                        message: e.to_string(),
                    };
                    return;
                }
                Ok(v) => v,
            };

            let mut byte_stream = response.bytes_stream();
            let mut buffer = Vec::<u8>::new();
            let mut accumulated = String::new();
            let mut input_tokens = 0u32;
            let mut output_tokens = 0u32;
            // Tracks the last output_tokens value at which a Usage event was emitted.
            // Used to implement the 50-token delta throttle: emit on first time or
            // when (current - last_emitted) >= 50.
            let mut last_emitted_output_tokens = 0u32;
            let mut done = false;

            // Per-index accumulator for tool_use blocks.
            // Key: block index (usize from content_block_start)
            // Value: (id, name, partial_json_string)
            let mut tool_accumulators: BTreeMap<usize, (String, String, String)> = BTreeMap::new();
            // Completed tool calls collected during this stream (carried into End).
            let mut stream_tool_calls: Vec<ToolCall> = Vec::new();

            // Per-index accumulator for thinking blocks.
            // Key: block index; Value: partial thinking text.
            let mut thinking_accum: BTreeMap<usize, String> = BTreeMap::new();
            // All completed thinking text concatenated (separator: newline).
            let mut accumulated_thinking = String::new();

            // Cache token counts from message_delta usage (Anthropic sends these
            // in the final message_delta before message_stop).
            let mut cache_creation_tokens: Option<u32> = None;
            let mut cache_read_tokens: Option<u32> = None;

            while !done {
                match byte_stream.next().await {
                    None => break,
                    Some(Err(e)) => {
                        let msg = if e.is_timeout() {
                            "timeout".to_string()
                        } else {
                            e.to_string()
                        };
                        yield LlmStreamEvent::Error {
                            message: LlmError::Network {
                                provider: "claude".into(),
                                msg,
                            }
                            .to_string(),
                        };
                        break;
                    }
                    Some(Ok(bytes)) => {
                        buffer.extend_from_slice(&bytes);
                        loop {
                            // Find \n\n separator between SSE events
                            let sep_pos = buffer.windows(2).position(|w| w == b"\n\n");
                            let Some(pos) = sep_pos else { break };

                            let event_bytes = buffer[..pos].to_vec();
                            buffer.drain(..pos + 2);

                            let event_str = match std::str::from_utf8(&event_bytes) {
                                Ok(s) => s.to_string(),
                                Err(_) => continue,
                            };

                            let (event_type, data) = parse_sse_event(&event_str);

                            match event_type.as_deref() {
                                Some("message_start") => {
                                    if let Some(ref v) = data {
                                        if let Some(n) = v
                                            .pointer("/message/usage/input_tokens")
                                            .and_then(|v| v.as_u64())
                                        {
                                            input_tokens = n as u32;
                                        }
                                    }
                                }
                                Some("content_block_start") => {
                                    // When a new tool_use block opens, capture its index,
                                    // id, and name so we can accumulate input_json_delta
                                    // chunks against it.
                                    // When a new thinking block opens, register the index.
                                    if let Some(ref v) = data {
                                        let block_type = v
                                            .pointer("/content_block/type")
                                            .and_then(|t| t.as_str())
                                            .unwrap_or("");
                                        if block_type == "tool_use" {
                                            if let Some(index) = v
                                                .get("index")
                                                .and_then(|i| i.as_u64())
                                            {
                                                let id = v
                                                    .pointer("/content_block/id")
                                                    .and_then(|s| s.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let name = v
                                                    .pointer("/content_block/name")
                                                    .and_then(|s| s.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                tool_accumulators.insert(
                                                    index as usize,
                                                    (id, name, String::new()),
                                                );
                                            }
                                        } else if block_type == "thinking" {
                                            if let Some(index) = v
                                                .get("index")
                                                .and_then(|i| i.as_u64())
                                            {
                                                thinking_accum.insert(index as usize, String::new());
                                            }
                                        }
                                    }
                                }
                                Some("content_block_delta") => {
                                    if let Some(ref v) = data {
                                        let delta_type = v
                                            .pointer("/delta/type")
                                            .and_then(|t| t.as_str())
                                            .unwrap_or("");
                                        if delta_type == "text_delta" {
                                            // Regular text chunk — yield Delta event.
                                            if let Some(text) = v
                                                .pointer("/delta/text")
                                                .and_then(|v| v.as_str())
                                            {
                                                accumulated.push_str(text);
                                                yield LlmStreamEvent::Delta {
                                                    text: text.to_string(),
                                                };
                                            }
                                        } else if delta_type == "input_json_delta" {
                                            // Partial JSON for a tool_use block —
                                            // append to the accumulator for this index.
                                            if let (Some(index), Some(partial)) = (
                                                v.get("index").and_then(|i| i.as_u64()),
                                                v.pointer("/delta/partial_json")
                                                    .and_then(|s| s.as_str()),
                                            ) {
                                                if let Some(acc) =
                                                    tool_accumulators.get_mut(&(index as usize))
                                                {
                                                    acc.2.push_str(partial);
                                                }
                                            }
                                        } else if delta_type == "thinking_delta" {
                                            // Partial thinking text — append to the
                                            // accumulator for this index.
                                            if let (Some(index), Some(chunk)) = (
                                                v.get("index").and_then(|i| i.as_u64()),
                                                v.pointer("/delta/thinking")
                                                    .and_then(|s| s.as_str()),
                                            ) {
                                                if let Some(acc) =
                                                    thinking_accum.get_mut(&(index as usize))
                                                {
                                                    acc.push_str(chunk);
                                                }
                                            }
                                        }
                                    }
                                }
                                Some("content_block_stop") => {
                                    // A content block completed.
                                    // - tool_use: parse accumulated JSON and emit ToolCall.
                                    // - thinking: emit Thinking event with full text.
                                    if let Some(ref v) = data {
                                        if let Some(index) = v
                                            .get("index")
                                            .and_then(|i| i.as_u64())
                                        {
                                            let idx = index as usize;
                                            if let Some((id, name, json_str)) =
                                                tool_accumulators.remove(&idx)
                                            {
                                                let arguments: serde_json::Value =
                                                    serde_json::from_str(&json_str)
                                                        .unwrap_or(serde_json::Value::Null);
                                                let call = ToolCall {
                                                    id,
                                                    name,
                                                    arguments,
                                                };
                                                stream_tool_calls.push(call.clone());
                                                yield LlmStreamEvent::ToolCall { call };
                                            } else if let Some(thinking_text) =
                                                thinking_accum.remove(&idx)
                                            {
                                                if !accumulated_thinking.is_empty() {
                                                    accumulated_thinking.push('\n');
                                                }
                                                accumulated_thinking.push_str(&thinking_text);
                                                yield LlmStreamEvent::Thinking {
                                                    text: thinking_text,
                                                };
                                            }
                                        }
                                    }
                                }
                                Some("message_delta") => {
                                    if let Some(ref v) = data {
                                        if let Some(n) = v
                                            .pointer("/usage/output_tokens")
                                            .and_then(|v| v.as_u64())
                                        {
                                            output_tokens = n as u32;

                                            // Progressive Usage emission with 50-token delta throttle.
                                            // Emit when the cumulative delta since the last emission
                                            // reaches 50 tokens. This means small deltas (< 50 from
                                            // baseline 0) are skipped until the threshold is crossed.
                                            let delta = output_tokens
                                                .saturating_sub(last_emitted_output_tokens);
                                            if delta >= 50 {
                                                let estimated_cost_usd =
                                                    ClaudeProvider::estimate_cost(
                                                        &actual_model,
                                                        input_tokens,
                                                        output_tokens,
                                                    );
                                                yield LlmStreamEvent::Usage {
                                                    input_tokens: Some(input_tokens),
                                                    output_tokens,
                                                    estimated_cost_usd,
                                                };
                                                last_emitted_output_tokens = output_tokens;
                                            }
                                        }
                                        // Cache token counts sent in message_delta usage
                                        if let Some(n) = v
                                            .pointer("/usage/cache_creation_input_tokens")
                                            .and_then(|v| v.as_u64())
                                        {
                                            cache_creation_tokens = Some(n as u32);
                                        }
                                        if let Some(n) = v
                                            .pointer("/usage/cache_read_input_tokens")
                                            .and_then(|v| v.as_u64())
                                        {
                                            cache_read_tokens = Some(n as u32);
                                        }
                                    }
                                }
                                Some("message_stop") => {
                                    let cost = ClaudeProvider::estimate_cost(
                                        &actual_model,
                                        input_tokens,
                                        output_tokens,
                                    );
                                    let thinking_text_final = if accumulated_thinking.is_empty() {
                                        None
                                    } else {
                                        Some(accumulated_thinking.clone())
                                    };
                                    yield LlmStreamEvent::End {
                                        total: LlmResponse {
                                            rendered_text: accumulated.clone(),
                                            model: actual_model.clone(),
                                            estimated_cost_usd: cost,
                                            input_tokens: Some(input_tokens),
                                            output_tokens: Some(output_tokens),
                                            tool_calls: stream_tool_calls.clone(),
                                            thinking_text: thinking_text_final,
                                            cache_creation_input_tokens: cache_creation_tokens,
                                            cache_read_input_tokens: cache_read_tokens,
                                            ..Default::default()
                                        },
                                    };
                                    done = true;
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        })
    }
}

/// Parses one SSE event block (text between `\n\n` separators).
/// Returns (event_type, data_json).
fn parse_sse_event(event_str: &str) -> (Option<String>, Option<Value>) {
    let mut event_type = None;
    let mut data_str = None;

    for line in event_str.lines() {
        if let Some(rest) = line.strip_prefix("event: ") {
            event_type = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("data: ") {
            data_str = Some(rest.trim().to_string());
        }
    }

    let data = data_str.and_then(|s| serde_json::from_str::<Value>(&s).ok());
    (event_type, data)
}

/// Map an HTTP status + body to the correct `LlmError` variant for Claude.
/// `retry_after` must be pre-computed via `parse_retry_after(&resp)` before the body is consumed.
fn claude_http_error(
    status: reqwest::StatusCode,
    err_body: String,
    retry_after: Option<u32>,
) -> LlmError {
    match status.as_u16() {
        401 | 403 => LlmError::Auth {
            provider: "claude".into(),
        },
        429 => LlmError::RateLimit {
            provider: "claude".into(),
            retry_after_secs: retry_after,
        },
        s => LlmError::Upstream {
            provider: "claude".into(),
            status: s,
            body: err_body,
        },
    }
}

impl LlmProviderDyn for ClaudeProvider {
    fn render_dyn(
        &self,
        system_prompt: &str,
        user_input: &str,
        _result_json: &Value,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + '_>,
    > {
        let messages = vec![ChatMessage::user(user_input)];
        let system = system_prompt.to_string();
        Box::pin(async move { self.chat_impl(&system, &messages).await })
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

    fn chat_stream_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        _result_json: &'a Value,
    ) -> futures::stream::BoxStream<'a, LlmStreamEvent> {
        self.stream_impl(
            system_prompt.to_string(),
            messages.to_vec(),
            None,
            Default::default(),
        )
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
        _result_json: &'a Value,
    ) -> futures::stream::BoxStream<'a, LlmStreamEvent> {
        self.stream_impl(
            system_prompt.to_string(),
            messages.to_vec(),
            options.timeout,
            options.clone(),
        )
    }
}

impl ClaudeProvider {
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
        let claude_messages = Self::build_messages(messages);

        let mut body = ClaudeRequest {
            model: self.model.clone(),
            max_tokens: options.max_tokens.unwrap_or(MAX_TOKENS),
            system: vec![SystemBlock {
                block_type: "text".into(),
                text: system_prompt.to_string(),
                cache_control: None,
            }],
            messages: claude_messages,
            stream: None,
            temperature: options.temperature,
            top_p: options.top_p,
            stop_sequences: options.stop_sequences.clone(),
            tools: Self::build_tools(options),
            tool_choice: Self::build_tool_choice(options),
            thinking: options
                .thinking_budget_tokens
                .map(|n| ClaudeThinkingConfig {
                    kind: "enabled".into(),
                    budget_tokens: n,
                }),
        };

        Self::apply_cache_breakpoints(&mut body, &options.cache_breakpoints);

        // OAuth token (sk-ant-oat-*) → Bearer auth, API key → x-api-key header
        let mut req = self
            .client
            .post(&self.base_url)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json");

        if self.api_key.starts_with("sk-ant-oat") {
            req = req
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("anthropic-beta", "claude-code-20250219,oauth-2025-04-20")
                .header("user-agent", "claude-cli/2.1.77")
                .header("x-app", "cli");
        } else {
            req = req.header("x-api-key", &self.api_key);
        }

        if options.thinking_budget_tokens.is_some() {
            req = req.header("anthropic-beta", "interleaved-thinking-2025-05-14");
        }

        let body_json = serde_json::to_string(&body).unwrap_or_default();
        tracing::debug!(request_body = %body_json, "Claude API request");

        let req = req.json(&body);
        let req = if let Some(d) = request_timeout {
            req.timeout(d)
        } else {
            req
        };

        let resp = req.send().await.map_err(|e| {
            if e.is_timeout() {
                LlmError::Network {
                    provider: "claude".into(),
                    msg: "timeout".into(),
                }
            } else {
                LlmError::Network {
                    provider: "claude".into(),
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
                body = %err_body,
                model = %self.model,
                is_oat = self.api_key.starts_with("sk-ant-oat"),
                "Claude API returned error"
            );

            // OAT token + 400: retry with haiku fallback (OAT may not support all models)
            if status.as_u16() == 400 && self.api_key.starts_with("sk-ant-oat") {
                const HAIKU_FALLBACK: &str = "claude-haiku-4-5";
                tracing::warn!(
                    original_model = %self.model,
                    fallback_model = HAIKU_FALLBACK,
                    "OAT 400 error, retrying with Haiku fallback"
                );
                let fallback_messages = Self::build_messages(messages);
                let mut fallback_body = ClaudeRequest {
                    model: HAIKU_FALLBACK.to_string(),
                    max_tokens: options.max_tokens.unwrap_or(MAX_TOKENS),
                    system: vec![SystemBlock {
                        block_type: "text".into(),
                        text: system_prompt.to_string(),
                        cache_control: None,
                    }],
                    messages: fallback_messages,
                    stream: None,
                    temperature: options.temperature,
                    top_p: options.top_p,
                    stop_sequences: options.stop_sequences.clone(),
                    tools: Self::build_tools(options),
                    tool_choice: Self::build_tool_choice(options),
                    thinking: options
                        .thinking_budget_tokens
                        .map(|n| ClaudeThinkingConfig {
                            kind: "enabled".into(),
                            budget_tokens: n,
                        }),
                };
                Self::apply_cache_breakpoints(&mut fallback_body, &options.cache_breakpoints);
                let mut fallback_req = self
                    .client
                    .post(&self.base_url)
                    .header("anthropic-version", ANTHROPIC_VERSION)
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {}", self.api_key))
                    .header("anthropic-beta", "claude-code-20250219,oauth-2025-04-20")
                    .header("user-agent", "claude-cli/2.1.77")
                    .header("x-app", "cli");
                if options.thinking_budget_tokens.is_some() {
                    fallback_req =
                        fallback_req.header("anthropic-beta", "interleaved-thinking-2025-05-14");
                }
                let mut fallback_req = fallback_req.json(&fallback_body);
                if let Some(d) = request_timeout {
                    fallback_req = fallback_req.timeout(d);
                }
                let fallback_resp = fallback_req.send().await.map_err(|e| {
                    if e.is_timeout() {
                        LlmError::Network {
                            provider: "claude".into(),
                            msg: "timeout".into(),
                        }
                    } else {
                        LlmError::Network {
                            provider: "claude".into(),
                            msg: format!("haiku fallback request failed: {e}"),
                        }
                    }
                })?;

                let fallback_status = fallback_resp.status();
                if !fallback_status.is_success() {
                    let fallback_retry_after = crate::error::parse_retry_after(&fallback_resp);
                    let fallback_err = fallback_resp.text().await.unwrap_or_default();
                    tracing::error!(
                        status = %fallback_status,
                        body = %fallback_err,
                        "Claude API (haiku fallback) returned error"
                    );
                    return Err(claude_http_error(
                        fallback_status,
                        format!(
                            "primary HTTP {status}; haiku fallback also failed: {fallback_err}"
                        ),
                        fallback_retry_after,
                    ));
                }

                let fallback_text = fallback_resp.text().await.map_err(|e| LlmError::Network {
                    provider: "claude".into(),
                    msg: format!("failed to read fallback response body: {e}"),
                })?;
                let fallback_parsed: ClaudeResponse = serde_json::from_str(&fallback_text)
                    .map_err(|e| LlmError::Parse {
                        provider: "claude".into(),
                        reason: e.to_string(),
                    })?;
                let fb_tool_calls = Self::parse_tool_calls(&fallback_parsed.content);
                let mut fb_text = String::new();
                let mut fb_thinking_acc = String::new();
                for block in &fallback_parsed.content {
                    match block.block_type.as_str() {
                        "text" => fb_text.push_str(&block.text),
                        "thinking" => {
                            if !fb_thinking_acc.is_empty() {
                                fb_thinking_acc.push('\n');
                            }
                            fb_thinking_acc.push_str(&block.thinking);
                        }
                        _ => {}
                    }
                }
                let cost = Self::estimate_cost(
                    &fallback_parsed.model,
                    fallback_parsed.usage.input_tokens,
                    fallback_parsed.usage.output_tokens,
                );
                return Ok(LlmResponse {
                    rendered_text: fb_text,
                    model: fallback_parsed.model,
                    estimated_cost_usd: cost,
                    input_tokens: Some(fallback_parsed.usage.input_tokens),
                    output_tokens: Some(fallback_parsed.usage.output_tokens),
                    tool_calls: fb_tool_calls,
                    thinking_text: if fb_thinking_acc.is_empty() {
                        None
                    } else {
                        Some(fb_thinking_acc)
                    },
                    cache_creation_input_tokens: fallback_parsed.usage.cache_creation_input_tokens,
                    cache_read_input_tokens: fallback_parsed.usage.cache_read_input_tokens,
                    ..Default::default()
                });
            }

            return Err(claude_http_error(status, err_body, retry_after));
        }

        let body_text = resp.text().await.map_err(|e| LlmError::Network {
            provider: "claude".into(),
            msg: format!("failed to read response body: {e}"),
        })?;

        let claude_resp: ClaudeResponse =
            serde_json::from_str(&body_text).map_err(|e| LlmError::Parse {
                provider: "claude".into(),
                reason: e.to_string(),
            })?;

        if claude_resp.stop_reason == "max_tokens" {
            tracing::warn!("Claude response truncated at max_tokens");
        }

        let tool_calls = Self::parse_tool_calls(&claude_resp.content);

        let mut text = String::new();
        let mut thinking_acc = String::new();
        for block in &claude_resp.content {
            match block.block_type.as_str() {
                "text" => text.push_str(&block.text),
                "thinking" => {
                    if !thinking_acc.is_empty() {
                        thinking_acc.push('\n');
                    }
                    thinking_acc.push_str(&block.thinking);
                }
                _ => {}
            }
        }

        let cost = Self::estimate_cost(
            &claude_resp.model,
            claude_resp.usage.input_tokens,
            claude_resp.usage.output_tokens,
        );

        Ok(LlmResponse {
            rendered_text: text,
            model: claude_resp.model,
            estimated_cost_usd: cost,
            input_tokens: Some(claude_resp.usage.input_tokens),
            output_tokens: Some(claude_resp.usage.output_tokens),
            tool_calls,
            thinking_text: if thinking_acc.is_empty() {
                None
            } else {
                Some(thinking_acc)
            },
            cache_creation_input_tokens: claude_resp.usage.cache_creation_input_tokens,
            cache_read_input_tokens: claude_resp.usage.cache_read_input_tokens,
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
            provider: "claude".into(),
            reason: "API key is empty".into(),
        });
    }
    Ok(Box::new(ClaudeProvider::new(
        api_key.to_string(),
        model.to_string(),
    )))
}

/// List available Claude models via the Anthropic API.
///
/// Calls `GET https://api.anthropic.com/v1/models` with `x-api-key` header.
pub async fn list_models(api_key: &str) -> Result<Vec<crate::ListModel>, LlmError> {
    if api_key.is_empty() {
        return Err(LlmError::MissingConfig {
            provider: "claude".into(),
            reason: "API key is empty".into(),
        });
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| LlmError::Other {
            provider: "claude".into(),
            message: format!("failed to build HTTP client: {e}"),
        })?;

    let resp = client
        .get("https://api.anthropic.com/v1/models")
        .header("x-api-key", api_key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("content-type", "application/json")
        .send()
        .await
        .map_err(|e| LlmError::Network {
            provider: "claude".into(),
            msg: e.to_string(),
        })?;

    let status = resp.status();
    if !status.is_success() {
        let retry_after = crate::error::parse_retry_after(&resp);
        let err_body = resp.text().await.unwrap_or_default();
        return Err(match status.as_u16() {
            401 | 403 => LlmError::Auth {
                provider: "claude".into(),
            },
            429 => LlmError::RateLimit {
                provider: "claude".into(),
                retry_after_secs: retry_after,
            },
            s => LlmError::Upstream {
                provider: "claude".into(),
                status: s,
                body: err_body,
            },
        });
    }

    let body: Value = resp.json().await.map_err(|e| LlmError::Parse {
        provider: "claude".into(),
        reason: e.to_string(),
    })?;

    let models = body
        .get("data")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let name = m.get("id").and_then(|v| v.as_str())?.to_string();
                    Some(crate::ListModel {
                        name,
                        display_name: m
                            .get("display_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        description: m
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        input_token_limit: m
                            .get("input_tokens")
                            .and_then(|v| v.as_i64())
                            .map(|v| v as i32),
                        output_token_limit: m
                            .get("output_tokens")
                            .and_then(|v| v.as_i64())
                            .map(|v| v as i32),
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

    #[test]
    fn test_stream_parsing_content_block_delta() {
        let event_str = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}";
        let (event_type, data) = parse_sse_event(event_str);
        assert_eq!(event_type.as_deref(), Some("content_block_delta"));
        let text = data.and_then(|v| {
            v.pointer("/delta/text")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });
        assert_eq!(text.as_deref(), Some("Hello"));
    }

    #[test]
    fn test_stream_parsing_message_stop() {
        let event_str = "event: message_stop\ndata: {\"type\":\"message_stop\"}";
        let (event_type, _data) = parse_sse_event(event_str);
        assert_eq!(event_type.as_deref(), Some("message_stop"));
    }

    #[test]
    fn test_stream_parsing_message_start_tokens() {
        let event_str = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":42,\"output_tokens\":0}}}";
        let (event_type, data) = parse_sse_event(event_str);
        assert_eq!(event_type.as_deref(), Some("message_start"));
        let input_tokens = data.and_then(|v| {
            v.pointer("/message/usage/input_tokens")
                .and_then(|v| v.as_u64())
        });
        assert_eq!(input_tokens, Some(42));
    }

    #[test]
    fn test_stream_parsing_message_delta_usage() {
        let event_str = "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":17}}";
        let (event_type, data) = parse_sse_event(event_str);
        assert_eq!(event_type.as_deref(), Some("message_delta"));
        let output_tokens =
            data.and_then(|v| v.pointer("/usage/output_tokens").and_then(|v| v.as_u64()));
        assert_eq!(output_tokens, Some(17));
    }

    #[test]
    fn test_stream_parsing_unknown_event() {
        let event_str = "event: ping\ndata: {\"type\":\"ping\"}";
        let (event_type, _) = parse_sse_event(event_str);
        assert_eq!(event_type.as_deref(), Some("ping"));
    }

    #[tokio::test]
    async fn timeout_option_aborts_blocking_call() {
        use crate::{ChatMessage, LlmError, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use std::time::Duration;
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_secs(2))
                    .set_body_json(serde_json::json!({
                        "id": "x",
                        "content": [{"type": "text", "text": "late"}],
                        "model": "claude-test",
                        "usage": {"input_tokens": 1, "output_tokens": 1},
                        "stop_reason": "end_turn"
                    })),
            )
            .mount(&server)
            .await;

        let provider = ClaudeProvider::new_with_base_url("k".into(), "test".into(), server.uri());
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
                    if provider == "claude" && msg.to_lowercase().contains("timeout")
            ),
            "expected claude Network/timeout, got {res:?}"
        );
        assert!(
            elapsed < Duration::from_millis(1500),
            "should abort < 1.5s, took {elapsed:?}"
        );
    }

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
                "stop_sequences": ["END"]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "x",
                "content": [{"type": "text", "text": "ok"}],
                "model": "claude-test",
                "usage": {"input_tokens": 1, "output_tokens": 1},
                "stop_reason": "end_turn"
            })))
            .mount(&server)
            .await;

        let provider = ClaudeProvider::new_with_base_url("k".into(), "test".into(), server.uri());
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

    #[tokio::test]
    async fn timeout_option_aborts_streaming_call() {
        use crate::{ChatMessage, LlmProviderDyn, LlmRequestOptions, LlmStreamEvent};
        use futures::StreamExt;
        use serde_json::Value;
        use std::time::Duration;
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_secs(2))
                    .set_body_string(""),
            )
            .mount(&server)
            .await;

        let provider = ClaudeProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let messages = vec![ChatMessage::user("x")];
        let opts = LlmRequestOptions {
            timeout: Some(Duration::from_millis(500)),
            ..Default::default()
        };

        let started = std::time::Instant::now();
        let mut stream = provider.chat_stream_with_options_dyn("", &messages, &opts, &Value::Null);
        let first_event = stream.next().await;
        let elapsed = started.elapsed();

        assert!(
            matches!(
                first_event,
                Some(LlmStreamEvent::Error { ref message })
                    if message.to_lowercase().contains("timeout")
            ),
            "expected Error event with timeout message, got {first_event:?}",
            first_event = first_event.map(|e| match e {
                LlmStreamEvent::Error { message } => format!("Error({message})"),
                LlmStreamEvent::Delta { text } => format!("Delta({text})"),
                LlmStreamEvent::End { .. } => "End".to_string(),
                _ => "Unknown".to_string(),
            })
        );
        assert!(
            elapsed < Duration::from_millis(1500),
            "should abort < 1.5s, took {elapsed:?}"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Task 5: Tool calling tests
    // ──────────────────────────────────────────────────────────────────────────

    /// Test A: blocking response with a `tool_use` block is parsed into
    /// `LlmResponse.tool_calls`.
    #[tokio::test(flavor = "multi_thread")]
    async fn tool_use_in_blocking_response_returns_tool_calls() {
        use crate::{ChatMessage, LlmProviderDyn, LlmRequestOptions, ToolDef};
        use serde_json::Value;
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

        let fixture = include_str!("../tests/fixtures/claude_tool_use_response.json");

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(fixture)
                    .insert_header("content-type", "application/json"),
            )
            .mount(&server)
            .await;

        let provider =
            ClaudeProvider::new_with_base_url("k".into(), "claude-sonnet-4-5".into(), server.uri());
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
        assert_eq!(call.id, "toolu_01");
        assert_eq!(call.arguments["city"], "seoul");
        // Text block is also captured
        assert_eq!(resp.rendered_text, "Checking the weather.");
    }

    /// Test B: `tools` array appears in the serialised request body.
    #[tokio::test(flavor = "multi_thread")]
    async fn tools_appear_in_request_body() {
        use crate::{ChatMessage, LlmProviderDyn, LlmRequestOptions, ToolDef};
        use serde_json::Value;
        use wiremock::matchers::{self, body_partial_json};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(serde_json::json!({
                "tools": [{"name": "get_weather", "description": "Get weather for a city"}]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "x",
                "content": [{"type": "text", "text": "ok"}],
                "model": "claude-test",
                "usage": {"input_tokens": 1, "output_tokens": 1},
                "stop_reason": "end_turn"
            })))
            .mount(&server)
            .await;

        let provider = ClaudeProvider::new_with_base_url("k".into(), "test".into(), server.uri());
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

    /// Test C: `tool_choice: Required` serialises as `{"type": "any"}` in the request body.
    #[tokio::test(flavor = "multi_thread")]
    async fn tool_choice_required_appears_in_request_body() {
        use crate::{ChatMessage, LlmProviderDyn, LlmRequestOptions, ToolChoice, ToolDef};
        use serde_json::Value;
        use wiremock::matchers::{self, body_partial_json};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(serde_json::json!({
                "tool_choice": {"type": "any"}
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "x",
                "content": [{"type": "text", "text": "ok"}],
                "model": "claude-test",
                "usage": {"input_tokens": 1, "output_tokens": 1},
                "stop_reason": "end_turn"
            })))
            .mount(&server)
            .await;

        let provider = ClaudeProvider::new_with_base_url("k".into(), "test".into(), server.uri());
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

    /// Test D: `ChatMessage::tool_result` encodes as a `tool_result` content block.
    #[tokio::test(flavor = "multi_thread")]
    async fn tool_result_block_serializes_correctly() {
        use crate::{ChatMessage, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use wiremock::matchers::{self, body_partial_json};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(serde_json::json!({
                "messages": [
                    {
                        "role": "user",
                        "content": [
                            {
                                "type": "tool_result",
                                "tool_use_id": "toolu_01",
                                "content": "25C",
                                "is_error": false
                            }
                        ]
                    }
                ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "x",
                "content": [{"type": "text", "text": "ok"}],
                "model": "claude-test",
                "usage": {"input_tokens": 1, "output_tokens": 1},
                "stop_reason": "end_turn"
            })))
            .mount(&server)
            .await;

        let provider = ClaudeProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let messages = vec![ChatMessage::tool_result("toolu_01", "25C", false)];
        let result = provider
            .chat_with_options_dyn("", &messages, &LlmRequestOptions::default(), &Value::Null)
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    /// Test E: streaming SSE with a `tool_use` block emits `LlmStreamEvent::ToolCall`
    /// and the terminal `End` event carries the accumulated tool call.
    #[tokio::test(flavor = "multi_thread")]
    async fn tool_use_in_streaming_emits_toolcall_event() {
        use crate::{
            ChatMessage, LlmProviderDyn, LlmRequestOptions, LlmStreamEvent, ToolCall as TC, ToolDef,
        };
        use futures::StreamExt;
        use serde_json::Value;
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

        // Build a minimal but complete SSE stream: one text block + one tool_use block.
        let sse_body = concat!(
            "event: message_start\n",
            "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":50,\"output_tokens\":0}}}\n\n",
            // Text block (index 0)
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Checking the weather.\"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            // Tool use block (index 1)
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_01\",\"name\":\"get_weather\",\"input\":{}}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"city\\\":\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"seoul\\\"\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"}\"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":20}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n",
        );

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(sse_body)
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;

        let provider = ClaudeProvider::new_with_base_url("k".into(), "test".into(), server.uri());
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
        assert_eq!(call.id, "toolu_01");
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
    async fn parallel_tool_calls_all_emitted_with_correct_ids() {
        // Three parallel tool_use blocks at indices 0, 1, 2 named "a", "b", "c".
        // SSE content_block_stop events arrive out of index order: 2, 0, 1.
        // Claude emits one ToolCall per content_block_stop, so ToolCall events follow
        // SSE arrival order.  The important regression check is that all three calls
        // are emitted and that no accumulator entry is silently lost or cross-wired
        // (i.e. id matches name for every call).
        use crate::{ChatMessage, LlmProviderDyn, LlmRequestOptions, LlmStreamEvent, ToolDef};
        use futures::StreamExt;
        use serde_json::Value;
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

        let sse_body = concat!(
            "event: message_start\n",
            "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n\n",
            // index 2 starts and stops first
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":2,\"content_block\":{\"type\":\"tool_use\",\"id\":\"id_c\",\"name\":\"c\",\"input\":{}}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":2,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{}\"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":2}\n\n",
            // index 0 starts and stops second
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"id_a\",\"name\":\"a\",\"input\":{}}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{}\"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            // index 1 starts and stops last
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"id_b\",\"name\":\"b\",\"input\":{}}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{}\"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":5}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n",
        );

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(sse_body)
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;

        let provider = ClaudeProvider::new_with_base_url("k".into(), "test".into(), server.uri());
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
        let mut stream = provider.chat_stream_with_options_dyn("", &msgs, &opts, &Value::Null);

        let mut tool_call_events: Vec<(String, String)> = Vec::new(); // (name, id)
        let mut end_names: Vec<String> = Vec::new();

        while let Some(event) = stream.next().await {
            match event {
                LlmStreamEvent::ToolCall { call } => {
                    tool_call_events.push((call.name.clone(), call.id.clone()));
                }
                LlmStreamEvent::End { total } => {
                    end_names = total.tool_calls.iter().map(|c| c.name.clone()).collect();
                    break;
                }
                LlmStreamEvent::Error { message } => panic!("unexpected error: {message}"),
                _ => {}
            }
        }

        assert_eq!(
            tool_call_events.len(),
            3,
            "expected 3 ToolCall events for 3 parallel tool calls"
        );

        // Each call must have matching id and name (no cross-wiring between accumulator slots).
        let names_and_ids: Vec<(&str, &str)> = tool_call_events
            .iter()
            .map(|(n, id)| (n.as_str(), id.as_str()))
            .collect();
        assert!(
            names_and_ids.contains(&("a", "id_a")),
            "tool 'a' must have id 'id_a', got: {names_and_ids:?}"
        );
        assert!(
            names_and_ids.contains(&("b", "id_b")),
            "tool 'b' must have id 'id_b', got: {names_and_ids:?}"
        );
        assert!(
            names_and_ids.contains(&("c", "id_c")),
            "tool 'c' must have id 'id_c', got: {names_and_ids:?}"
        );

        // End.total must carry all three calls.
        let mut end_sorted = end_names.clone();
        end_sorted.sort();
        assert_eq!(
            end_sorted,
            vec!["a", "b", "c"],
            "End.total.tool_calls must include all three parallel calls"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Task 10: Image content block tests
    // ──────────────────────────────────────────────────────────────────────────

    /// Test A: base64 image block serializes with type, media_type, and data fields.
    #[tokio::test(flavor = "multi_thread")]
    async fn image_base64_block_serializes_correctly() {
        use crate::{ChatContent, ChatMessage, ImageSource, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use wiremock::matchers::{self, body_partial_json};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(serde_json::json!({
                "messages": [
                    {
                        "role": "user",
                        "content": [
                            {"type": "text", "text": "describe this"},
                            {
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": "image/png",
                                    "data": "AAAA"
                                }
                            }
                        ]
                    }
                ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "x",
                "content": [{"type": "text", "text": "a red circle"}],
                "model": "claude-test",
                "usage": {"input_tokens": 10, "output_tokens": 5},
                "stop_reason": "end_turn"
            })))
            .mount(&server)
            .await;

        let provider = ClaudeProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let image = ChatContent::Image {
            source: ImageSource::Base64("AAAA".into()),
            mime_type: "image/png".into(),
        };
        let messages = vec![ChatMessage::user_with_image("describe this", image)];
        let result = provider
            .chat_with_options_dyn("", &messages, &LlmRequestOptions::default(), &Value::Null)
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    /// Test B: URL image block serializes with type and url only — no media_type field.
    #[tokio::test(flavor = "multi_thread")]
    async fn image_url_block_serializes_correctly() {
        use crate::{ChatContent, ChatMessage, ImageSource, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use wiremock::matchers::{self, body_partial_json};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(serde_json::json!({
                "messages": [
                    {
                        "role": "user",
                        "content": [
                            {"type": "text", "text": "describe"},
                            {
                                "type": "image",
                                "source": {
                                    "type": "url",
                                    "url": "https://example.com/x.png"
                                }
                            }
                        ]
                    }
                ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "x",
                "content": [{"type": "text", "text": "a blue square"}],
                "model": "claude-test",
                "usage": {"input_tokens": 10, "output_tokens": 5},
                "stop_reason": "end_turn"
            })))
            .mount(&server)
            .await;

        let provider = ClaudeProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let image = ChatContent::Image {
            source: ImageSource::Url("https://example.com/x.png".into()),
            mime_type: "image/png".into(),
        };
        let messages = vec![ChatMessage::user_with_image("describe", image)];
        let result = provider
            .chat_with_options_dyn("", &messages, &LlmRequestOptions::default(), &Value::Null)
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Task 4: Extended thinking tests
    // ──────────────────────────────────────────────────────────────────────────

    /// Test A: blocking response with a `thinking` block populates `thinking_text`.
    #[tokio::test(flavor = "multi_thread")]
    async fn thinking_block_in_blocking_response_populates_thinking_text() {
        use crate::{ChatMessage, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

        let fixture = include_str!("../tests/fixtures/claude_thinking_response.json");

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(fixture)
                    .insert_header("content-type", "application/json"),
            )
            .mount(&server)
            .await;

        let provider =
            ClaudeProvider::new_with_base_url("k".into(), "claude-sonnet-4-5".into(), server.uri());
        let opts = LlmRequestOptions {
            thinking_budget_tokens: Some(8192),
            ..Default::default()
        };
        let resp = provider
            .chat_with_options_dyn("", &[ChatMessage::user("think")], &opts, &Value::Null)
            .await
            .expect("expected Ok response");

        assert_eq!(
            resp.thinking_text,
            Some("Let me reason about this...".to_string()),
            "thinking_text should capture the thinking block"
        );
        assert_eq!(
            resp.rendered_text, "Final answer.",
            "rendered_text should capture the text block"
        );
    }

    /// Test B: `thinking_budget_tokens` appears in the serialized request body.
    #[tokio::test(flavor = "multi_thread")]
    async fn thinking_budget_appears_in_request_body() {
        use crate::{ChatMessage, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use wiremock::matchers::{self, body_partial_json};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(serde_json::json!({
                "thinking": {"type": "enabled", "budget_tokens": 8192}
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "x",
                "content": [{"type": "text", "text": "ok"}],
                "model": "claude-test",
                "usage": {"input_tokens": 1, "output_tokens": 1},
                "stop_reason": "end_turn"
            })))
            .mount(&server)
            .await;

        let provider = ClaudeProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let opts = LlmRequestOptions {
            thinking_budget_tokens: Some(8192),
            ..Default::default()
        };
        let result = provider
            .chat_with_options_dyn("", &[ChatMessage::user("hi")], &opts, &Value::Null)
            .await;
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    /// Test C: `thinking_budget_tokens` causes the beta header to be added.
    #[tokio::test(flavor = "multi_thread")]
    async fn thinking_budget_adds_beta_header() {
        use crate::{ChatMessage, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use wiremock::matchers::{self, header};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .and(header("anthropic-beta", "interleaved-thinking-2025-05-14"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "x",
                "content": [{"type": "text", "text": "ok"}],
                "model": "claude-test",
                "usage": {"input_tokens": 1, "output_tokens": 1},
                "stop_reason": "end_turn"
            })))
            .mount(&server)
            .await;

        let provider = ClaudeProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let opts = LlmRequestOptions {
            thinking_budget_tokens: Some(1024),
            ..Default::default()
        };
        let result = provider
            .chat_with_options_dyn("", &[ChatMessage::user("hi")], &opts, &Value::Null)
            .await;
        assert!(
            result.is_ok(),
            "expected ok (mock matched beta header), got {result:?}"
        );
    }

    /// Test D: streaming SSE with thinking blocks emits `LlmStreamEvent::Thinking` events
    /// and the terminal `End` event carries `thinking_text`.
    #[tokio::test(flavor = "multi_thread")]
    async fn thinking_streaming_emits_thinking_event() {
        use crate::{ChatMessage, LlmProviderDyn, LlmRequestOptions, LlmStreamEvent};
        use futures::StreamExt;
        use serde_json::Value;
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

        let sse_body = concat!(
            "event: message_start\n",
            "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":50,\"output_tokens\":0}}}\n\n",
            // Thinking block (index 0)
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"Let me \"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"reason...\"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            // Text block (index 1)
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"Final answer.\"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":30}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n",
        );

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(sse_body)
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;

        let provider = ClaudeProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let opts = LlmRequestOptions {
            thinking_budget_tokens: Some(8192),
            ..Default::default()
        };

        let msgs = vec![ChatMessage::user("think")];
        let mut stream = provider.chat_stream_with_options_dyn("", &msgs, &opts, &Value::Null);

        let mut thinking_events: Vec<String> = Vec::new();
        let mut got_delta = false;
        let mut end_total: Option<LlmResponse> = None;

        while let Some(event) = stream.next().await {
            match event {
                LlmStreamEvent::Thinking { text } => thinking_events.push(text),
                LlmStreamEvent::Delta { .. } => got_delta = true,
                LlmStreamEvent::End { total } => {
                    end_total = Some(total);
                    break;
                }
                LlmStreamEvent::Error { message } => panic!("unexpected error: {message}"),
                _ => {}
            }
        }

        assert!(
            !thinking_events.is_empty(),
            "expected at least one Thinking event before Delta"
        );
        // Thinking events should arrive before text delta
        assert!(
            got_delta,
            "expected at least one Delta event after thinking"
        );

        // The thinking content should be the full accumulated text from the two deltas
        assert_eq!(
            thinking_events[0], "Let me reason...",
            "thinking event text should be full accumulated thinking from both deltas"
        );

        let total = end_total.expect("expected End event");
        assert!(
            total.thinking_text.is_some(),
            "End.total.thinking_text should be Some when thinking blocks were present"
        );
        assert_eq!(
            total.thinking_text.as_deref(),
            Some("Let me reason..."),
            "End.total.thinking_text should contain the full accumulated thinking"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Task 9: Anthropic Prompt Caching — cache_breakpoints + cache token parsing
    // ──────────────────────────────────────────────────────────────────────────

    /// Test A: CacheBreakpoint::System places cache_control on the last system block.
    #[tokio::test(flavor = "multi_thread")]
    async fn cache_breakpoint_system_appears_on_system_block() {
        use crate::{CacheBreakpoint, ChatMessage, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use wiremock::matchers::{self, body_partial_json};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(serde_json::json!({
                "system": [
                    {
                        "type": "text",
                        "text": "You are helpful.",
                        "cache_control": {"type": "ephemeral"}
                    }
                ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "x",
                "content": [{"type": "text", "text": "ok"}],
                "model": "claude-test",
                "usage": {"input_tokens": 1, "output_tokens": 1},
                "stop_reason": "end_turn"
            })))
            .mount(&server)
            .await;

        let provider = ClaudeProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let opts = LlmRequestOptions {
            cache_breakpoints: vec![CacheBreakpoint::System],
            ..Default::default()
        };
        let result = provider
            .chat_with_options_dyn(
                "You are helpful.",
                &[ChatMessage::user("hi")],
                &opts,
                &Value::Null,
            )
            .await;
        assert!(result.is_ok(), "expected Ok, got {result:?}");
    }

    /// Test B: CacheBreakpoint::MessageIndex(0) places cache_control on the last content
    /// block of messages[0].
    #[tokio::test(flavor = "multi_thread")]
    async fn cache_breakpoint_message_index_appears_on_message() {
        use crate::{CacheBreakpoint, ChatMessage, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use wiremock::matchers::{self, body_partial_json};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(serde_json::json!({
                "messages": [
                    {
                        "role": "user",
                        "content": [
                            {
                                "type": "text",
                                "text": "hi",
                                "cache_control": {"type": "ephemeral"}
                            }
                        ]
                    }
                ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "x",
                "content": [{"type": "text", "text": "ok"}],
                "model": "claude-test",
                "usage": {"input_tokens": 1, "output_tokens": 1},
                "stop_reason": "end_turn"
            })))
            .mount(&server)
            .await;

        let provider = ClaudeProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let opts = LlmRequestOptions {
            cache_breakpoints: vec![CacheBreakpoint::MessageIndex(0)],
            ..Default::default()
        };
        let result = provider
            .chat_with_options_dyn("", &[ChatMessage::user("hi")], &opts, &Value::Null)
            .await;
        assert!(result.is_ok(), "expected Ok, got {result:?}");
    }

    /// Test C: MessageIndex out of range is silently skipped — request succeeds and
    /// the body contains NO cache_control field.
    #[tokio::test(flavor = "multi_thread")]
    async fn cache_breakpoint_message_index_out_of_range_skipped() {
        use crate::{CacheBreakpoint, ChatMessage, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use wiremock::matchers;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        // Use a body matcher that captures the raw body so we can inspect it.
        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "x",
                "content": [{"type": "text", "text": "ok"}],
                "model": "claude-test",
                "usage": {"input_tokens": 1, "output_tokens": 1},
                "stop_reason": "end_turn"
            })))
            .mount(&server)
            .await;

        let provider = ClaudeProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let opts = LlmRequestOptions {
            cache_breakpoints: vec![CacheBreakpoint::MessageIndex(99)],
            ..Default::default()
        };
        // Should succeed (out-of-range skipped silently)
        let result = provider
            .chat_with_options_dyn("", &[ChatMessage::user("hi")], &opts, &Value::Null)
            .await;
        assert!(
            result.is_ok(),
            "expected Ok when MessageIndex out of range, got {result:?}"
        );

        // Verify no cache_control appeared in the request body via received requests
        let received = server.received_requests().await.unwrap_or_default();
        assert_eq!(received.len(), 1, "expected exactly 1 request");
        let body_str = std::str::from_utf8(&received[0].body).unwrap_or("");
        assert!(
            !body_str.contains("cache_control"),
            "cache_control should NOT appear in body when MessageIndex is out of range; body: {body_str}"
        );
    }

    /// Test D: 5 breakpoints → only 4 cache_control markers in the request body.
    #[tokio::test(flavor = "multi_thread")]
    async fn cache_breakpoints_truncated_at_4() {
        use crate::{CacheBreakpoint, ChatMessage, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "x",
                "content": [{"type": "text", "text": "ok"}],
                "model": "claude-test",
                "usage": {"input_tokens": 1, "output_tokens": 1},
                "stop_reason": "end_turn"
            })))
            .mount(&server)
            .await;

        let provider = ClaudeProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        // 5 breakpoints: System + MessageIndex 0..3 (4 messages provided)
        let messages = vec![
            ChatMessage::user("msg0"),
            ChatMessage::user("msg1"),
            ChatMessage::user("msg2"),
            ChatMessage::user("msg3"),
        ];
        let opts = LlmRequestOptions {
            cache_breakpoints: vec![
                CacheBreakpoint::System,
                CacheBreakpoint::MessageIndex(0),
                CacheBreakpoint::MessageIndex(1),
                CacheBreakpoint::MessageIndex(2),
                CacheBreakpoint::MessageIndex(3),
            ],
            ..Default::default()
        };
        let result = provider
            .chat_with_options_dyn("system", &messages, &opts, &Value::Null)
            .await;
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let received = server.received_requests().await.unwrap_or_default();
        assert_eq!(received.len(), 1, "expected exactly 1 request");
        let body_str = std::str::from_utf8(&received[0].body).unwrap_or("");
        let count = body_str.matches("cache_control").count();
        assert_eq!(
            count, 4,
            "expected exactly 4 cache_control markers (truncated from 5), got {count}; body: {body_str}"
        );
    }

    /// Test E: Response with cache token counts populates LlmResponse fields.
    #[tokio::test(flavor = "multi_thread")]
    async fn cache_token_counts_extracted_from_response() {
        use crate::{ChatMessage, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

        let fixture = include_str!("../tests/fixtures/claude_cache_response.json");

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(fixture)
                    .insert_header("content-type", "application/json"),
            )
            .mount(&server)
            .await;

        let provider =
            ClaudeProvider::new_with_base_url("k".into(), "claude-sonnet-4-5".into(), server.uri());
        let resp = provider
            .chat_with_options_dyn(
                "",
                &[ChatMessage::user("hi")],
                &LlmRequestOptions::default(),
                &Value::Null,
            )
            .await
            .expect("expected Ok response");

        assert_eq!(
            resp.cache_creation_input_tokens,
            Some(200),
            "cache_creation_input_tokens should be Some(200)"
        );
        assert_eq!(
            resp.cache_read_input_tokens,
            Some(5000),
            "cache_read_input_tokens should be Some(5000)"
        );
        assert_eq!(resp.rendered_text, "Cached response.");
    }

    /// Test F: Default options (no breakpoints) → request body has NO cache_control.
    #[tokio::test(flavor = "multi_thread")]
    async fn cache_options_ignored_when_breakpoints_empty() {
        use crate::{ChatMessage, LlmProviderDyn, LlmRequestOptions};
        use serde_json::Value;
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "x",
                "content": [{"type": "text", "text": "ok"}],
                "model": "claude-test",
                "usage": {"input_tokens": 1, "output_tokens": 1},
                "stop_reason": "end_turn"
            })))
            .mount(&server)
            .await;

        let provider = ClaudeProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let result = provider
            .chat_with_options_dyn(
                "",
                &[ChatMessage::user("hi")],
                &LlmRequestOptions::default(),
                &Value::Null,
            )
            .await;
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let received = server.received_requests().await.unwrap_or_default();
        assert_eq!(received.len(), 1, "expected exactly 1 request");
        let body_str = std::str::from_utf8(&received[0].body).unwrap_or("");
        assert!(
            !body_str.contains("cache_control"),
            "cache_control should NOT appear in body when breakpoints is empty; body: {body_str}"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Task 10: Progressive LlmStreamEvent::Usage emission (50-token throttle)
    // ──────────────────────────────────────────────────────────────────────────

    /// SSE stream with multiple message_delta events at output_tokens=10, 60, 110, 200.
    ///
    /// Throttle decisions (threshold = 50, last_emitted starts at 0):
    ///   output=10  → delta = 10 - 0 = 10  < 50 → SKIP
    ///   output=60  → delta = 60 - 0 = 60  ≥ 50 → EMIT  (last_emitted = 60)
    ///   output=110 → delta = 110 - 60 = 50 ≥ 50 → EMIT  (last_emitted = 110)
    ///   output=200 → delta = 200 - 110 = 90 ≥ 50 → EMIT  (last_emitted = 200)
    ///
    /// Expected: 3 Usage events. End.total.output_tokens = 200, input_tokens = 50.
    #[tokio::test(flavor = "multi_thread")]
    async fn streaming_emits_usage_events_with_throttle() {
        use crate::{ChatMessage, LlmProviderDyn, LlmRequestOptions, LlmStreamEvent};
        use futures::StreamExt;
        use serde_json::Value;
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

        // message_start gives input_tokens=50.
        // Four message_delta events at cumulative output_tokens 10, 60, 110, 200.
        // message_stop terminates the stream.
        let sse_body = concat!(
            "event: message_start\n",
            "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_01\",\"model\":\"claude-sonnet-4-5\",\"usage\":{\"input_tokens\":50,\"output_tokens\":1}}}\n\n",
            // Text block
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            // message_delta at output=10  → delta from 0 = 10  < 50 → SKIP
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{},\"usage\":{\"output_tokens\":10}}\n\n",
            // message_delta at output=60  → delta from 0 = 60  ≥ 50 → EMIT (1st)
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{},\"usage\":{\"output_tokens\":60}}\n\n",
            // message_delta at output=110 → delta from 60 = 50 ≥ 50 → EMIT (2nd)
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{},\"usage\":{\"output_tokens\":110}}\n\n",
            // message_delta at output=200 → delta from 110 = 90 ≥ 50 → EMIT (3rd)
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":200}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n",
        );

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(sse_body)
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;

        // Use "claude-sonnet-4-5" so estimate_cost returns non-zero (sonnet rate = $3/$15 per MT)
        let provider =
            ClaudeProvider::new_with_base_url("k".into(), "claude-sonnet-4-5".into(), server.uri());
        let msgs = vec![ChatMessage::user("count")];
        let opts = LlmRequestOptions::default();
        let mut stream = provider.chat_stream_with_options_dyn("", &msgs, &opts, &Value::Null);

        let mut usage_events: Vec<(Option<u32>, u32, f64)> = Vec::new(); // (input_tokens, output_tokens, cost)
        let mut end_total: Option<LlmResponse> = None;

        while let Some(event) = stream.next().await {
            match event {
                LlmStreamEvent::Usage {
                    input_tokens,
                    output_tokens,
                    estimated_cost_usd,
                } => {
                    usage_events.push((input_tokens, output_tokens, estimated_cost_usd));
                }
                LlmStreamEvent::End { total } => {
                    end_total = Some(total);
                    break;
                }
                LlmStreamEvent::Error { message } => panic!("unexpected error: {message}"),
                _ => {}
            }
        }

        // Throttle produces exactly 3 Usage events (skip output=10, emit output=60/110/200).
        assert_eq!(
            usage_events.len(),
            3,
            "expected 3 Usage events (skip delta=10, emit delta=60, 50, 90); got: {usage_events:?}"
        );

        // First emitted at output=60
        assert_eq!(
            usage_events[0].1, 60,
            "first Usage should be at output_tokens=60"
        );
        assert_eq!(
            usage_events[0].0,
            Some(50),
            "input_tokens should be 50 from message_start"
        );
        assert!(
            usage_events[0].2 > 0.0,
            "estimated_cost_usd should be positive for claude-sonnet-4-5"
        );

        // Second emitted at output=110
        assert_eq!(
            usage_events[1].1, 110,
            "second Usage should be at output_tokens=110"
        );

        // Third emitted at output=200
        assert_eq!(
            usage_events[2].1, 200,
            "third Usage should be at output_tokens=200"
        );

        // End total carries the final cumulative values
        let total = end_total.expect("expected End event");
        assert_eq!(
            total.output_tokens,
            Some(200),
            "End.total.output_tokens should be 200 (final cumulative)"
        );
        assert_eq!(
            total.input_tokens,
            Some(50),
            "End.total.input_tokens should be 50 from message_start"
        );
        assert!(
            total.estimated_cost_usd > 0.0,
            "End.total.estimated_cost_usd should be positive for claude-sonnet-4-5"
        );
    }

    /// Unknown model name falls back to the default pricing tier (non-zero),
    /// so this test verifies that estimate_cost degrades gracefully to a
    /// non-panic, non-negative value — and that Usage events ARE emitted
    /// even when the model is not in the pricing table.
    #[tokio::test(flavor = "multi_thread")]
    async fn streaming_usage_with_unknown_model_returns_nonzero_cost() {
        use crate::{ChatMessage, LlmProviderDyn, LlmRequestOptions, LlmStreamEvent};
        use futures::StreamExt;
        use serde_json::Value;
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

        // Single message_delta at output=60 → delta from 0 = 60 ≥ 50 → EMIT.
        let sse_body = concat!(
            "event: message_start\n",
            "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n\n",
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            // Single delta at output=60 → EMIT
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":60}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n",
        );

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(sse_body)
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;

        // "claude-foo" is not in the pricing table; falls back to sonnet default rates
        // (3.0 / 15.0 per MTok) — still non-zero.
        let provider =
            ClaudeProvider::new_with_base_url("k".into(), "claude-foo".into(), server.uri());
        let msgs = vec![ChatMessage::user("hi")];
        let opts = LlmRequestOptions::default();
        let mut stream = provider.chat_stream_with_options_dyn("", &msgs, &opts, &Value::Null);

        let mut usage_events: Vec<(u32, f64)> = Vec::new();

        while let Some(event) = stream.next().await {
            match event {
                LlmStreamEvent::Usage {
                    output_tokens,
                    estimated_cost_usd,
                    ..
                } => {
                    usage_events.push((output_tokens, estimated_cost_usd));
                }
                LlmStreamEvent::End { .. } | LlmStreamEvent::Error { .. } => break,
                _ => {}
            }
        }

        assert_eq!(
            usage_events.len(),
            1,
            "expected exactly 1 Usage event at output=60"
        );
        assert_eq!(usage_events[0].0, 60);
        // estimate_cost falls back to sonnet default ($3/$15) — cost is positive, not 0
        assert!(
            usage_events[0].1 > 0.0,
            "estimate_cost should fall back to default (non-zero) for unknown model 'claude-foo'"
        );
    }
}
