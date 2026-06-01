//! `scaffold` — copy a skill's curated `assets/` into the workspace.
//!
//! Mirrors open-codesign's `scaffold(kind, path)` tool. The skill id
//! identifies the catalog entry; the catalog supplies the absolute
//! `assets/` path; this tool walks the dir, reads each file via the
//! ToolCtx fs callbacks, and writes copies under
//! `working_dir/<dest>/`. Pure pull — no host fs assumptions beyond
//! what `FsCallbacks` exposes.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use kangnam_harness_core::{AgentTool, ToolCtx, ToolResult};
use serde_json::{Value, json};

use crate::catalog::SkillCatalog;

pub struct ScaffoldTool {
    catalog: Arc<dyn SkillCatalog>,
}

impl ScaffoldTool {
    pub fn new(catalog: Arc<dyn SkillCatalog>) -> Self {
        Self { catalog }
    }
}

#[async_trait]
impl AgentTool for ScaffoldTool {
    fn name(&self) -> &str {
        "scaffold"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "required": ["skill_id", "dest"],
            "properties": {
                "skill_id": { "type": "string", "description": "Catalog id of the skill to scaffold from" },
                "dest": { "type": "string", "description": "Destination subdirectory inside working_dir" }
            }
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolCtx) -> ToolResult {
        let skill_id = match params.get("skill_id").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return ToolResult::Failed {
                    error: "missing `skill_id`".into(),
                };
            }
        };
        let dest_rel = match params.get("dest").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return ToolResult::Failed {
                    error: "missing `dest`".into(),
                };
            }
        };
        let assets = match self.catalog.assets_dir(skill_id).await {
            Some(p) => p,
            None => {
                return ToolResult::Failed {
                    error: format!("skill `{skill_id}` has no assets directory in catalog"),
                };
            }
        };

        let dest_root = match ctx.resolve_path(dest_rel) {
            Some(p) => p,
            None => {
                return ToolResult::Failed {
                    error: "scaffold requires a working directory".into(),
                };
            }
        };
        let mut copied: Vec<String> = Vec::new();
        if let Err(e) = walk_and_copy(&assets, &assets, &dest_root, ctx, &mut copied).await {
            return ToolResult::Failed {
                error: format!("scaffold failed: {e}"),
            };
        }

        ToolResult::Success {
            content: json!({
                "skill_id": skill_id,
                "dest": dest_root.display().to_string(),
                "files": copied,
            }),
        }
    }
}

async fn walk_and_copy(
    root: &std::path::Path,
    cur: &std::path::Path,
    dest_root: &std::path::Path,
    ctx: &ToolCtx,
    copied: &mut Vec<String>,
) -> std::io::Result<()> {
    let mut stack: Vec<PathBuf> = vec![cur.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut rd = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = rd.next_entry().await? {
            let path = entry.path();
            let file_type = entry.file_type().await?;
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            let dest = dest_root.join(rel);
            let bytes = tokio::fs::read(&path).await?;
            // Write through the ToolCtx so the host's FsCallbacks can
            // sandbox / log uniformly.
            ctx.capabilities
                .fs
                .write(&dest, &bytes)
                .await
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            copied.push(rel.display().to_string());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::StaticSkillCatalog;
    use crate::tests::test_ctx_with_workspace;
    use serde_json::json;

    #[tokio::test]
    async fn errors_when_skill_unknown() {
        let cat = Arc::new(StaticSkillCatalog::new());
        let tool = ScaffoldTool::new(cat);
        let (ctx, _g) = test_ctx_with_workspace();
        let res = tool
            .execute(json!({"skill_id":"nope","dest":"x"}), &ctx)
            .await;
        match res {
            ToolResult::Failed { error } => assert!(error.contains("no assets")),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn copies_assets_dir_into_workspace() {
        // Set up source assets in a tempdir.
        let src = tempfile::tempdir().unwrap();
        tokio::fs::create_dir_all(src.path().join("nested"))
            .await
            .unwrap();
        tokio::fs::write(src.path().join("a.html"), b"<p>a</p>")
            .await
            .unwrap();
        tokio::fs::write(src.path().join("nested/b.css"), b"body{}")
            .await
            .unwrap();

        let mut cat = StaticSkillCatalog::new();
        cat.with_assets("web-prototype", src.path().to_path_buf());
        let tool = ScaffoldTool::new(Arc::new(cat));

        let (ctx, ws) = test_ctx_with_workspace();
        let res = tool
            .execute(json!({"skill_id":"web-prototype","dest":"out"}), &ctx)
            .await;
        match res {
            ToolResult::Success { content } => {
                let files = content["files"].as_array().unwrap();
                assert_eq!(files.len(), 2);
            }
            other => panic!("expected Success, got {other:?}"),
        }
        assert!(ws.path().join("out/a.html").exists());
        assert!(ws.path().join("out/nested/b.css").exists());
    }
}
