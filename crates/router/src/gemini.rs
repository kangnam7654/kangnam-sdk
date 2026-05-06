//! Gemini HTTP provider (Google Generative Language / Cloud Code API).
//!
//! # Image input
//!
//! `ChatContent::Image` with `ImageSource::Base64` is encoded as an `inlineData`
//! part. URL-based image sources (`ImageSource::Url`) are NOT supported —
//! Gemini requires public images to be uploaded via the Files API first to
//! obtain a `gs://` or `files/...` URI, which is out of scope for v0.4.0.
//! Passing `ImageSource::Url` logs a warning and skips the image.
//!
//! ## Tool calling — Gemini-specific conventions
//!
//! Gemini's tool call protocol differs from Anthropic and OpenAI:
//!
//! * **Request**: all function definitions are grouped under a single
//!   `tools[0].functionDeclarations` array, not one object per tool.
//!   Tool-choice maps via `toolConfig.functionCallingConfig.mode`:
//!   `Auto` → `"AUTO"`, `Required` → `"ANY"`, `None` → `"NONE"`,
//!   `Specific(name)` → `"ANY"` + `allowedFunctionNames: [name]`.
//!
//! * **Response**: `functionCall` parts carry `{name, args}`.
//!   Gemini has **no stable per-call ID** — `args` is a complete JSON object
//!   (not a string). We expose `id == name` in [`ToolCall`].
//!
//! * **Tool result feedback**: because there is no stable call ID, Gemini
//!   pairs `functionResponse` parts with the preceding `functionCall` by
//!   **name**, not id. Callers must therefore pass the **function name** as
//!   `tool_use_id` when constructing
//!   [`ChatMessage::tool_result`](crate::ChatMessage::tool_result):
//!
//!   ```ignore
//!   // ToolCall from Gemini: tc.id == tc.name == "get_weather"
//!   ChatMessage::tool_result(tc.id /* == name */, result_json, false)
//!   ```
//!
//! * **Streaming**: `functionCall` parts arrive as complete objects inside
//!   each SSE chunk — no accumulation across chunks is needed. A
//!   [`LlmStreamEvent::ToolCall`] is emitted once per `functionCall` part,
//!   immediately when encountered.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tokio::sync::Mutex;

use super::{
    ChatContent, ChatMessage, LlmError, LlmProviderDyn, LlmResponse, LlmStreamEvent, ToolCall,
};

const GEMINI_API_URL: &str =
    "https://cloudcode-pa.googleapis.com/v1internal:streamGenerateContent?alt=sse";
const GEMINI_MODELS_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";
/// Code Assist onboarding endpoints. Calling `streamGenerateContent`
/// directly without first registering the user (and learning their
/// `cloudaicompanionProject`) returns 500 for non-FREE-tier accounts.
/// gemini-cli does this dance on every cold start; we mirror it here.
/// See `packages/core/src/code_assist/setup.ts` in google-gemini/gemini-cli.
const CODE_ASSIST_LOAD_URL: &str =
    "https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist";
const CODE_ASSIST_ONBOARD_URL: &str =
    "https://cloudcode-pa.googleapis.com/v1internal:onboardUser";
const CODE_ASSIST_OPERATION_URL_PREFIX: &str =
    "https://cloudcode-pa.googleapis.com/v1internal/";
const REQUEST_TIMEOUT_SECS: u64 = 60;
const MAX_OUTPUT_TOKENS: u32 = 1024;
const ONBOARD_POLL_INTERVAL_SECS: u64 = 2;
const ONBOARD_POLL_MAX_ATTEMPTS: u32 = 60; // 60 * 2s = 2 min

pub struct GeminiProvider {
    client: Client,
    token: String,
    model: String,
    base_url: String,
    /// Cached `cloudaicompanionProject` resolved from the Code Assist
    /// onboarding flow. First request populates this; subsequent
    /// requests reuse the cached id. Caching is per-provider-instance —
    /// callers that re-create the provider per request (e.g.
    /// lunawave's per-turn factory) pay one onboarding round-trip per
    /// turn. Acceptable: `loadCodeAssist` for an already-onboarded
    /// user is a single ~200ms POST, and the tradeoff buys robustness
    /// against project-id changes.
    project_id: Mutex<Option<String>>,
}

// -- Request types --

#[derive(Serialize)]
struct GeminiEnvelope {
    model: String,
    /// `cloudaicompanionProject` for the streamGenerateContent call.
    /// Resolved by [`GeminiProvider::ensure_project_id`] from the
    /// Code Assist onboarding flow. Required for non-FREE-tier
    /// accounts (Workspace / paid Google AI Studio plans). FREE-tier
    /// users get this populated server-side, but we always send it
    /// for symmetry — the server ignores the field on FREE.
    #[serde(skip_serializing_if = "Option::is_none")]
    project: Option<String>,
    request: GeminiRequest,
}

// -- Code Assist onboarding types --

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ClientMetadata {
    ide_type: String,
    platform: String,
    plugin_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    duet_project: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadCodeAssistRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    cloudaicompanion_project: Option<String>,
    metadata: ClientMetadata,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct LoadCodeAssistResponse {
    cloudaicompanion_project: Option<String>,
    current_tier: Option<TierInfo>,
    allowed_tiers: Option<Vec<TierInfo>>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
struct TierInfo {
    id: Option<String>,
    is_default: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OnboardUserRequest {
    tier_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cloudaicompanion_project: Option<String>,
    metadata: ClientMetadata,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct LongRunningOperation {
    name: Option<String>,
    #[serde(default)]
    done: bool,
    response: Option<LroResponse>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct LroResponse {
    cloudaicompanion_project: Option<LroProjectId>,
}

#[derive(Deserialize, Default)]
struct LroProjectId {
    id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    system_instruction: GeminiSystemInstruction,
    generation_config: GeminiGenerationConfig,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<GeminiToolWrapper>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "toolConfig")]
    tool_config: Option<GeminiToolConfig>,
}

#[derive(Serialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

/// A request part: plain text, an `inlineData` image block, or a
/// `functionResponse` feedback block.
///
/// `functionResponse` is used to return a tool result to the model. The
/// `name` must match the function name from the preceding `functionCall`
/// (Gemini pairs by name, not id — see module-level docs).
///
/// `inlineData` carries a base64-encoded image together with its MIME type.
/// URL-based images are not supported here — see module-level docs.
#[derive(Serialize)]
#[serde(untagged)]
enum GeminiPart {
    Text {
        text: String,
    },
    InlineData {
        #[serde(rename = "inlineData")]
        inline_data: GeminiInlineData,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: GeminiFunctionResponse,
    },
    /// Replays an assistant turn's tool invocation. Gemini pairs by
    /// function `name`, not by id, so the originating
    /// [`crate::ChatContent::ToolUse::name`] must match the responding
    /// [`crate::ChatContent::ToolResult::tool_use_id`].
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: GeminiFunctionCallOut,
    },
}

#[derive(Serialize)]
struct GeminiFunctionCallOut {
    name: String,
    args: serde_json::Value,
}

#[derive(Serialize)]
struct GeminiInlineData {
    #[serde(rename = "mimeType")]
    mime_type: String,
    data: String,
}

#[derive(Serialize)]
struct GeminiFunctionResponse {
    name: String,
    response: GeminiFunctionResponseBody,
}

#[derive(Serialize)]
struct GeminiFunctionResponseBody {
    content: String,
}

#[derive(Serialize)]
struct GeminiSystemInstruction {
    parts: Vec<GeminiTextPart>,
}

#[derive(Serialize)]
struct GeminiTextPart {
    text: String,
}

/// Gemini 3.x thinking configuration.
///
/// Verified against the Gemini API docs (https://ai.google.dev/gemini-api/docs/thinking,
/// fetched 2026-04-25):
///
/// - `thinkingLevel` is the correct field name (inside `thinkingConfig` nested object).
///   Values: `"low"` | `"medium"` | `"high"`. Recommended for Gemini 3 models.
/// - `includeThoughts: true` is required to receive thought summaries in the response.
///
/// Request JSON wire shape:
/// ```json
/// {
///   "generationConfig": {
///     "thinkingConfig": {
///       "thinkingLevel": "high",
///       "includeThoughts": true
///     }
///   }
/// }
/// ```
///
/// Response shape (shape (a) — confirmed): `candidates[0].content.parts[]` where
/// each part with `"thought": true` contains the thinking text (alongside the
/// regular text parts with no `thought` field or `"thought": false`).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiThinkingConfig {
    /// Controls the depth of thinking. One of `"low"`, `"medium"`, or `"high"`.
    /// Mapped from `LlmRequestOptions.reasoning_effort`.
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking_level: Option<String>,
    /// Must be `true` to receive thought summaries in the response parts.
    /// Always set when `thinking_level` is present.
    include_thoughts: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiGenerationConfig {
    candidate_count: u32,
    temperature: f32,
    max_output_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none", rename = "topP")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty", rename = "stopSequences")]
    stop_sequences: Vec<String>,
    /// Present only when `reasoning_effort` or `thinking_budget_tokens` is set.
    /// Gemini 3.x: `thinkingConfig.thinkingLevel` controls thinking depth.
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking_config: Option<GeminiThinkingConfig>,
}

