//! `gen_image` — call the host image gen API and write the result.

use async_trait::async_trait;
use kangnam_harness_runtime::{DesignTool, ToolCtx, ToolResult};
use serde_json::{json, Value};

pub struct GenImageTool;

#[async_trait]
impl DesignTool for GenImageTool {
    fn name(&self) -> &str { "gen_image" }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "required": ["prompt", "out"],
            "properties": {
                "prompt": { "type": "string", "description": "Free-text description for the image generator." },
                "out": { "type": "string", "description": "Destination path inside working_dir." }
            }
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolCtx) -> ToolResult {
        let prompt = match params.get("prompt").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return ToolResult::Failed { error: "missing `prompt`".into() },
        };
        let out_rel = match params.get("out").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return ToolResult::Failed { error: "missing `out`".into() },
        };
        let abs = ctx.working_dir.join(&out_rel);
        match ctx.image.generate(&prompt, &abs).await {
            Ok(resolved) => ToolResult::Success {
                content: json!({
                    "prompt": prompt,
                    "path": resolved.display().to_string(),
                }),
            },
            Err(e) => ToolResult::Failed { error: format!("image gen failed: {e}") },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_ctx_with_workspace;
    use serde_json::json;

    #[tokio::test]
    async fn calls_image_callback() {
        let tool = GenImageTool;
        let (ctx, ws) = test_ctx_with_workspace();
        let res = tool.execute(
            json!({"prompt":"a misty pier", "out":"hero.png"}),
            &ctx,
        ).await;
        match res {
            ToolResult::Success { content } => {
                let p = content["path"].as_str().unwrap();
                assert!(p.ends_with("hero.png"));
            }
            other => panic!("expected Success, got {other:?}"),
        }
        let _ = ws; // keep tempdir alive until end of test
    }
}
