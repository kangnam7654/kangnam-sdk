//! `preview` — request the host to render an artifact in a sandboxed
//! iframe and return screenshot + console errors.

use async_trait::async_trait;
use kangnam_harness_runtime::{AgentTool, AwaitKind, ToolCtx, ToolResult};
use serde_json::{Value, json};

pub struct PreviewTool;

#[async_trait]
impl AgentTool for PreviewTool {
    fn name(&self) -> &str {
        "preview"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string", "description": "Path inside working_dir to render." },
                "viewport": {
                    "type": "object",
                    "properties": {
                        "width":  { "type": "integer", "minimum": 200, "maximum": 4000 },
                        "height": { "type": "integer", "minimum": 200, "maximum": 4000 }
                    }
                }
            }
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolCtx) -> ToolResult {
        let path = match params.get("path").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => {
                return ToolResult::Failed {
                    error: "missing `path`".into(),
                };
            }
        };
        let abs = match ctx.resolve_path(&path) {
            Some(p) => p,
            None => {
                return ToolResult::Failed {
                    error: "preview requires a working directory".into(),
                };
            }
        };
        let payload = json!({
            "path": abs.display().to_string(),
            "viewport": params.get("viewport").cloned().unwrap_or(Value::Null),
        });
        match ctx.capabilities.bridge.register_preview(&payload).await {
            Ok((await_id, receiver)) => ToolResult::AwaitUser {
                await_id,
                kind: AwaitKind::Preview,
                payload,
                receiver,
            },
            Err(e) => ToolResult::Failed {
                error: format!("bridge error: {e}"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_ctx;
    use serde_json::json;

    #[tokio::test]
    async fn missing_path_fails() {
        let tool = PreviewTool;
        let ctx = test_ctx();
        match tool.execute(json!({}), &ctx).await {
            ToolResult::Failed { error } => assert!(error.contains("missing `path`")),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn happy_path_returns_await_preview() {
        let tool = PreviewTool;
        let ctx = test_ctx();
        match tool.execute(json!({"path":"index.html"}), &ctx).await {
            ToolResult::AwaitUser { kind, .. } => assert_eq!(kind, AwaitKind::Preview),
            other => panic!("expected AwaitUser, got {other:?}"),
        }
    }
}
