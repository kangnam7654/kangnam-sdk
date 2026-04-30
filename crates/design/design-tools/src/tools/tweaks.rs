//! `tweaks` — declare cross-file editable controls.
//!
//! Mirrors open-codesign's tweak protocol: the agent decides which
//! parameters in already-emitted artifact files are worth a slider /
//! color picker, then writes structured `<!-- @tweak ... -->` markers
//! into those files. The frontend's TweakPanel later reads the markers
//! back and shows live controls.
//!
//! This Rust implementation is intentionally simple: the tool accepts
//! a list of `{ path, anchor, replacement }` triples and uses the
//! `FsCallbacks::str_replace` callback to splice in the tweak marker.
//! No DOM parsing — anchors are exact strings the artifact already
//! contains.

use async_trait::async_trait;
use kangnam_harness_runtime::{AgentTool, ToolCtx, ToolResult};
use serde::Deserialize;
use serde_json::{json, Value};

pub struct TweaksTool;

#[derive(Deserialize)]
struct TweakEdit {
    path: String,
    anchor: String,
    replacement: String,
}

#[async_trait]
impl AgentTool for TweaksTool {
    fn name(&self) -> &str { "tweaks" }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "required": ["edits"],
            "properties": {
                "edits": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["path", "anchor", "replacement"],
                        "properties": {
                            "path": { "type": "string" },
                            "anchor": { "type": "string", "description": "Exact string in the file to splice on." },
                            "replacement": { "type": "string", "description": "Replacement (typically anchor + a `<!-- @tweak ... -->` marker)." }
                        }
                    }
                }
            }
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolCtx) -> ToolResult {
        let edits: Vec<TweakEdit> = match params.get("edits") {
            Some(arr) => match serde_json::from_value(arr.clone()) {
                Ok(v) => v,
                Err(e) => return ToolResult::Failed { error: format!("invalid edits: {e}") },
            },
            None => return ToolResult::Failed { error: "missing `edits`".into() },
        };
        let mut applied = 0usize;
        for edit in &edits {
            let path = ctx.working_dir.join(&edit.path);
            if let Err(e) = ctx.fs.str_replace(&path, &edit.anchor, &edit.replacement).await {
                return ToolResult::Failed {
                    error: format!("str_replace failed for {}: {e}", edit.path),
                };
            }
            applied += 1;
        }
        ToolResult::Success {
            content: json!({
                "applied": applied,
                "files": edits.iter().map(|e| e.path.clone()).collect::<Vec<_>>(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{record_str_replaces, test_ctx_recording};
    use serde_json::json;

    #[tokio::test]
    async fn applies_each_edit_via_fs_callback() {
        let recorder = record_str_replaces();
        let ctx = test_ctx_recording(recorder.clone());
        let tool = TweaksTool;
        let res = tool.execute(
            json!({"edits": [
                {"path":"a.html","anchor":"<h1>","replacement":"<h1><!-- @tweak font-size:96 -->"}
            ]}),
            &ctx,
        ).await;
        match res {
            ToolResult::Success { content } => assert_eq!(content["applied"], 1),
            other => panic!("expected Success, got {other:?}"),
        }
        let log = recorder.lock().unwrap();
        assert_eq!(log.len(), 1);
        assert!(log[0].1.contains("<h1>"));
    }
}