/// Outer wrapper: all function definitions go inside `functionDeclarations`
/// of a **single** element in the `tools` array.
#[derive(Serialize)]
struct GeminiToolWrapper {
    #[serde(rename = "functionDeclarations")]
    function_declarations: Vec<GeminiFunctionDecl>,
}

#[derive(Serialize)]
struct GeminiFunctionDecl {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize)]
struct GeminiToolConfig {
    #[serde(rename = "functionCallingConfig")]
    function_calling_config: GeminiFunctionCallingConfig,
}

#[derive(Serialize)]
struct GeminiFunctionCallingConfig {
    /// One of `"AUTO"`, `"ANY"`, or `"NONE"`.
    mode: String,
    #[serde(skip_serializing_if = "Vec::is_empty", rename = "allowedFunctionNames")]
    allowed_function_names: Vec<String>,
}

// -- Response types --

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<GeminiUsageMetadata>,
}

#[derive(Deserialize, Default)]
struct GeminiUsageMetadata {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: Option<u32>,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: Option<u32>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiCandidateContent>,
}

#[derive(Deserialize)]
struct GeminiCandidateContent {
    parts: Option<Vec<GeminiResponsePart>>,
}

/// A response part: either a text fragment, a `functionCall` invocation, or a
/// Gemini 3.x thinking/thought part.
///
/// Gemini 3.x thinking parts (verified from API docs, 2026-04-25):
/// - `thought: true` marks a part as a reasoning/thinking summary.
/// - `text` carries the thought text when `thought == true`.
/// - Regular answer parts have no `thought` field (deserializes as `None` / `false`).
#[derive(Deserialize)]
struct GeminiResponsePart {
    text: Option<String>,
    /// `true` when this part contains Gemini 3.x thinking/reasoning content.
    /// Absent in regular text parts (defaults to `false` via `#[serde(default)]`).
    #[serde(default)]
    thought: bool,
    #[serde(rename = "functionCall")]
    function_call: Option<GeminiFunctionCall>,
}

#[derive(Deserialize)]
struct GeminiFunctionCall {
    name: String,
    args: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

impl GeminiProvider {
    pub fn new(token: String, model: String) -> Self {
        Self::new_with_base_url(token, model, GEMINI_API_URL.to_string())
    }

    /// For testing only — allows pointing the provider at a mock HTTP server.
    /// Production callers should use [`Self::new`], which defaults to
    /// `GEMINI_API_URL`
    /// (`https://cloudcode-pa.googleapis.com/v1internal:streamGenerateContent?alt=sse`).
    ///
    /// **Important:** the default `GEMINI_API_URL` embeds the `?alt=sse`
    /// query parameter required by the real Google endpoint. Callers passing
    /// a bare `base_url` (e.g. a wiremock server URI without a query string)
    /// bypass that parameter — acceptable for tests where the mock does not
    /// enforce SSE negotiation.
    pub fn new_with_base_url(token: String, model: String, base_url: String) -> Self {
        let model = if model.is_empty() {
            "gemini-3.1-pro-preview".to_string()
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
            project_id: Mutex::new(None),
        }
    }

    /// Resolve the user's `cloudaicompanionProject` via Code Assist
    /// onboarding. Cached after first successful resolution; failures
    /// don't poison the cache (the next request retries).
    ///
    /// Order of preference:
    /// 1. `GOOGLE_CLOUD_PROJECT` / `GOOGLE_CLOUD_PROJECT_ID` env vars
    ///    (matches gemini-cli — Workspace accounts require an explicit
    ///    project that the server can't discover).
    /// 2. `loadCodeAssist` response's `cloudaicompanionProject` (the
    ///    user has been onboarded before — gemini-cli or another
    ///    client did the onboarding).
    /// 3. Walk `allowed_tiers` for the default tier and call
    ///    `onboardUser` (long-running operation; poll until done).
    async fn ensure_project_id(&self) -> Result<String, LlmError> {
        // Tests point base_url at a wiremock URL (no `cloudcode-pa`).
        // Onboarding hits the production Code Assist endpoint which
        // mocks don't simulate — skip in that path. Production
        // callers always go through GEMINI_API_URL which contains
        // `cloudcode-pa.googleapis.com`.
        if !self.base_url.contains("cloudcode-pa.googleapis.com") {
            return Ok(String::new());
        }
        let mut guard = self.project_id.lock().await;
        if let Some(p) = guard.as_ref() {
            return Ok(p.clone());
        }
        let p = self.setup_user().await?;
        *guard = Some(p.clone());
        Ok(p)
    }

    async fn setup_user(&self) -> Result<String, LlmError> {
        let env_project = std::env::var("GOOGLE_CLOUD_PROJECT")
            .ok()
            .or_else(|| std::env::var("GOOGLE_CLOUD_PROJECT_ID").ok())
            .filter(|s| !s.is_empty());

        let metadata = ClientMetadata {
            ide_type: "IDE_UNSPECIFIED".into(),
            platform: "PLATFORM_UNSPECIFIED".into(),
            plugin_type: "GEMINI".into(),
            duet_project: env_project.clone(),
        };

        let load_resp = self.load_code_assist(&env_project, &metadata).await?;

        // 1. Server already knows the project — short-circuit.
        if let Some(p) = load_resp.cloudaicompanion_project.clone() {
            tracing::debug!(project = %p, "gemini: using existing cloudaicompanionProject");
            return Ok(p);
        }

        // 2. Caller pre-supplied a project AND has a tier (Workspace path).
        if let (Some(p), Some(_)) = (env_project.clone(), load_resp.current_tier.as_ref()) {
            tracing::debug!(project = %p, "gemini: using env-supplied project");
            return Ok(p);
        }

        // 3. Onboard the user. Pick the default-marked tier from the
        //    server response or fall back to "legacy-tier".
        let tier_id = load_resp
            .allowed_tiers
            .as_ref()
            .and_then(|tiers| tiers.iter().find(|t| t.is_default == Some(true)))
            .and_then(|t| t.id.clone())
            .unwrap_or_else(|| "legacy-tier".to_string());

        tracing::info!(tier = %tier_id, "gemini: onboarding user via Code Assist");
        self.onboard_user(&tier_id, &env_project, &metadata).await
    }

