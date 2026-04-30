//! `done` — turn-end signal. The harness uses the marker to stop
//! looping back into the model.

use async_trait::async_trait;
use kangnam_harness_runtime::{DesignTool, ToolCtx, ToolResult};
use serde_json::{json, Value};

pub struct DoneTool;

#[async_trait]
impl DesignTool for DoneTool {
    fn name(&self) -> &str { "done" }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "summary": { "type": "string", "description": "One-sentence summary of what was produced." },
                "path": { "type": "string", "description": "Path of the final artifact, relative to working_dir." }
            }
        })
    }

    async fn execute(&self, params: Value, _ctx: &ToolCtx) -> ToolResult {
        ToolResult::Success {
            content: json!({
                "done": true,
                "summary": params.get("summary").cloned().unwrap_or(Value::Null),
                "path": params.get("path").cloned().unwrap_or(Value::Null),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_ctx;

    #[tokio::test]
    async fn returns_done_marker() {
        let tool = DoneTool;
        let ctx = test_ctx();
        match tool.execute(serde_json::json!({"summary":"shipped"}), &ctx).await {
            ToolResult::Success { content } => {
                assert_eq!(content["done"], true);
                assert_eq!(content["summary"], "shipped");
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }
}
