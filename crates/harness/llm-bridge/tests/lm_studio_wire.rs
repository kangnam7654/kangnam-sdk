//! Layer 2 — Wire-format integration tests against a wiremock OpenAI-
//! compat HTTP server. Real `OpenAICompatProvider` from
//! `kangnam_router` drives the request bodies; we verify the bridge's
//! multi-turn loop produces a wire-compatible LM Studio conversation:
//!
//! - First call advertises `tools: [...]`
//! - Mock returns canned `tool_calls` JSON
//! - Bridge dispatches the tool, then sends a SECOND call where the
//!   assistant's prior turn includes a `tool_calls` array (Round 20
//!   wire reconstruction) AND the `tool_call_id` round-trips through
//!   the `{role: "tool", tool_call_id, content}` message.
//!
//! These tests exercise the same network path LM Studio at
//! `http://localhost:1234/v1` would receive — without needing LM Studio
//! actually running.

use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use kangnam_harness_llm_bridge::LlmAgent;
use kangnam_harness_runtime::{
    AgentTool, DefaultCapabilities, FsCallbacks, ImageCallbacks, InteractionBridge, ToolCtx,
    ToolError, ToolResult, WebCallbacks,
};
use kangnam_router::create_provider;
use wiremock::matchers::{body_partial_json, body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ── stub capabilities (same as layer 1) ─────────────────────────────

struct NopFs;
#[async_trait]
impl FsCallbacks for NopFs {
    async fn read(&self, _: &Path) -> Result<Vec<u8>, ToolError> {
        Ok(vec![])
    }
    async fn write(&self, _: &Path, _: &[u8]) -> Result<(), ToolError> {
        Ok(())
    }
    async fn str_replace(&self, _: &Path, _: &str, _: &str) -> Result<(), ToolError> {
        Ok(())
    }
}
struct NopWeb;
#[async_trait]
impl WebCallbacks for NopWeb {
    async fn fetch(&self, _: &str) -> Result<Vec<u8>, ToolError> {
        Ok(vec![])
    }
}
struct NopImage;
#[async_trait]
impl ImageCallbacks for NopImage {
    async fn generate(&self, _: &str, p: &Path) -> Result<std::path::PathBuf, ToolError> {
        Ok(p.to_path_buf())
    }
}
struct NopBridge;
#[async_trait]
impl InteractionBridge for NopBridge {
    async fn register_question_form(
        &self,
        _: &Value,
    ) -> Result<(String, oneshot::Receiver<Value>), ToolError> {
        let (_tx, rx) = oneshot::channel();
        Ok(("await-x".into(), rx))
    }
    async fn register_preview(
        &self,
        _: &Value,
    ) -> Result<(String, oneshot::Receiver<Value>), ToolError> {
        let (_tx, rx) = oneshot::channel();
        Ok(("await-y".into(), rx))
    }
}

fn make_ctx() -> ToolCtx {
    ToolCtx::new(
        "wire-test",
        DefaultCapabilities {
            fs: Arc::new(NopFs),
            web: Arc::new(NopWeb),
            image: Some(Arc::new(NopImage)),
            bridge: Arc::new(NopBridge),
        },
    )
}

struct GetWeather {
    log: Arc<Mutex<Vec<Value>>>,
}
#[async_trait]
impl AgentTool for GetWeather {
    fn name(&self) -> &str {
        "get_weather"
    }
    fn parameters(&self) -> Value {
        json!({"type": "object", "properties": {"city": {"type": "string"}}, "required": ["city"]})
    }
    async fn execute(&self, params: Value, _: &ToolCtx) -> ToolResult {
        self.log.lock().unwrap().push(params.clone());
        ToolResult::Success {
            content: json!({"temp_c": 25, "city": params["city"]}),
        }
    }
}

// ── tests ────────────────────────────────────────────────────────────

/// End-to-end: bridge → OpenAICompatProvider → mock LM Studio server.
///
/// Server scripted to:
/// 1. First request → return `tool_calls: [{id: call_01, name: get_weather, args: {city: "seoul"}}]`
/// 2. Second request (must include reconstructed `tool_calls` on the
///    prior assistant turn) → return final text "It is 25C."
#[tokio::test(flavor = "multi_thread")]
async fn bridge_round_trips_tool_call_through_lm_studio_wire() {
    let server = MockServer::start().await;

    // Second mock: HIGHER priority (lower number wins in wiremock).
    // Matches when the wire body contains a `tool_call_id` — this
    // token only appears on the second iteration after the bridge
    // emits the `{role: "tool", tool_call_id}` follow-up. The match
    // proves Round 20's reconstruction reached the wire intact.
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(body_string_contains("tool_call_id"))
        .and(body_string_contains("call_01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "qwen2.5-7b-instruct",
            "choices": [{
                "message": {"role": "assistant", "content": "It is 25C in Seoul."},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 30, "completion_tokens": 8}
        })))
        .with_priority(1)
        .expect(1)
        .mount(&server)
        .await;

    // First mock: lower priority — fires for any other request,
    // including the initial one. Returns the canned tool_call.
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "qwen2.5-7b-instruct",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Checking the weather.",
                    "tool_calls": [{
                        "id": "call_01",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"city\":\"seoul\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5}
        })))
        .with_priority(5)
        .expect(1)
        .mount(&server)
        .await;

    // Build a real OpenAICompatProvider pointing at the mock server.
    let provider =
        create_provider("openai_compat", "", "qwen2.5-7b-instruct", &server.uri()).unwrap();

    let log = Arc::new(Mutex::new(Vec::new()));
    let agent = LlmAgent::new(provider, make_ctx())
        .with_tool(
            GetWeather {
                log: Arc::clone(&log),
            },
            "Get current weather for a city",
        )
        .with_system_prompt("Use tools when asked.");

    let run = agent.run("weather in seoul?").await.unwrap();

    assert_eq!(run.iterations, 2);
    assert_eq!(run.final_text, "It is 25C in Seoul.");
    assert_eq!(run.tool_invocations.len(), 1);
    assert_eq!(run.tool_invocations[0].call.id, "call_01");

    // The tool actually got the parsed args from the wire.
    let logged = log.lock().unwrap();
    assert_eq!(logged.len(), 1);
    assert_eq!(logged[0]["city"], "seoul");
    // Both expected mocks fired exactly once — wiremock asserts on drop.
}

