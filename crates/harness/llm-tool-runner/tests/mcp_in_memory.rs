//! Unit tests for the MCP module — `McpClient` driven by
//! `InMemoryTransport` (no subprocess, no IO). Verifies:
//!
//! - `initialize` parses both `serverInfo`-nested and flat shapes.
//! - `list_tools` round-trips tool descriptors.
//! - `call_tool` flattens text content blocks.
//! - JSON-RPC `error` envelopes map to `McpError::Server`.
//! - `LlmAgent::with_mcp_client` registers each advertised tool and
//!   the autonomous loop dispatches them correctly.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use kangnam_harness_core::{
    DefaultCapabilities, FsCallbacks, ImageCallbacks, InteractionBridge, ToolCtx, ToolError,
    WebCallbacks,
};
use kangnam_harness_llm_tool_runner::LlmAgent;
use kangnam_harness_llm_tool_runner::mcp::{ClientInfo, InMemoryTransport, McpClient, McpError};
use kangnam_harness_llm_tool_runner::test_util::{MockLlmProvider, Step};

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
        "mcp-test",
        DefaultCapabilities {
            fs: Arc::new(NopFs),
            web: Arc::new(NopWeb),
            image: Some(Arc::new(NopImage)),
            bridge: Arc::new(NopBridge),
        },
    )
}

// ── client unit tests ───────────────────────────────────────────────

use kangnam_harness_llm_tool_runner::mcp::transport::ScriptedResponse;

#[tokio::test]
async fn initialize_parses_nested_server_info_shape() {
    let transport = InMemoryTransport::new().with_response(
        "initialize",
        ScriptedResponse::Ok(json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {"name": "mock-mcp", "version": "0.1.0"},
            "capabilities": {"tools": {}}
        })),
    );
    let client = McpClient::from_in_memory(transport);
    let info = client.initialize(ClientInfo::default()).await.unwrap();
    assert_eq!(info.name, "mock-mcp");
    assert_eq!(info.version, "0.1.0");
    assert_eq!(info.protocol_version.as_deref(), Some("2024-11-05"));
}

#[tokio::test]
async fn initialize_parses_flat_server_info_shape() {
    // Some early MCP servers put name/version at top level; we accept both.
    let transport = InMemoryTransport::new().with_response(
        "initialize",
        ScriptedResponse::Ok(json!({
            "name": "legacy-mcp",
            "version": "0.0.1",
            "protocolVersion": "2024-11-05"
        })),
    );
    let client = McpClient::from_in_memory(transport);
    let info = client.initialize(ClientInfo::default()).await.unwrap();
    assert_eq!(info.name, "legacy-mcp");
}

#[tokio::test]
async fn list_tools_round_trips_descriptors() {
    let transport = InMemoryTransport::new().with_response(
        "tools/list",
        ScriptedResponse::Ok(json!({
            "tools": [
                {
                    "name": "echo",
                    "description": "Echo the input",
                    "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}}
                },
                {"name": "noop"}
            ]
        })),
    );
    let client = McpClient::from_in_memory(transport);
    let tools = client.list_tools().await.unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].name, "echo");
    assert_eq!(tools[0].description.as_deref(), Some("Echo the input"));
    assert_eq!(tools[1].name, "noop");
    // Default schema applied when omitted.
    assert_eq!(tools[1].input_schema, json!({"type": "object"}));
}

#[tokio::test]
async fn call_tool_flattens_text_blocks() {
    let transport = InMemoryTransport::new().with_response(
        "tools/call",
        ScriptedResponse::Ok(json!({
            "content": [
                {"type": "text", "text": "result: "},
                {"type": "text", "text": "42"}
            ]
        })),
    );
    let client = McpClient::from_in_memory(transport);
    let result = client
        .call_tool("multiply", json!({"a": 6, "b": 7}))
        .await
        .unwrap();
    assert!(!result.is_error);
    assert_eq!(result.flatten_text(), "result: 42");
}

#[tokio::test]
async fn call_tool_propagates_is_error_flag() {
    let transport = InMemoryTransport::new().with_response(
        "tools/call",
        ScriptedResponse::Ok(json!({
            "content": [{"type": "text", "text": "boom"}],
            "isError": true
        })),
    );
    let client = McpClient::from_in_memory(transport);
    let result = client.call_tool("explode", json!({})).await.unwrap();
    assert!(result.is_error);
    assert_eq!(result.flatten_text(), "boom");
}

