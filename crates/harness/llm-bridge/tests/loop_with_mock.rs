//! Layer 1 — Unit-level loop verification with the in-memory
//! `MockLlmProvider`. No HTTP, no real model. Validates that the
//! bridge:
//!
//! - Stops when the model returns no `tool_calls` (terminal text).
//! - Dispatches each tool call to the matching `AgentTool::execute`.
//! - Records assistant tool-call turns in history (so the next request
//!   re-emits them on the wire — verified at the integration layer).
//! - Errors with `UnknownTool` when the model invokes a missing tool.
//! - Errors with `MaxIterations` when the model loops forever.
//! - Captures parallel tool calls in dispatch order.

use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use kangnam_harness_llm_bridge::test_util::{MockLlmProvider, Step};
use kangnam_harness_llm_bridge::{BridgeError, LlmAgent};
use kangnam_harness_runtime::{
    AgentTool, DefaultCapabilities, FsCallbacks, ImageCallbacks, InteractionBridge, ToolCtx,
    ToolError, ToolResult, WebCallbacks,
};
use kangnam_router::{ChatContent, ToolCall};

// ── stub capabilities ───────────────────────────────────────────────

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
        "test-session",
        DefaultCapabilities {
            fs: Arc::new(NopFs),
            web: Arc::new(NopWeb),
            image: Some(Arc::new(NopImage)),
            bridge: Arc::new(NopBridge),
        },
    )
}

// ── tools ────────────────────────────────────────────────────────────

/// Records every invocation in a shared log so tests can assert on
/// dispatch order and arguments.
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
        let city = params["city"].as_str().unwrap_or("?");
        ToolResult::Success {
            content: json!({"city": city, "temp_c": 25}),
        }
    }
}

/// Always fails — used to verify that `is_error: true` propagates back
/// to the model on the wire.
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

// ── tests ────────────────────────────────────────────────────────────

#[tokio::test]
async fn terminal_text_response_returns_immediately() {
    let mock = MockLlmProvider::new(vec![Step::text("done.")]);
    let agent = LlmAgent::new(Box::new(mock), make_ctx());
    let run = agent.run("hi").await.unwrap();

    assert_eq!(run.iterations, 1);
    assert_eq!(run.final_text, "done.");
    assert!(run.tool_invocations.is_empty());
    // History: user input + assistant terminal text.
    assert_eq!(run.messages.len(), 2);
}

#[tokio::test]
async fn tool_call_dispatches_and_continues_loop() {
    let mock = MockLlmProvider::new(vec![
        Step::tool_call("call_01", "get_weather", json!({"city": "seoul"})),
        Step::text("It is 25C in Seoul."),
    ]);
    let observer = mock.clone(); // share state for post-run inspection

    let log = Arc::new(Mutex::new(Vec::new()));
    let agent = LlmAgent::new(Box::new(mock), make_ctx()).with_tool(
        GetWeather {
            log: Arc::clone(&log),
        },
        "Get current weather",
    );

    let run = agent.run("weather in seoul?").await.unwrap();

    assert_eq!(run.iterations, 2);
    assert_eq!(run.final_text, "It is 25C in Seoul.");
    assert_eq!(run.tool_invocations.len(), 1);
    assert_eq!(run.tool_invocations[0].call.name, "get_weather");
    assert_eq!(run.tool_invocations[0].call.id, "call_01");
    assert!(!run.tool_invocations[0].is_error);

    let logged = log.lock().unwrap();
    assert_eq!(logged.len(), 1);
    assert_eq!(logged[0]["city"], "seoul");

    // History shape: user → assistant(tool_call) → user(tool_result) → assistant(final).
    assert_eq!(run.messages.len(), 4);
    let assistant_with_calls = &run.messages[1];
    assert_eq!(assistant_with_calls.role, "assistant");
    let has_tool_use = assistant_with_calls
        .content
        .iter()
        .any(|c| matches!(c, ChatContent::ToolUse { .. }));
    assert!(
        has_tool_use,
        "assistant turn must record ToolUse content for wire reconstruction"
    );

    let tool_result_msg = &run.messages[2];
    assert_eq!(tool_result_msg.role, "user");
    let has_tool_result = tool_result_msg
        .content
        .iter()
        .any(|c| matches!(c, ChatContent::ToolResult { .. }));
    assert!(has_tool_result);

    // The bridge sent two distinct requests; both advertised the tool.
    let observed = observer.observed();
    assert_eq!(observed.len(), 2);
    assert_eq!(observed[0].tool_count, 1);
    assert_eq!(observed[1].tool_count, 1);
}

