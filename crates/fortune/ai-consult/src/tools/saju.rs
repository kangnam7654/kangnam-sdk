use async_trait::async_trait;
use kangnam_harness_runtime::{AgentTool, ToolCtx, ToolResult};
use serde_json::{Value, json};

use crate::persona::{build_user_saju_context, parse_birth_components};
use crate::types::ConsultCapabilities;

pub struct SajuContextTool;

#[async_trait]
impl AgentTool<ConsultCapabilities> for SajuContextTool {
    fn name(&self) -> &str {
        "consult_saju_context"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    async fn execute(&self, _params: Value, ctx: &ToolCtx<ConsultCapabilities>) -> ToolResult {
        let Some(profile) = ctx.capabilities.birth_profile.as_ref() else {
            return ToolResult::Failed {
                error: "상담 세션에 연결된 생년월일 프로필이 없습니다.".into(),
            };
        };

        let (year, month, day, hour, minute, has_birth_time) = match parse_birth_components(profile)
        {
            Ok(parts) => parts,
            Err(e) => {
                return ToolResult::Failed {
                    error: e.to_string(),
                };
            }
        };

        let pillars = saju_engine::calculate_four_pillars_precise(year, month, day, hour, minute);
        let balance = saju_engine::ElementBalance::from_pillars_with_hour(&pillars, has_birth_time);
        let context = match build_user_saju_context(profile) {
            Ok(ctx) => ctx,
            Err(e) => {
                return ToolResult::Failed {
                    error: e.to_string(),
                };
            }
        };

        ToolResult::Success {
            content: json!({
                "birth": {
                    "birth_date": profile.birth_date,
                    "birth_time": profile.birth_time,
                    "calendar_type": profile.calendar_type.as_deref().unwrap_or("solar"),
                    "has_birth_time": has_birth_time,
                },
                "day_master": {
                    "korean": context.day_master_korean,
                    "hanja": context.day_master_hanja,
                    "element": context.day_master_element,
                    "polarity": context.day_master_polarity,
                    "symbol": context.day_master_symbol,
                    "psyche_keywords": context.day_master_psyche,
                },
                "pillars": {
                    "year": format!("{}", pillars.year),
                    "month": format!("{}", pillars.month),
                    "day": format!("{}", pillars.day),
                    "hour": if has_birth_time { Some(format!("{}", pillars.hour)) } else { None },
                },
                "element_balance": {
                    "wood": balance.wood,
                    "fire": balance.fire,
                    "earth": balance.earth,
                    "metal": balance.metal,
                    "water": balance.water,
                    "dominant": balance.dominant().korean(),
                    "weakest": balance.weakest().korean(),
                }
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::BirthProfile;

    fn ctx_with_profile() -> ToolCtx<ConsultCapabilities> {
        ToolCtx::new(
            "saju-test",
            ConsultCapabilities {
                birth_profile: Some(BirthProfile {
                    birth_date: "1990-05-15".into(),
                    birth_time: Some("14:30".into()),
                    calendar_type: Some("solar".into()),
                    gender: Some("male".into()),
                }),
            },
        )
    }

    #[tokio::test]
    async fn saju_context_returns_day_master_and_balance() {
        let tool = SajuContextTool;
        let result = tool.execute(json!({}), &ctx_with_profile()).await;
        let ToolResult::Success { content } = result else {
            panic!("expected success, got {result:?}");
        };
        assert!(content["day_master"]["korean"].as_str().is_some());
        assert!(content["day_master"]["psyche_keywords"].is_array());
        assert!(content["pillars"]["day"].as_str().is_some());
        assert_eq!(
            content["element_balance"]["wood"].as_u64().unwrap()
                + content["element_balance"]["fire"].as_u64().unwrap()
                + content["element_balance"]["earth"].as_u64().unwrap()
                + content["element_balance"]["metal"].as_u64().unwrap()
                + content["element_balance"]["water"].as_u64().unwrap(),
            8
        );
    }

    #[tokio::test]
    async fn saju_context_fails_without_profile() {
        let tool = SajuContextTool;
        let ctx = ToolCtx::new(
            "saju-test",
            ConsultCapabilities {
                birth_profile: None,
            },
        );
        let result = tool.execute(json!({}), &ctx).await;
        assert!(matches!(result, ToolResult::Failed { .. }));
    }
}