    async fn load_code_assist(
        &self,
        project: &Option<String>,
        metadata: &ClientMetadata,
    ) -> Result<LoadCodeAssistResponse, LlmError> {
        let req = LoadCodeAssistRequest {
            cloudaicompanion_project: project.clone(),
            metadata: metadata.clone(),
        };
        let resp = self
            .client
            .post(CODE_ASSIST_LOAD_URL)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("content-type", "application/json")
            .json(&req)
            .send()
            .await
            .map_err(|e| LlmError::Network {
                provider: "gemini".into(),
                msg: format!("loadCodeAssist: {e}"),
            })?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            tracing::error!(status = %status, body = %body, "loadCodeAssist failed");
            return Err(match status.as_u16() {
                401 | 403 => LlmError::Auth {
                    provider: "gemini".into(),
                },
                s => LlmError::Upstream {
                    provider: "gemini".into(),
                    status: s,
                    body,
                },
            });
        }
        resp.json::<LoadCodeAssistResponse>()
            .await
            .map_err(|e| LlmError::Network {
                provider: "gemini".into(),
                msg: format!("loadCodeAssist parse: {e}"),
            })
    }

    async fn onboard_user(
        &self,
        tier_id: &str,
        env_project: &Option<String>,
        metadata: &ClientMetadata,
    ) -> Result<String, LlmError> {
        // FREE tier uses a managed project — sending project causes
        // a `Precondition Failed` from Code Assist. Other tiers
        // require the env-supplied project.
        let is_free = tier_id == "free-tier";
        let project_for_onboard = if is_free { None } else { env_project.clone() };
        let metadata_for_onboard = if is_free {
            ClientMetadata {
                duet_project: None,
                ..metadata.clone()
            }
        } else {
            metadata.clone()
        };

        let req = OnboardUserRequest {
            tier_id: tier_id.to_string(),
            cloudaicompanion_project: project_for_onboard,
            metadata: metadata_for_onboard,
        };

        let resp = self
            .client
            .post(CODE_ASSIST_ONBOARD_URL)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("content-type", "application/json")
            .json(&req)
            .send()
            .await
            .map_err(|e| LlmError::Network {
                provider: "gemini".into(),
                msg: format!("onboardUser: {e}"),
            })?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            tracing::error!(status = %status, body = %body, "onboardUser failed");
            return Err(match status.as_u16() {
                401 | 403 => LlmError::Auth {
                    provider: "gemini".into(),
                },
                s => LlmError::Upstream {
                    provider: "gemini".into(),
                    status: s,
                    body,
                },
            });
        }
        let mut lro: LongRunningOperation = resp.json().await.map_err(|e| LlmError::Network {
            provider: "gemini".into(),
            msg: format!("onboardUser parse: {e}"),
        })?;

        let mut polls = 0;
        while !lro.done {
            if polls >= ONBOARD_POLL_MAX_ATTEMPTS {
                return Err(LlmError::Upstream {
                    provider: "gemini".into(),
                    status: 504,
                    body: "Code Assist onboarding timed out".into(),
                });
            }
            let Some(name) = lro.name.clone() else { break };
            polls += 1;
            tokio::time::sleep(Duration::from_secs(ONBOARD_POLL_INTERVAL_SECS)).await;
            let url = format!("{CODE_ASSIST_OPERATION_URL_PREFIX}{name}");
            let poll_resp = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", self.token))
                .send()
                .await
                .map_err(|e| LlmError::Network {
                    provider: "gemini".into(),
                    msg: format!("getOperation: {e}"),
                })?;
            if !poll_resp.status().is_success() {
                let body = poll_resp.text().await.unwrap_or_default();
                return Err(LlmError::Upstream {
                    provider: "gemini".into(),
                    status: 500,
                    body: format!("onboarding poll failed: {body}"),
                });
            }
            lro = poll_resp.json().await.map_err(|e| LlmError::Network {
                provider: "gemini".into(),
                msg: format!("getOperation parse: {e}"),
            })?;
        }

