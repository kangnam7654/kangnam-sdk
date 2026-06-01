//! `skill` — lazy-load a skill body from the catalog.

use std::sync::Arc;

use async_trait::async_trait;
use kangnam_harness_core::{AgentTool, ToolCtx, ToolResult};
use serde_json::{Value, json};

use crate::catalog::SkillCatalog;

pub struct SkillTool {
    catalog: Arc<dyn SkillCatalog>,
}

impl SkillTool {
    pub fn new(catalog: Arc<dyn SkillCatalog>) -> Self {
        Self { catalog }
    }
}

#[async_trait]
impl AgentTool for SkillTool {
    fn name(&self) -> &str {
        "skill"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "string", "description": "Skill id (catalog directory name)" }
            }
        })
    }

    async fn execute(&self, params: Value, _ctx: &ToolCtx) -> ToolResult {
        let id = match params.get("id").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return ToolResult::Failed {
                    error: "missing `id`".into(),
                };
            }
        };
        match self.catalog.lookup(id).await {
            Some(skill) => ToolResult::Success {
                content: json!({
                    "id": id,
                    "name": skill.name,
                    "description": skill.description,
                    "body": skill.body,
                }),
            },
            None => ToolResult::Failed {
                error: format!("skill `{id}` not found"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::StaticSkillCatalog;
    use crate::tests::test_ctx;
    use kangnam_design_skill::DesignSkill;
    use serde_json::json;

    fn fake_skill(name: &str) -> DesignSkill {
        DesignSkill {
            id: name.into(),
            name: name.into(),
            description: "test".into(),
            triggers: vec![],
            body: format!("# {name}\nbody"),
            od: Default::default(),
            frontmatter_extras: serde_json::Value::Null,
            root: std::path::PathBuf::new(),
        }
    }

    #[tokio::test]
    async fn returns_body_when_present() {
        let mut cat = StaticSkillCatalog::new();
        cat.insert("foo", fake_skill("foo"));
        let tool = SkillTool::new(Arc::new(cat));
        let ctx = test_ctx();
        match tool.execute(json!({"id":"foo"}), &ctx).await {
            ToolResult::Success { content } => {
                assert_eq!(content["name"], "foo");
                assert!(content["body"].as_str().unwrap().contains("# foo"));
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn errors_when_unknown() {
        let cat = Arc::new(StaticSkillCatalog::new());
        let tool = SkillTool::new(cat);
        let ctx = test_ctx();
        let res = tool.execute(json!({"id":"nope"}), &ctx).await;
        assert!(matches!(res, ToolResult::Failed { .. }));
    }
}
