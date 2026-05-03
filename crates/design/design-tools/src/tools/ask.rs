//! `ask` — post a `<question-form>` schema and suspend until response.

use async_trait::async_trait;
use kangnam_design_artifact::question_form::parse_question_form;
use kangnam_harness_runtime::{AwaitKind, AgentTool, ToolCtx, ToolResult};
use serde_json::{json, Value};

pub struct AskTool;

#[async_trait]
impl AgentTool for AskTool {
    fn name(&self) -> &str { "ask" }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "required": ["form"],
            "properties": {
                "form": {
                    "type": "object",
                    "description": "QuestionForm body — the same JSON dialect emitted between <question-form>...</question-form>.",
                    "required": ["questions"],
                    "properties": {
                        "description": { "type": "string" },
                        "questions": { "type": "array", "items": { "type": "object" } }
                    }
                }
            }
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolCtx) -> ToolResult {
        let form_value = match params.get("form") {
            Some(v) => v.clone(),
            None => return ToolResult::Failed { error: "missing `form` parameter".into() },
        };
        // Validate via the artifact crate so the renderer never sees a
        // bad schema.
        let form_str = match serde_json::to_string(&form_value) {
            Ok(s) => s,
            Err(e) => return ToolResult::Failed { error: format!("form serialize: {e}") },
        };
        if let Err(e) = parse_question_form(&form_str) {
            return ToolResult::Failed { error: format!("invalid form: {e}") };
        }

        match ctx.capabilities.bridge.register_question_form(&form_value).await {
            Ok((await_id, receiver)) => ToolResult::AwaitUser {
                await_id,
                kind: AwaitKind::QuestionForm,
                payload: form_value,
                receiver,
            },
            Err(e) => ToolResult::Failed { error: format!("bridge error: {e}") },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_ctx;
    use serde_json::json;

    #[tokio::test]
    async fn rejects_invalid_form_schema() {
        let tool = AskTool;
        let ctx = test_ctx();
        let bad = json!({"form": {"questions": [{"id": "", "label": "x", "type": "text"}]}});
        match tool.execute(bad, &ctx).await {
            ToolResult::Failed { error } => assert!(error.contains("invalid form")),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn rejects_missing_form() {
        let tool = AskTool;
        let ctx = test_ctx();
        match tool.execute(json!({}), &ctx).await {
            ToolResult::Failed { error } => assert!(error.contains("missing")),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn happy_path_returns_await_user() {
        let tool = AskTool;
        let ctx = test_ctx();
        let good = json!({
            "form": {
                "questions": [
                    { "id": "x", "label": "Q?", "type": "text" }
                ]
            }
        });
        match tool.execute(good, &ctx).await {
            ToolResult::AwaitUser { kind, .. } => assert_eq!(kind, AwaitKind::QuestionForm),
            other => panic!("expected AwaitUser, got {other:?}"),
        }
    }
}