        // Prefer the project the LRO returned; fall back to env if
        // the response didn't carry one (some tier paths skip it).
        lro.response
            .and_then(|r| r.cloudaicompanion_project)
            .map(|p| p.id)
            .or_else(|| env_project.clone())
            .ok_or_else(|| LlmError::Upstream {
                provider: "gemini".into(),
                status: 500,
                body: "onboardUser returned no projectId".into(),
            })
    }

    /// Build Gemini `contents` from [`ChatMessage`] slice.
    ///
    /// Each message maps to one [`GeminiContent`] element. The parts builder
    /// handles three content kinds:
    ///
    /// - [`ChatContent::Text`] → `{"text": "..."}` part.
    /// - [`ChatContent::Image`] with [`crate::ImageSource::Base64`] →
    ///   `{"inlineData": {"mimeType": "...", "data": "..."}}` part.
    ///   [`crate::ImageSource::Url`] logs a warning and the part is skipped
    ///   (see module-level docs for the Files API limitation).
    /// - [`ChatContent::ToolResult`] → `{"functionResponse": {"name": tool_use_id,
    ///   "response": {"content": "..."}}}` part. If `is_error == true`, the
    ///   content string is prefixed with `"Error: "` (same convention as other
    ///   providers). **`tool_use_id` must be the function name** — Gemini pairs
    ///   by name, not id (see module-level docs).
    fn build_contents(messages: &[ChatMessage]) -> Vec<GeminiContent> {
        messages
            .iter()
            .map(|m| {
                let role = if m.role == "assistant" {
                    "model"
                } else {
                    "user"
                };
                let parts: Vec<GeminiPart> = m
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        ChatContent::Text(t) => Some(GeminiPart::Text { text: t.clone() }),
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
                            Some(GeminiPart::FunctionResponse {
                                function_response: GeminiFunctionResponse {
                                    name: tool_use_id.clone(),
                                    response: GeminiFunctionResponseBody {
                                        content: wire_content,
                                    },
                                },
                            })
                        }
                        ChatContent::Image { source, mime_type } => match source {
                            crate::ImageSource::Base64(data) => Some(GeminiPart::InlineData {
                                inline_data: GeminiInlineData {
                                    mime_type: mime_type.clone(),
                                    data: data.clone(),
                                },
                            }),
                            crate::ImageSource::Url(url) => {
                                tracing::warn!(
                                    "gemini provider does not support URL image sources; \
                                         URL would need Files API upload first. \
                                         Skipping image: {}",
                                    url
                                );
                                None
                            }
                        },
                        ChatContent::ToolUse {
                            name, arguments, ..
                        } => Some(GeminiPart::FunctionCall {
                            function_call: GeminiFunctionCallOut {
                                name: name.clone(),
                                args: arguments.clone(),
                            },
                        }),
                    })
                    .collect();
                GeminiContent {
                    role: role.into(),
                    parts,
                }
            })
            .collect()
    }

    /// Build the `tools` array (one wrapper with all function declarations).
    fn build_tools(options: &crate::LlmRequestOptions) -> Vec<GeminiToolWrapper> {
        if options.tools.is_empty() {
            return Vec::new();
        }
        let decls = options
            .tools
            .iter()
            .map(|t| GeminiFunctionDecl {
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: t.input_schema.clone(),
            })
            .collect();
        vec![GeminiToolWrapper {
            function_declarations: decls,
        }]
    }

    /// Build `toolConfig` from the `tool_choice` option.
    fn build_tool_config(options: &crate::LlmRequestOptions) -> Option<GeminiToolConfig> {
        use crate::ToolChoice;
        let choice = options.tool_choice.as_ref()?;
        let (mode, allowed) = match choice {
            ToolChoice::Auto => ("AUTO".to_string(), Vec::new()),
            ToolChoice::Required => ("ANY".to_string(), Vec::new()),
            ToolChoice::None => ("NONE".to_string(), Vec::new()),
            ToolChoice::Specific(name) => ("ANY".to_string(), vec![name.clone()]),
        };
        Some(GeminiToolConfig {
            function_calling_config: GeminiFunctionCallingConfig {
                mode,
                allowed_function_names: allowed,
            },
        })
    }

    /// Map `LlmRequestOptions.reasoning_effort` and `thinking_budget_tokens`
    /// to a Gemini 3.x `thinkingConfig` object.
    ///
    /// Mapping (per spec):
    /// - `reasoning_effort: Some("low"|"medium"|"high")` → `thinkingLevel` of same value.
    /// - `thinking_budget_tokens: Some(_)` with no `reasoning_effort` → `thinkingLevel: "high"`.
    /// - Neither set → `None` (no `thinkingConfig` in the request body).
    ///
    /// Wire format verified against https://ai.google.dev/gemini-api/docs/thinking (2026-04-25):
    /// `generationConfig.thinkingConfig.thinkingLevel` + `includeThoughts: true`.
    fn build_thinking_config(options: &crate::LlmRequestOptions) -> Option<GeminiThinkingConfig> {
        let level = if let Some(effort) = &options.reasoning_effort {
            match effort.as_str() {
                "low" | "medium" | "high" => Some(effort.clone()),
                _ => None,
            }
        } else if options.thinking_budget_tokens.is_some() {
            // Any budget token hint → treat as "high" effort (mirrors codex pattern).
            Some("high".to_string())
        } else {
            None
        };

        level.map(|thinking_level| GeminiThinkingConfig {
            thinking_level: Some(thinking_level),
            include_thoughts: true,
        })
    }

    /// Parse `functionCall` parts from the first candidate into [`ToolCall`]s.
    ///
    /// Because Gemini has no stable per-call id, `ToolCall.id` is set to the
    /// function name. Callers should pass `ToolCall.id` (== name) as
    /// `tool_use_id` in the subsequent [`ChatMessage::tool_result`].
    fn parse_tool_calls(resp: &GeminiResponse) -> Vec<ToolCall> {
        resp.candidates
            .as_ref()
            .and_then(|c| c.first())
            .and_then(|c| c.content.as_ref())
            .and_then(|c| c.parts.as_ref())
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|p| {
                        let fc = p.function_call.as_ref()?;
                        Some(ToolCall {
                            id: fc.name.clone(), // Gemini: id == name
                            name: fc.name.clone(),
                            arguments: fc.args.clone(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl LlmProviderDyn for GeminiProvider {
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
        let fut = self.chat_with_options_dyn(system_prompt, messages, options, result_json);
        Box::pin(async_stream::stream! {
            match fut.await {
                Ok(resp) => {
                    // Emit Gemini 3.x thinking text before the answer delta.
                    if let Some(ref thinking) = resp.thinking_text {
                        yield LlmStreamEvent::Thinking { text: thinking.clone() };
                    }
                    // Emit any tool calls encountered before the text delta.
                    for tc in &resp.tool_calls {
                        yield LlmStreamEvent::ToolCall { call: tc.clone() };
                    }
                    yield LlmStreamEvent::Delta { text: resp.rendered_text.clone() };
                    // Emit Usage before End so callers can display token counts
                    // after the blocking call completes. estimated_cost_usd is 0.0
                    // because llm-router has no built-in Gemini pricing table.
                    if let Some(out) = resp.output_tokens {
                        yield LlmStreamEvent::Usage {
                            input_tokens: resp.input_tokens,
                            output_tokens: out,
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

impl GeminiProvider {
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
        // Resolve `cloudaicompanionProject` once per provider lifetime.
        // The Code Assist endpoint returns 500 for non-FREE-tier users
        // who haven't been onboarded — gemini-cli does this dance on
        // every cold start, so we mirror it.
        let project = self.ensure_project_id().await?;

        let contents = Self::build_contents(messages);

        let body = GeminiEnvelope {
            model: self.model.clone(),
            // Empty string from `ensure_project_id` (test path) ⇒
            // skip the field entirely so the wire shape is the same
            // as before this PR for mocks that match exact JSON.
            project: if project.is_empty() {
                None
            } else {
                Some(project)
            },
            request: GeminiRequest {
                contents,
                system_instruction: GeminiSystemInstruction {
                    parts: vec![GeminiTextPart {
                        text: system_prompt.to_string(),
                    }],
                },
                generation_config: GeminiGenerationConfig {
                    candidate_count: 1,
                    temperature: options.temperature.unwrap_or(1.0),
                    max_output_tokens: options.max_tokens.unwrap_or(MAX_OUTPUT_TOKENS),
                    top_p: options.top_p,
                    stop_sequences: options.stop_sequences.clone(),
                    thinking_config: Self::build_thinking_config(options),
                },
                tools: Self::build_tools(options),
                tool_config: Self::build_tool_config(options),
            },
        };

        // ?alt=sse → returns SSE stream; fallback parsers handle both SSE and JSON array
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
                    provider: "gemini".into(),
                    msg: "timeout".into(),
                }
            } else {
                LlmError::Network {
                    provider: "gemini".into(),
                    msg: e.to_string(),
                }
            }
        })?;

        let status = resp.status();
        if !status.is_success() {
            let retry_after = crate::error::parse_retry_after(&resp);
            let err_body = resp.text().await.unwrap_or_default();
            tracing::error!(status = %status, body = %err_body, model = %self.model, "Gemini API error");
            return Err(match status.as_u16() {
                401 | 403 => LlmError::Auth {
                    provider: "gemini".into(),
                },
                429 => LlmError::RateLimit {
                    provider: "gemini".into(),
                    retry_after_secs: retry_after,
                },
                s => LlmError::Upstream {
                    provider: "gemini".into(),
                    status: s,
                    body: err_body,
                },
            });
        }

        let body_text = resp.text().await.map_err(|e| LlmError::Network {
            provider: "gemini".into(),
            msg: format!("failed to read response body: {e}"),
        })?;

        // Response can be a single object or an array of SSE-style chunks.
        // Try parsing as single response first, then as array, then SSE.
        let (text, thinking_text, tool_calls, input_tokens, output_tokens) =
            if let Ok(single) = serde_json::from_str::<GeminiResponse>(&body_text) {
                let tc = Self::parse_tool_calls(&single);
                let thinking = extract_gemini_thinking(&single);
                let (inp, out) = extract_usage_tokens(&single);
                (extract_gemini_text(&single), thinking, tc, inp, out)
            } else if let Ok(arr) = serde_json::from_str::<Vec<GeminiResponse>>(&body_text) {
                let text = arr
                    .iter()
                    .map(extract_gemini_text)
                    .collect::<Vec<_>>()
                    .join("");
                // Accumulate thinking text across all chunks.
                let thinking_chunks: Vec<String> =
                    arr.iter().filter_map(extract_gemini_thinking).collect();
                let thinking = if thinking_chunks.is_empty() {
                    None
                } else {
                    Some(thinking_chunks.join(""))
                };
                // Collect tool calls across all chunks (deduplicated by index is
                // not needed — each chunk carries distinct parts).
                let tcs: Vec<ToolCall> = arr.iter().flat_map(Self::parse_tool_calls).collect();
                // usageMetadata is cumulative — use the last chunk that has it.
                let (inp, out) = arr
                    .iter()
                    .rev()
                    .find(|r| r.usage_metadata.is_some())
                    .map(extract_usage_tokens)
                    .unwrap_or((None, None));
                (text, thinking, tcs, inp, out)
            } else {
                // Try SSE parsing as fallback.
                let (text, thinking, tcs, inp, out) = parse_gemini_sse(&body_text);
                (text, thinking, tcs, inp, out)
            };

        Ok(LlmResponse {
            rendered_text: text,
            model: self.model.clone(),
            estimated_cost_usd: 0.0,
            tool_calls,
            thinking_text,
            input_tokens,
            output_tokens,
            ..Default::default()
        })
    }
}

/// Extract the regular (non-thinking) text from a Gemini response.
///
/// Skips parts with `thought == true` (Gemini 3.x thinking parts) — those go
/// to `extract_gemini_thinking` instead.
fn extract_gemini_text(resp: &GeminiResponse) -> String {
    resp.candidates
        .as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.content.as_ref())
        .and_then(|c| c.parts.as_ref())
        .map(|parts| {
            parts
                .iter()
                // Only include non-thinking parts in the rendered text.
                .filter(|p| !p.thought)
                .filter_map(|p| p.text.as_deref())
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

/// Extract Gemini 3.x thinking/reasoning text from a response.
///
/// Returns `None` if no thinking parts are present (pre-3.x model or thinking
/// not requested), so callers can distinguish "no thinking" from empty thinking.
///
/// Verified shape (https://ai.google.dev/gemini-api/docs/thinking, 2026-04-25):
/// parts with `"thought": true` carry reasoning text. They are separate from
/// regular answer parts in the same `candidates[0].content.parts[]` array.
fn extract_gemini_thinking(resp: &GeminiResponse) -> Option<String> {
    let parts = resp
        .candidates
        .as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.content.as_ref())
        .and_then(|c| c.parts.as_ref())?;

    let thinking: String = parts
        .iter()
        .filter(|p| p.thought)
        .filter_map(|p| p.text.as_deref())
        .collect::<Vec<_>>()
        .join("");

    if thinking.is_empty() {
        None
    } else {
        Some(thinking)
    }
}

/// Extract input and output token counts from a Gemini response's `usageMetadata`.
///
/// Returns `(input_tokens, output_tokens)`. Both are `None` when `usageMetadata`
/// is absent (e.g., intermediate SSE chunks that carry no token summary yet).
fn extract_usage_tokens(resp: &GeminiResponse) -> (Option<u32>, Option<u32>) {
    match resp.usage_metadata.as_ref() {
        Some(u) => (u.prompt_token_count, u.candidates_token_count),
        None => (None, None),
    }
}

/// Parse SSE format: lines starting with `"data: "` contain JSON.
///
/// Returns `(accumulated_text, thinking_text, tool_calls, input_tokens, output_tokens)`.
/// Tool calls and thinking content across chunks are accumulated — each chunk
/// that contains the relevant parts contributes to the totals. Token counts are
/// cumulative per the Gemini API contract, so the last chunk that carries
/// `usageMetadata` holds the final totals.
///
/// Gemini 3.x thinking parts in SSE: same shape as blocking — each SSE chunk's
/// `candidates[0].content.parts[]` may contain `{"thought": true, "text": "..."}`.
fn parse_gemini_sse(
    body: &str,
) -> (
    String,
    Option<String>,
    Vec<ToolCall>,
    Option<u32>,
    Option<u32>,
) {
    let mut text = String::new();
    let mut thinking_acc = String::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    let mut last_input_tokens: Option<u32> = None;
    let mut last_output_tokens: Option<u32> = None;
    for line in body.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            if let Ok(resp) = serde_json::from_str::<GeminiResponse>(data) {
                text.push_str(&extract_gemini_text(&resp));
                if let Some(t) = extract_gemini_thinking(&resp) {
                    thinking_acc.push_str(&t);
                }
                tool_calls.extend(GeminiProvider::parse_tool_calls(&resp));
                let (inp, out) = extract_usage_tokens(&resp);
                if inp.is_some() {
                    last_input_tokens = inp;
                }
                if out.is_some() {
                    last_output_tokens = out;
                }
            }
        }
    }
    let thinking = if thinking_acc.is_empty() {
        None
    } else {
        Some(thinking_acc)
    };
    (
        text,
        thinking,
        tool_calls,
        last_input_tokens,
        last_output_tokens,
    )
}

/// List available Gemini models via the Google Gemini API.
///
/// Calls `GET https://generativelanguage.googleapis.com/v1beta/models?key={api_key}`
/// and parses the `models[]` array from the response.
pub async fn list_models(api_key: &str) -> Result<Vec<crate::ListModel>, LlmError> {
    list_models_with_base_url(api_key, GEMINI_MODELS_URL).await
}

/// Internal helper that accepts a custom base URL for testing.
pub(crate) async fn list_models_with_base_url(
    api_key: &str,
    base_url: &str,
) -> Result<Vec<crate::ListModel>, LlmError> {
    if api_key.is_empty() {
        return Err(LlmError::MissingConfig {
            provider: "gemini".into(),
            reason: "API key is empty".into(),
        });
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| LlmError::Other {
            provider: "gemini".into(),
            message: format!("failed to build HTTP client: {e}"),
        })?;

    let url = format!("{base_url}?key={api_key}");

    let resp = client
        .get(&url)
        .header("content-type", "application/json")
        .send()
        .await
        .map_err(|e| LlmError::Network {
            provider: "gemini".into(),
            msg: e.to_string(),
        })?;

    let status = resp.status();
    if !status.is_success() {
        let retry_after = crate::error::parse_retry_after(&resp);
        let err_body = resp.text().await.unwrap_or_default();
        return Err(match status.as_u16() {
            401 | 403 => LlmError::Auth {
                provider: "gemini".into(),
            },
            429 => LlmError::RateLimit {
                provider: "gemini".into(),
                retry_after_secs: retry_after,
            },
            s => LlmError::Upstream {
                provider: "gemini".into(),
                status: s,
                body: err_body,
            },
        });
    }

    let body: Value = resp.json().await.map_err(|e| LlmError::Parse {
        provider: "gemini".into(),
        reason: e.to_string(),
    })?;

    let models = body
        .get("models")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let name = m.get("name").and_then(|v| v.as_str())?.to_string();
                    Some(crate::ListModel {
                        name,
                        display_name: m
                            .get("displayName")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        description: m
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        input_token_limit: m
                            .get("inputTokenLimit")
                            .and_then(|v| v.as_i64())
                            .map(|v| v as i32),
                        output_token_limit: m
                            .get("outputTokenLimit")
                            .and_then(|v| v.as_i64())
                            .map(|v| v as i32),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(models)
}

pub fn make(
    api_key: &str,
    model: &str,
    _base_url: &str,
) -> Result<Box<dyn crate::LlmProviderDyn>, crate::LlmError> {
    if api_key.is_empty() {
        return Err(crate::LlmError::MissingConfig {
            provider: "gemini".into(),
            reason: "API key is empty".into(),
        });
    }
    Ok(Box::new(GeminiProvider::new(
        api_key.to_string(),
        model.to_string(),
    )))
}

#[cfg(test)]
mod list_models_tests {
    use super::*;

    fn mock_models_response() -> String {
        serde_json::json!({
            "models": [
                {
                    "name": "gemini-2.0-flash-exp",
                    "displayName": "Gemini 2.0 Flash Exp",
                    "description": "Gemini 2.0 Flash experimental model",
                    "inputTokenLimit": 1048576,
                    "outputTokenLimit": 8192
                },
                {
                    "name": "gemini-1.5-pro",
                    "displayName": "Gemini 1.5 Pro",
                    "description": "Gemini 1.5 Pro model",
                    "inputTokenLimit": 2097152,
                    "outputTokenLimit": 8192
                }
            ]
        })
        .to_string()
    }

    #[tokio::test]
    async fn list_models_returns_parsed_models() {
        let mock = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_string(mock_models_response()),
            )
            .mount(&mock)
            .await;

        let mock_url = mock.uri();
        let models = list_models_with_base_url("test-key", &mock_url)
            .await
            .expect("list_models should succeed");

        assert_eq!(models.len(), 2);
        assert_eq!(models[0].name, "gemini-2.0-flash-exp");
        assert_eq!(models[0].display_name, "Gemini 2.0 Flash Exp");
        assert_eq!(
            models[0].description,
            Some("Gemini 2.0 Flash experimental model".to_string())
        );
        assert_eq!(models[0].input_token_limit, Some(1048576));
        assert_eq!(models[0].output_token_limit, Some(8192));
        assert_eq!(models[1].name, "gemini-1.5-pro");
    }

    #[tokio::test]
    async fn list_models_empty_key_returns_missing_config() {
        let result = list_models("").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, LlmError::MissingConfig { ref provider, .. } if provider == "gemini"),
            "expected MissingConfig, got: {err:?}",
        );
    }

    #[tokio::test]
    async fn list_models_401_returns_auth_error() {
        let mock = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(401).set_body_string("unauthorized"))
            .mount(&mock)
            .await;

        let mock_url = mock.uri();
        let result = list_models_with_base_url("test-key", &mock_url).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, LlmError::Auth { ref provider } if provider == "gemini"),
            "expected Auth, got: {err:?}",
        );
    }

    #[tokio::test]
    async fn list_models_429_returns_rate_limit() {
        let mock = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(
                wiremock::ResponseTemplate::new(429)
                    .insert_header("retry-after", "30")
                    .set_body_string("rate limited"),
            )
            .mount(&mock)
            .await;

        let mock_url = mock.uri();
        let result = list_models_with_base_url("test-key", &mock_url).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            LlmError::RateLimit {
                provider,
                retry_after_secs,
            } => {
                assert_eq!(provider, "gemini");
                assert_eq!(retry_after_secs, Some(30));
            }
            other => panic!("expected RateLimit, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn list_models_invalid_json_returns_parse_error() {
        let mock = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("{invalid json"))
            .mount(&mock)
            .await;

        let mock_url = mock.uri();
        let result = list_models_with_base_url("test-key", &mock_url).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, LlmError::Parse { ref provider, .. } if provider == "gemini"),
            "expected Parse, got: {err:?}",
        );
    }

    #[tokio::test]
    async fn list_models_empty_models_array_returns_empty_vec() {
        let mock = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(r#"{"models": []}"#))
            .mount(&mock)
            .await;

        let mock_url = mock.uri();
        let models = list_models_with_base_url("test-key", &mock_url)
            .await
            .expect("list_models should succeed");

        assert!(models.is_empty());
    }

    #[tokio::test]
    async fn list_models_missing_models_field_returns_empty_vec() {
        let mock = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_string(r#"{"other": "data"}"#),
            )
            .mount(&mock)
            .await;

        let mock_url = mock.uri();
        let models = list_models_with_base_url("test-key", &mock_url)
            .await
            .expect("list_models should succeed");

        assert!(models.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use crate::{ChatMessage, LlmProviderDyn, LlmRequestOptions, ToolChoice, ToolDef};
    use serde_json::{Value, json};
    use wiremock::matchers::{self, body_partial_json};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn weather_tool() -> ToolDef {
        ToolDef {
            name: "get_weather".into(),
            description: "Get the current weather for a city.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"city": {"type": "string"}},
                "required": ["city"]
            }),
        }
    }

    fn function_call_response() -> Value {
        serde_json::from_str(include_str!(
            "../tests/fixtures/gemini_function_call_response.json"
        ))
        .expect("fixture is valid JSON")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sampling_params_appear_in_request_body() {
        let server = MockServer::start().await;
        // Use exact f32-representable values (0.5 = 2^-1, 0.25 = 2^-2) so that
        // f32 serialization matches the f64 literal in the partial-JSON matcher.
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(json!({
                "request": {
                    "generationConfig": {
                        "temperature": 0.5,
                        "maxOutputTokens": 256,
                        "topP": 0.25,
                        "stopSequences": ["END"]
                    }
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "candidates": [{"content": {"parts": [{"text": "ok"}]}}]
            })))
            .mount(&server)
            .await;

        let provider =
            super::GeminiProvider::new_with_base_url("k".into(), "test".into(), server.uri());
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
        use crate::LlmError;
        use std::time::Duration;

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_secs(2))
                    .set_body_json(json!({"candidates":[{"content":{"parts":[{"text":"late"}]}}]})),
            )
            .mount(&server)
            .await;

        let provider =
            super::GeminiProvider::new_with_base_url("k".into(), "test".into(), server.uri());
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
                    if provider == "gemini" && msg.to_lowercase().contains("timeout")
            ),
            "expected gemini Network/timeout, got {res:?}"
        );
        assert!(
            elapsed < Duration::from_millis(1500),
            "should abort < 1.5s, took {elapsed:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Tool calling tests
    // -----------------------------------------------------------------------

    #[tokio::test(flavor = "multi_thread")]
    async fn tool_call_in_blocking_response_returns_tool_calls() {
        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(function_call_response()))
            .mount(&server)
            .await;

        let provider =
            super::GeminiProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let opts = LlmRequestOptions {
            tools: vec![weather_tool()],
            ..Default::default()
        };
        let resp = provider
            .chat_with_options_dyn("", &[ChatMessage::user("weather?")], &opts, &Value::Null)
            .await
            .expect("should succeed");

        assert_eq!(resp.tool_calls.len(), 1, "expected one tool call");
        let tc = &resp.tool_calls[0];
        assert_eq!(tc.name, "get_weather");
        assert_eq!(tc.id, "get_weather", "id must equal name for Gemini");
        assert_eq!(tc.arguments, json!({"city": "seoul"}));
        assert_eq!(resp.rendered_text, "Checking the weather.");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tools_appear_as_function_declarations_in_request() {
        let server = MockServer::start().await;
        // Verify the wire shape: one tools[0].functionDeclarations entry.
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(json!({
                "request": {
                    "tools": [{
                        "functionDeclarations": [{
                            "name": "get_weather",
                            "description": "Get the current weather for a city."
                        }]
                    }]
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "candidates": [{"content": {"parts": [{"text": "ok"}]}}]
            })))
            .mount(&server)
            .await;

        let provider =
            super::GeminiProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let opts = LlmRequestOptions {
            tools: vec![weather_tool()],
            ..Default::default()
        };
        let result = provider
            .chat_with_options_dyn("", &[ChatMessage::user("x")], &opts, &Value::Null)
            .await;
        assert!(
            result.is_ok(),
            "request with tools should succeed: {result:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tool_choice_required_appears_as_any_mode() {
        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(json!({
                "request": {
                    "toolConfig": {
                        "functionCallingConfig": {
                            "mode": "ANY"
                        }
                    }
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "candidates": [{"content": {"parts": [{"text": "ok"}]}}]
            })))
            .mount(&server)
            .await;

        let provider =
            super::GeminiProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let opts = LlmRequestOptions {
            tools: vec![weather_tool()],
            tool_choice: Some(ToolChoice::Required),
            ..Default::default()
        };
        let result = provider
            .chat_with_options_dyn("", &[ChatMessage::user("x")], &opts, &Value::Null)
            .await;
        assert!(
            result.is_ok(),
            "tool_choice Required should map to ANY: {result:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tool_choice_specific_uses_allowed_function_names() {
        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(json!({
                "request": {
                    "toolConfig": {
                        "functionCallingConfig": {
                            "mode": "ANY",
                            "allowedFunctionNames": ["get_weather"]
                        }
                    }
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "candidates": [{"content": {"parts": [{"text": "ok"}]}}]
            })))
            .mount(&server)
            .await;

        let provider =
            super::GeminiProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let opts = LlmRequestOptions {
            tools: vec![weather_tool()],
            tool_choice: Some(ToolChoice::Specific("get_weather".into())),
            ..Default::default()
        };
        let result = provider
            .chat_with_options_dyn("", &[ChatMessage::user("x")], &opts, &Value::Null)
            .await;
        assert!(
            result.is_ok(),
            "Specific tool_choice should use allowedFunctionNames: {result:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tool_result_becomes_function_response_part() {
        let server = MockServer::start().await;
        // Check that a ToolResult block is serialized as functionResponse.
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(json!({
                "request": {
                    "contents": [
                        {
                            "role": "user",
                            "parts": [{"text": "weather?"}]
                        },
                        {
                            "role": "user",
                            "parts": [{
                                "functionResponse": {
                                    "name": "get_weather",
                                    "response": {"content": "25C sunny"}
                                }
                            }]
                        }
                    ]
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "candidates": [{"content": {"parts": [{"text": "Done."}]}}]
            })))
            .mount(&server)
            .await;

        let provider =
            super::GeminiProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let messages = vec![
            ChatMessage::user("weather?"),
            // For Gemini: tool_use_id must be the function name.
            ChatMessage::tool_result("get_weather", "25C sunny", false),
        ];
        let opts = LlmRequestOptions {
            tools: vec![weather_tool()],
            ..Default::default()
        };
        let result = provider
            .chat_with_options_dyn("", &messages, &opts, &Value::Null)
            .await;
        assert!(
            result.is_ok(),
            "tool_result should serialize as functionResponse: {result:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tool_call_in_streaming_emits_toolcall_event() {
        use crate::LlmStreamEvent;
        use futures::StreamExt;

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(function_call_response()))
            .mount(&server)
            .await;

        let provider =
            super::GeminiProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let opts = LlmRequestOptions {
            tools: vec![weather_tool()],
            ..Default::default()
        };
        let mut events: Vec<LlmStreamEvent> = provider
            .chat_stream_with_options_dyn("", &[ChatMessage::user("weather?")], &opts, &Value::Null)
            .collect()
            .await;

        // First event must be a ToolCall.
        let first = events.remove(0);
        match &first {
            LlmStreamEvent::ToolCall { call } => {
                assert_eq!(call.name, "get_weather");
                assert_eq!(call.id, "get_weather");
            }
            LlmStreamEvent::Delta { text } => {
                panic!("expected ToolCall event, got Delta({text:?})")
            }
            LlmStreamEvent::End { .. } => panic!("expected ToolCall event, got End"),
            LlmStreamEvent::Error { message } => {
                panic!("expected ToolCall event, got Error({message})")
            }
            _ => panic!("expected ToolCall event, got unknown variant"),
        }

        // A Delta and End must follow.
        assert!(
            events
                .iter()
                .any(|e| matches!(e, LlmStreamEvent::Delta { .. })),
            "stream must contain a Delta event"
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, LlmStreamEvent::End { .. })),
            "stream must contain an End event"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn no_tools_field_when_options_empty() {
        let server = MockServer::start().await;
        // Body must NOT contain "tools" key when no tools given.
        Mock::given(matchers::method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "candidates": [{"content": {"parts": [{"text": "hi"}]}}]
            })))
            .mount(&server)
            .await;

        let provider =
            super::GeminiProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let result = provider
            .chat_with_options_dyn(
                "",
                &[ChatMessage::user("hi")],
                &Default::default(),
                &Value::Null,
            )
            .await;
        assert!(result.is_ok());

        // Inspect the captured request body.
        let reqs = server.received_requests().await.unwrap();
        assert_eq!(reqs.len(), 1);
        let body: Value = serde_json::from_slice(&reqs[0].body).unwrap();
        assert!(
            body["request"]["tools"].is_null() || body["request"]["tools"] == json!([]),
            "tools field should be absent/empty when no tools provided: {body}"
        );
        assert!(
            body["request"]["toolConfig"].is_null(),
            "toolConfig field should be absent when no tool_choice provided"
        );
    }

    // -----------------------------------------------------------------------
    // Image input tests
    // -----------------------------------------------------------------------

    #[tokio::test(flavor = "multi_thread")]
    async fn image_base64_serializes_as_inline_data() {
        use crate::{ChatContent, ImageSource};

        let server = MockServer::start().await;
        // Assert that the request body contains the inlineData part alongside
        // the text part.
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(json!({
                "request": {
                    "contents": [{
                        "parts": [
                            {"text": "describe"},
                            {"inlineData": {"mimeType": "image/png", "data": "AAAA"}}
                        ]
                    }]
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "candidates": [{"content": {"parts": [{"text": "a red square"}]}}]
            })))
            .mount(&server)
            .await;

        let provider =
            super::GeminiProvider::new_with_base_url("k".into(), "test".into(), server.uri());

        let messages = vec![ChatMessage::user_multipart(vec![
            ChatContent::Text("describe".into()),
            ChatContent::Image {
                source: ImageSource::Base64("AAAA".into()),
                mime_type: "image/png".into(),
            },
        ])];

        let result = provider
            .chat_with_options_dyn("", &messages, &Default::default(), &Value::Null)
            .await;
        assert!(
            result.is_ok(),
            "base64 image should serialize as inlineData: {result:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn image_url_logs_warning_and_skips() {
        use crate::{ChatContent, ImageSource};

        let server = MockServer::start().await;
        // The request must contain the text part but NOT an inlineData part,
        // because URL-form images are skipped by the provider.
        Mock::given(matchers::method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "candidates": [{"content": {"parts": [{"text": "ok"}]}}]
            })))
            .mount(&server)
            .await;

        let provider =
            super::GeminiProvider::new_with_base_url("k".into(), "test".into(), server.uri());

        let messages = vec![ChatMessage::user_multipart(vec![
            ChatContent::Text("describe".into()),
            ChatContent::Image {
                source: ImageSource::Url("https://example.com/photo.png".into()),
                mime_type: "image/png".into(),
            },
        ])];

        let result = provider
            .chat_with_options_dyn("", &messages, &Default::default(), &Value::Null)
            .await;
        assert!(
            result.is_ok(),
            "URL image should be skipped (not an error): {result:?}"
        );

        // Verify no inlineData in the serialized request body.
        let reqs = server.received_requests().await.unwrap();
        assert_eq!(reqs.len(), 1);
        let body: Value = serde_json::from_slice(&reqs[0].body).unwrap();
        let parts = &body["request"]["contents"][0]["parts"];
        assert!(
            parts.as_array().map(|a| a.len()).unwrap_or(0) == 1,
            "only the text part should be present; inlineData must be absent: {parts}"
        );
        assert!(
            parts[0]["text"].as_str() == Some("describe"),
            "text part must be present"
        );
        assert!(
            parts[0]["inlineData"].is_null(),
            "inlineData must not appear for URL images"
        );
    }

    // -----------------------------------------------------------------------
    // Gemini 3.x thinking tests
    //
    // Verified JSON shape (https://ai.google.dev/gemini-api/docs/thinking, 2026-04-25):
    // - Response: `candidates[0].content.parts[]` with `"thought": true` on thinking parts.
    // - Request: `generationConfig.thinkingConfig.thinkingLevel` + `includeThoughts: true`.
    // -----------------------------------------------------------------------

    /// Test A: a response with a `thought: true` part populates `thinking_text`
    /// and keeps the thinking text OUT of `rendered_text`.
    #[tokio::test(flavor = "multi_thread")]
    async fn thinking_part_in_blocking_response_populates_thinking_text() {
        let server = MockServer::start().await;
        let fixture: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/gemini_thinking_response.json"
        ))
        .expect("fixture is valid JSON");

        Mock::given(matchers::method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(fixture))
            .mount(&server)
            .await;

        let provider =
            super::GeminiProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let opts = LlmRequestOptions {
            reasoning_effort: Some("high".into()),
            ..Default::default()
        };
        let resp = provider
            .chat_with_options_dyn(
                "",
                &[ChatMessage::user("what is 6*7?")],
                &opts,
                &Value::Null,
            )
            .await
            .expect("should succeed");

        // thinking_text must be populated from the `thought: true` part.
        assert!(
            resp.thinking_text.is_some(),
            "thinking_text should be populated from thought parts"
        );
        let thinking = resp.thinking_text.as_deref().unwrap();
        assert!(
            thinking.contains("analyze"),
            "thinking_text should contain the thought content, got: {thinking:?}"
        );

        // rendered_text must NOT contain the thought text.
        assert_eq!(
            resp.rendered_text, "Final answer: 42.",
            "rendered_text should only contain the non-thinking part"
        );
        assert!(
            !resp.rendered_text.contains("analyze"),
            "thinking text must not bleed into rendered_text"
        );
    }

    /// Test B: `reasoning_effort: Some("high")` emits `thinkingConfig.thinkingLevel: "high"`
    /// with `includeThoughts: true` in the request body.
    #[tokio::test(flavor = "multi_thread")]
    async fn thinking_level_appears_in_request_body() {
        let server = MockServer::start().await;

        // Assert the wire shape: generationConfig.thinkingConfig.thinkingLevel == "high"
        // and includeThoughts == true. Both are required per the Gemini 3 API docs.
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(json!({
                "request": {
                    "generationConfig": {
                        "thinkingConfig": {
                            "thinkingLevel": "high",
                            "includeThoughts": true
                        }
                    }
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "candidates": [{"content": {"parts": [{"text": "answer"}]}}]
            })))
            .mount(&server)
            .await;

        let provider =
            super::GeminiProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let opts = LlmRequestOptions {
            reasoning_effort: Some("high".into()),
            ..Default::default()
        };
        let result = provider
            .chat_with_options_dyn("", &[ChatMessage::user("x")], &opts, &Value::Null)
            .await;
        assert!(
            result.is_ok(),
            "thinking_level=high should produce a valid request: {result:?}"
        );
    }

    /// Test C (streaming): a response with thinking parts emits `Thinking` event
    /// before `Delta` in the stream.
    #[tokio::test(flavor = "multi_thread")]
    async fn thinking_in_streaming_emits_thinking_event() {
        use crate::LlmStreamEvent;
        use futures::StreamExt;

        let server = MockServer::start().await;
        let fixture: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/gemini_thinking_response.json"
        ))
        .expect("fixture is valid JSON");

        Mock::given(matchers::method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(fixture))
            .mount(&server)
            .await;

        let provider =
            super::GeminiProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let opts = LlmRequestOptions {
            reasoning_effort: Some("high".into()),
            ..Default::default()
        };
        let events: Vec<LlmStreamEvent> = provider
            .chat_stream_with_options_dyn(
                "",
                &[ChatMessage::user("what is 6*7?")],
                &opts,
                &Value::Null,
            )
            .collect()
            .await;

        // Must contain a Thinking event.
        let has_thinking = events
            .iter()
            .any(|e| matches!(e, LlmStreamEvent::Thinking { .. }));
        assert!(
            has_thinking,
            "stream must emit a Thinking event for thought parts"
        );

        // Thinking must precede Delta.
        let thinking_pos = events
            .iter()
            .position(|e| matches!(e, LlmStreamEvent::Thinking { .. }))
            .expect("Thinking event must exist");
        let delta_pos = events
            .iter()
            .position(|e| matches!(e, LlmStreamEvent::Delta { .. }))
            .expect("Delta event must exist");
        assert!(
            thinking_pos < delta_pos,
            "Thinking event ({thinking_pos}) must precede Delta ({delta_pos})"
        );

        // The thinking event text must contain the thought content.
        if let LlmStreamEvent::Thinking { text } = &events[thinking_pos] {
            assert!(
                text.contains("analyze"),
                "Thinking event text should contain thought content, got: {text:?}"
            );
        }
    }

    /// Test D: no `thinkingConfig` in request body when neither `reasoning_effort`
    /// nor `thinking_budget_tokens` is set (backward-compat guarantee).
    #[tokio::test(flavor = "multi_thread")]
    async fn no_thinking_config_when_options_not_set() {
        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "candidates": [{"content": {"parts": [{"text": "hi"}]}}]
            })))
            .mount(&server)
            .await;

        let provider =
            super::GeminiProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let result = provider
            .chat_with_options_dyn(
                "",
                &[ChatMessage::user("hi")],
                &Default::default(),
                &Value::Null,
            )
            .await;
        assert!(result.is_ok());

        // Inspect serialized body — thinkingConfig must be absent.
        let reqs = server.received_requests().await.unwrap();
        let body: Value = serde_json::from_slice(&reqs[0].body).unwrap();
        assert!(
            body["request"]["generationConfig"]["thinkingConfig"].is_null(),
            "thinkingConfig must be absent when no reasoning_effort/thinking_budget_tokens set: {body}"
        );
    }

    /// Test E: `thinking_budget_tokens: Some(_)` with no `reasoning_effort` maps to `"high"`.
    #[tokio::test(flavor = "multi_thread")]
    async fn thinking_budget_tokens_maps_to_high_thinking_level() {
        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .and(body_partial_json(json!({
                "request": {
                    "generationConfig": {
                        "thinkingConfig": {
                            "thinkingLevel": "high"
                        }
                    }
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "candidates": [{"content": {"parts": [{"text": "ok"}]}}]
            })))
            .mount(&server)
            .await;

        let provider =
            super::GeminiProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let opts = LlmRequestOptions {
            thinking_budget_tokens: Some(8192),
            ..Default::default()
        };
        let result = provider
            .chat_with_options_dyn("", &[ChatMessage::user("x")], &opts, &Value::Null)
            .await;
        assert!(
            result.is_ok(),
            "thinking_budget_tokens should map to thinking_level=high: {result:?}"
        );
    }

    /// Task 12: streaming emits a single `Usage` event before `End` when
    /// the blocking response carries `usageMetadata` (Path B — Delta+End wrap).
    ///
    /// The mock returns a single JSON response (not SSE chunks) that includes
    /// `usageMetadata`. The stream must contain events in this order:
    /// Delta → Usage → End, with token counts matching the mock payload.
    #[tokio::test(flavor = "multi_thread")]
    async fn usage_event_emitted_before_end_with_token_counts() {
        use crate::LlmStreamEvent;
        use futures::StreamExt;

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "candidates": [{"content": {"parts": [{"text": "hello"}]}}],
                "usageMetadata": {
                    "promptTokenCount": 42,
                    "candidatesTokenCount": 17,
                    "totalTokenCount": 59
                }
            })))
            .mount(&server)
            .await;

        let provider =
            super::GeminiProvider::new_with_base_url("k".into(), "test".into(), server.uri());
        let events: Vec<LlmStreamEvent> = provider
            .chat_stream_with_options_dyn(
                "",
                &[ChatMessage::user("hi")],
                &Default::default(),
                &Value::Null,
            )
            .collect()
            .await;

        // Must contain Usage event.
        let usage_pos = events
            .iter()
            .position(|e| matches!(e, LlmStreamEvent::Usage { .. }))
            .expect("stream must emit a Usage event when usageMetadata is present");

        let end_pos = events
            .iter()
            .position(|e| matches!(e, LlmStreamEvent::End { .. }))
            .expect("stream must emit End");

        // Usage must precede End.
        assert!(
            usage_pos < end_pos,
            "Usage ({usage_pos}) must precede End ({end_pos})"
        );

        // Token counts must match the mock payload.
        if let LlmStreamEvent::Usage {
            input_tokens,
            output_tokens,
            estimated_cost_usd,
        } = &events[usage_pos]
        {
            assert_eq!(
                *input_tokens,
                Some(42),
                "input_tokens should be promptTokenCount=42"
            );
            assert_eq!(
                *output_tokens, 17,
                "output_tokens should be candidatesTokenCount=17"
            );
            assert!(
                (*estimated_cost_usd - 0.0_f64).abs() < 1e-9,
                "estimated_cost_usd should be 0.0 (no Gemini pricing table)"
            );
        }

        // End total must also carry the token counts.
        if let LlmStreamEvent::End { total } = &events[end_pos] {
            assert_eq!(total.input_tokens, Some(42));
            assert_eq!(total.output_tokens, Some(17));
        }
    }
}