#[tokio::test]
async fn parallel_tool_calls_dispatch_in_order() {
    let calls = vec![
        ToolCall {
            id: "a".into(),
            name: "get_weather".into(),
            arguments: json!({"city": "seoul"}),
        },
        ToolCall {
            id: "b".into(),
            name: "get_weather".into(),
            arguments: json!({"city": "tokyo"}),
        },
    ];
    let mock = MockLlmProvider::new(vec![Step::tool_calls(calls), Step::text("done")]);

    let log = Arc::new(Mutex::new(Vec::new()));
    let agent = LlmAgent::new(Box::new(mock), make_ctx()).with_tool(
        GetWeather {
            log: Arc::clone(&log),
        },
        "Get current weather",
    );

    let run = agent.run("weather everywhere").await.unwrap();
    assert_eq!(run.tool_invocations.len(), 2);
    assert_eq!(run.tool_invocations[0].call.id, "a");
    assert_eq!(run.tool_invocations[1].call.id, "b");

    let logged = log.lock().unwrap();
    assert_eq!(logged.len(), 2);
    assert_eq!(logged[0]["city"], "seoul");
    assert_eq!(logged[1]["city"], "tokyo");
}

#[tokio::test]
async fn unknown_tool_errors_with_registered_list() {
    let mock = MockLlmProvider::new(vec![Step::tool_call("x", "nonexistent", json!({}))]);
    let log = Arc::new(Mutex::new(Vec::new()));
    let agent = LlmAgent::new(Box::new(mock), make_ctx()).with_tool(
        GetWeather {
            log: Arc::clone(&log),
        },
        "Weather",
    );

    let err = agent.run("???").await.unwrap_err();
    match err {
        BridgeError::UnknownTool { name, registered } => {
            assert_eq!(name, "nonexistent");
            assert_eq!(registered, vec!["get_weather"]);
        }
        other => panic!("expected UnknownTool, got {other:?}"),
    }
}

#[tokio::test]
async fn failed_tool_propagates_is_error_to_history() {
    let mock = MockLlmProvider::new(vec![
        Step::tool_call("c1", "always_fail", json!({})),
        Step::text("ack"),
    ]);
    let agent = LlmAgent::new(Box::new(mock), make_ctx()).with_tool(AlwaysFail, "Always fails");

    let run = agent.run("try it").await.unwrap();
    assert_eq!(run.tool_invocations.len(), 1);
    assert!(run.tool_invocations[0].is_error);
    assert_eq!(run.tool_invocations[0].result, "intentional");

    let tool_result = run
        .messages
        .iter()
        .flat_map(|m| m.content.iter())
        .find_map(|c| match c {
            ChatContent::ToolResult {
                is_error, content, ..
            } => Some((*is_error, content.clone())),
            _ => None,
        })
        .expect("expected one ToolResult content block");
    assert!(tool_result.0);
    assert_eq!(tool_result.1, "intentional");
}

#[tokio::test]
async fn max_iterations_caps_runaway_loop() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mock = MockLlmProvider::new(vec![
        Step::tool_call("a", "get_weather", json!({"city": "1"})),
        Step::tool_call("b", "get_weather", json!({"city": "2"})),
        Step::tool_call("c", "get_weather", json!({"city": "3"})),
        Step::tool_call("d", "get_weather", json!({"city": "4"})),
        Step::tool_call("e", "get_weather", json!({"city": "5"})),
    ]);
    let agent = LlmAgent::new(Box::new(mock), make_ctx())
        .with_tool(GetWeather { log }, "Weather")
        .with_max_iterations(3);

    let err = agent.run("loop").await.unwrap_err();
    assert!(matches!(err, BridgeError::MaxIterations { max: 3 }));
}

#[tokio::test]
async fn second_request_history_reemits_assistant_tool_calls() {
    // Round 20 wire-correctness gate: the bridge must record the
    // assistant turn with `ChatContent::ToolUse` so the second request
    // serialises a proper `tool_calls` array. We inspect what the mock
    // observed on its second invocation.
    let mock = MockLlmProvider::new(vec![
        Step::tool_call("call_42", "get_weather", json!({"city": "seoul"})),
        Step::text("done"),
    ]);
    let observer = mock.clone();

    let log = Arc::new(Mutex::new(Vec::new()));
    let agent = LlmAgent::new(Box::new(mock), make_ctx()).with_tool(
        GetWeather {
            log: Arc::clone(&log),
        },
        "Weather",
    );
    let _ = agent.run("?").await.unwrap();

    let observed = observer.observed();
    assert_eq!(observed.len(), 2, "bridge should issue two requests");

    let second = &observed[1];
    let assistant_msg = second
        .messages
        .iter()
        .find(|m| m.role == "assistant")
        .expect("second request must include the prior assistant turn");
    let mut found_tool_use = false;
    for c in &assistant_msg.content {
        if let ChatContent::ToolUse { id, name, .. } = c {
            assert_eq!(id, "call_42");
            assert_eq!(name, "get_weather");
            found_tool_use = true;
        }
    }
    assert!(
        found_tool_use,
        "second request's assistant turn must carry the ToolUse block"
    );
}
