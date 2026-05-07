//! Spawned-process stdio MCP integration test.
//!
//! Uses `tests/mcp_fixtures/echo_server.py` as a tiny canned MCP
//! server. Verifies the full happy path:
//!
//! - Process spawns + handshake completes (`initialize`).
//! - Tool listing returns the advertised `echo` tool.
//! - `tools/call` round-trips through stdin/stdout.
//! - Errors from the server arrive as `McpError::Server`.
//!
//! Skips early if `python3` is not on `PATH` (CI environments without
//! Python still run the rest of the suite).

use std::path::PathBuf;
use std::process::Command;

use serde_json::json;

use kangnam_harness_llm_bridge::mcp::{ClientInfo, McpClient, McpError};

fn python3_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("mcp_fixtures")
        .join("echo_server.py")
}

#[tokio::test(flavor = "multi_thread")]
async fn stdio_handshake_list_call_round_trip() {
    if !python3_available() {
        eprintln!("skipping: python3 not found on PATH");
        return;
    }
    let script = fixture_path();
    let script_str = script.to_string_lossy();

    let client = McpClient::new_stdio("python3", &[&script_str], ClientInfo::default())
        .await
        .expect("spawn + initialize should succeed");

    let server_info = client.server_info().expect("server_info captured");
    assert_eq!(server_info.name, "echo-fixture");
    assert_eq!(server_info.version, "0.1.0");

    let tools = client.list_tools().await.expect("list_tools");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "echo");
    assert_eq!(
        tools[0].description.as_deref(),
        Some("Echo the input text verbatim.")
    );

    let result = client
        .call_tool("echo", json!({"text": "hello mcp"}))
        .await
        .expect("call_tool");
    assert!(!result.is_error);
    assert_eq!(result.flatten_text(), "hello mcp");
}

#[tokio::test(flavor = "multi_thread")]
async fn stdio_unknown_tool_returns_server_error() {
    if !python3_available() {
        eprintln!("skipping: python3 not found on PATH");
        return;
    }
    let script = fixture_path();
    let script_str = script.to_string_lossy();

    let client = McpClient::new_stdio("python3", &[&script_str], ClientInfo::default())
        .await
        .unwrap();

    let err = client.call_tool("nonexistent", json!({})).await.unwrap_err();
    match err {
        McpError::Server { code, message, .. } => {
            assert_eq!(code, -32602);
            assert!(message.contains("nonexistent"));
        }
        other => panic!("expected McpError::Server, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn stdio_concurrent_requests_do_not_interleave() {
    // Issue 8 calls in parallel and verify each result matches its input
    // text. If line writes interleaved, the JSON would corrupt and the
    // server would silently drop frames (none would round-trip).
    if !python3_available() {
        eprintln!("skipping: python3 not found on PATH");
        return;
    }
    let script = fixture_path();
    let script_str = script.to_string_lossy();
    let client = McpClient::new_stdio("python3", &[&script_str], ClientInfo::default())
        .await
        .unwrap();

    let mut joinset = tokio::task::JoinSet::new();
    for i in 0..8 {
        let c = client.clone();
        joinset.spawn(async move {
            let r = c
                .call_tool("echo", json!({"text": format!("turn {i}")}))
                .await
                .unwrap();
            (i, r.flatten_text())
        });
    }

    let mut got = std::collections::HashMap::new();
    while let Some(j) = joinset.join_next().await {
        let (i, text) = j.unwrap();
        got.insert(i, text);
    }
    assert_eq!(got.len(), 8);
    for i in 0..8 {
        assert_eq!(got.get(&i).map(|s| s.as_str()), Some(format!("turn {i}").as_str()));
    }
}