#[tokio::test]
async fn server_error_envelope_maps_to_mcp_error_server() {
    let transport = InMemoryTransport::new().with_response(
        "tools/call",
        ScriptedResponse::Error {
            code: -32602,
            message: "invalid params".into(),
            data: None,
        },
    );
    let client = McpClient::from_in_memory(transport);
    let err = client.call_tool("anything", json!({})).await.unwrap_err();
    match err {
        McpError::Server { code, message, .. } => {
            assert_eq!(code, -32602);
            assert!(message.contains("invalid params"));
        }
        other => panic!("expected McpError::Server, got {other:?}"),
    }
}

#[tokio::test]
async fn list_tools_missing_field_yields_protocol_error() {
    let transport = InMemoryTransport::new()
        .with_response("tools/list", ScriptedResponse::Ok(json!({"oops": []})));
    let client = McpClient::from_in_memory(transport);
    let err = client.list_tools().await.unwrap_err();
    assert!(matches!(err, McpError::Protocol(_)));
}

// ── LlmAgent integration ─────────────────────────────────────────────

#[tokio::test]
async fn llm_agent_dispatches_through_mcp_tool() {
    // Pre-canned MCP transport with one tool 'multiply'.
    let mcp_transport = InMemoryTransport::new()
        .with_response(
            "initialize",
            ScriptedResponse::Ok(json!({
                "protocolVersion": "2024-11-05",
                "serverInfo": {"name": "mock-mcp", "version": "0.1.0"}
            })),
        )
        .with_response(
            "tools/list",
            ScriptedResponse::Ok(json!({
                "tools": [{
                    "name": "multiply",
                    "description": "Multiply two numbers.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "a": {"type": "number"},
                            "b": {"type": "number"}
                        }
                    }
                }]
            })),
        )
        .with_response(
            "tools/call",
            ScriptedResponse::Ok(json!({
                "content": [{"type": "text", "text": "42"}]
            })),
        );

    let mcp_client = McpClient::from_in_memory(mcp_transport.clone());
    mcp_client.initialize(ClientInfo::default()).await.unwrap();

    // Mock LLM scripts: first emit a tool_call, then final text.
    let mock = MockLlmProvider::new(vec![
        Step::tool_call("call_99", "multiply", json!({"a": 6, "b": 7})),
        Step::text("The answer is 42."),
    ]);
    let observer = mock.clone();

    let agent = LlmAgent::new(Box::new(mock), make_ctx())
        .with_mcp_client("mock-mcp", mcp_client)
        .await
        .unwrap();
    let run = agent.run("what is 6 times 7?").await.unwrap();

    assert_eq!(run.iterations, 2);
    assert_eq!(run.final_text, "The answer is 42.");
    assert_eq!(run.tool_invocations.len(), 1);
    let inv = &run.tool_invocations[0];
    assert_eq!(inv.call.name, "multiply");
    assert!(!inv.is_error);
    assert_eq!(inv.result, "42");

    // The MCP transport saw initialize + tools/list (during with_mcp_client)
    // + tools/call (during the agent loop).
    let observed = mcp_transport.observed();
    let methods: Vec<_> = observed.iter().map(|o| o.method.as_str()).collect();
    assert_eq!(methods, vec!["initialize", "tools/list", "tools/call"]);

    // The call was dispatched with the model's args.
    assert_eq!(observed[2].params["name"], "multiply");
    assert_eq!(observed[2].params["arguments"]["a"], 6);
    assert_eq!(observed[2].params["arguments"]["b"], 7);

    // The mock LLM was queried twice and saw one tool advertised both turns.
    let llm_obs = observer.observed();
    assert_eq!(llm_obs.len(), 2);
    assert_eq!(llm_obs[0].tool_count, 1);
}

#[tokio::test]
async fn llm_agent_with_mcp_tool_propagates_is_error() {
    let mcp_transport = InMemoryTransport::new()
        .with_response(
            "initialize",
            ScriptedResponse::Ok(json!({
                "serverInfo": {"name": "mock", "version": "0.1.0"}
            })),
        )
        .with_response(
            "tools/list",
            ScriptedResponse::Ok(json!({
                "tools": [{"name": "must_fail", "description": "Always errors"}]
            })),
        )
        .with_response(
            "tools/call",
            ScriptedResponse::Ok(json!({
                "content": [{"type": "text", "text": "tool said no"}],
                "isError": true
            })),
        );
    let mcp_client = McpClient::from_in_memory(mcp_transport);
    mcp_client.initialize(ClientInfo::default()).await.unwrap();

    let mock = MockLlmProvider::new(vec![
        Step::tool_call("c1", "must_fail", json!({})),
        Step::text("apologies"),
    ]);
    let agent = LlmAgent::new(Box::new(mock), make_ctx())
        .with_mcp_client("mock", mcp_client)
        .await
        .unwrap();
    let run = agent.run("try it").await.unwrap();
    assert!(run.tool_invocations[0].is_error);
    assert_eq!(run.tool_invocations[0].result, "tool said no");
}