/// Verify that when a tool fails the wire response carries
/// `Error: <message>` per the openai_compat error convention, and the
/// model receives it in the next turn so it can apologise / retry.
#[tokio::test(flavor = "multi_thread")]
async fn failed_tool_carries_error_prefix_on_wire() {
    let server = MockServer::start().await;

    // High-priority second-call mock: matches when the body contains
    // the `tool_call_id` and the "Error: intentional" content the
    // bridge writes back when a tool returns ToolResult::Failed.
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(body_partial_json(json!({
            "messages": [
                {"role": "user", "content": "fail it"},
                {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call_99",
                        "function": {"name": "always_fail"}
                    }]
                },
                {"role": "tool", "tool_call_id": "call_99", "content": "Error: intentional"}
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "m",
            "choices": [{"message": {"role": "assistant", "content": "sorry, that failed."}}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5}
        })))
        .with_priority(1)
        .expect(1)
        .mount(&server)
        .await;

    // Lower-priority first-call fallback: returns the failing tool_call.
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "m",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_99",
                        "type": "function",
                        "function": {"name": "always_fail", "arguments": "{}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {"prompt_tokens": 5, "completion_tokens": 1}
        })))
        .with_priority(5)
        .expect(1)
        .mount(&server)
        .await;

    struct AlwaysFail;
    #[async_trait]
    impl AgentTool for AlwaysFail {
        fn name(&self) -> &str {
            "always_fail"
        }
        fn parameters(&self) -> Value {
            json!({"type": "object"})
        }
        async fn execute(&self, _: Value, _: &ToolCtx) -> ToolResult {
            ToolResult::Failed {
                error: "intentional".into(),
            }
        }
    }

    let provider = create_provider("openai_compat", "", "m", &server.uri()).unwrap();
    let agent = LlmAgent::new(provider, make_ctx()).with_tool(AlwaysFail, "Always fails");
    let run = agent.run("fail it").await.unwrap();

    assert_eq!(run.final_text, "sorry, that failed.");
    assert!(run.tool_invocations[0].is_error);
}
