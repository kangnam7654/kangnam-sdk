use async_trait::async_trait;
use kangnam_harness_runtime::{AgentTool, ToolCtx, ToolResult};
use serde_json::{Map, Value, json};

use crate::types::ConsultCapabilities;

pub struct TarotDrawTool;

#[async_trait]
impl AgentTool<ConsultCapabilities> for TarotDrawTool {
    fn name(&self) -> &str {
        "consult_tarot_draw"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "reading_type": {
                    "type": "string",
                    "enum": ["tarot_daily", "tarot_one", "tarot_three", "tarot_celtic"]
                },
                "question": {"type": "string"},
                "options": {"type": "object"}
            },
            "required": ["reading_type"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolCtx<ConsultCapabilities>) -> ToolResult {
        let reading_type = params
            .get("reading_type")
            .and_then(Value::as_str)
            .unwrap_or("tarot_one");

        let mut input = Map::new();
        if let Some(profile) = &ctx.capabilities.birth_profile {
            input.insert("birth_date".into(), json!(profile.birth_date));
            input.insert(
                "birth_time".into(),
                json!(profile.birth_time.as_deref().unwrap_or("")),
            );
            input.insert(
                "calendar_type".into(),
                json!(profile.calendar_type.as_deref().unwrap_or("solar")),
            );
            if let Some(gender) = &profile.gender {
                input.insert("gender".into(), json!(gender));
            }
        } else {
            input.insert("birth_date".into(), json!(""));
            input.insert("birth_time".into(), json!(""));
            input.insert("calendar_type".into(), json!("solar"));
        }

        if let Some(options) = params.get("options").filter(|v| v.is_object()) {
            input.insert("options".into(), options.clone());
        }
        if let Some(question) = params.get("question").and_then(Value::as_str) {
            input.insert("question".into(), json!(question));
        }

        let engine = tarot_engine::TarotEngine;
        let (result, version) = engine.generate(reading_type, &Value::Object(input));
        if result.get("error").is_some() {
            return ToolResult::Failed {
                error: result["error"]
                    .as_str()
                    .unwrap_or("타로 생성 실패")
                    .to_string(),
            };
        }

        ToolResult::Success {
            content: json!({
                "engine_version": version,
                "reading": result,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::BirthProfile;

    fn ctx() -> ToolCtx<ConsultCapabilities> {
        ToolCtx::new(
            "tarot-test",
            ConsultCapabilities {
                birth_profile: Some(BirthProfile {
                    birth_date: "1990-05-15".into(),
                    birth_time: Some("14:30".into()),
                    calendar_type: Some("solar".into()),
                    gender: None,
                }),
            },
        )
    }

    #[tokio::test]
    async fn tarot_one_returns_one_card() {
        let tool = TarotDrawTool;
        let result = tool
            .execute(json!({"reading_type": "tarot_one"}), &ctx())
            .await;
        let ToolResult::Success { content } = result else {
            panic!("expected success, got {result:?}");
        };
        assert_eq!(
            content["reading"]["cards"].as_array().expect("cards").len(),
            1
        );
        assert_eq!(content["reading"]["spread_type"], "tarot_one");
    }

    #[tokio::test]
    async fn tarot_three_returns_three_cards() {
        let tool = TarotDrawTool;
        let result = tool
            .execute(json!({"reading_type": "tarot_three"}), &ctx())
            .await;
        let ToolResult::Success { content } = result else {
            panic!("expected success, got {result:?}");
        };
        assert_eq!(
            content["reading"]["cards"].as_array().expect("cards").len(),
            3
        );
    }

    #[tokio::test]
    async fn unsupported_spread_fails() {
        let tool = TarotDrawTool;
        let result = tool
            .execute(json!({"reading_type": "tarot_unknown"}), &ctx())
            .await;
        assert!(matches!(result, ToolResult::Failed { .. }));
    }
}
