//! `/api/app-config` — onboarding state, default agent / skill /
//! design-system selections, opt-out lists. Mirrors
//! `@open-design/contracts/src/api/app-config.ts`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Per-agent model + reasoning preferences. Both fields optional so
/// "no preference, use the daemon default" is the default state.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentModelPrefs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
}

/// User-level preferences stored on the daemon. Every field optional —
/// shape is sparsely populated as the user makes choices.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppConfigPrefs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onboarding_completed: Option<bool>,
    /// Default agent. `Some(None)` (deserialized from `null`) means
    /// "explicitly cleared"; `None` means "never set"; `Some(Some(_))`
    /// means "set to this value". See [`crate::serde_helpers::double_option`]
    /// for the wire-state mapping.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub agent_id: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_models: Option<HashMap<String, AgentModelPrefs>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub skill_id: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub design_system_id: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_skills: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_design_systems: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfigResponse {
    pub config: AppConfigPrefs,
}

/// `Partial<AppConfigPrefs>` upstream — same shape since every field is
/// already optional. Defined as an alias so call sites read symmetrically.
pub type UpdateAppConfigRequest = AppConfigPrefs;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_prefs_serialize_to_empty_object() {
        let p = AppConfigPrefs::default();
        let s = serde_json::to_string(&p).unwrap();
        assert_eq!(s, "{}");
    }

    #[test]
    fn camel_case_field_names() {
        let p = AppConfigPrefs {
            onboarding_completed: Some(true),
            agent_id: Some(Some("a1".into())),
            ..Default::default()
        };
        let s = serde_json::to_string(&p).unwrap();
        assert!(s.contains("\"onboardingCompleted\":true"));
        assert!(s.contains("\"agentId\":\"a1\""));
    }

    #[test]
    fn null_agent_id_round_trips() {
        let json = r#"{"agentId":null}"#;
        let p: AppConfigPrefs = serde_json::from_str(json).unwrap();
        assert_eq!(p.agent_id, Some(None));
        let back = serde_json::to_string(&p).unwrap();
        assert_eq!(back, json);
    }

    #[test]
    fn missing_agent_id_yields_none_outer() {
        let json = r#"{}"#;
        let p: AppConfigPrefs = serde_json::from_str(json).unwrap();
        assert_eq!(p.agent_id, None);
    }

    #[test]
    fn agent_models_map_round_trip() {
        let mut models = HashMap::new();
        models.insert(
            "claude".to_string(),
            AgentModelPrefs {
                model: Some("claude-opus-4-7".into()),
                reasoning: None,
            },
        );
        let p = AppConfigPrefs {
            agent_models: Some(models),
            ..Default::default()
        };
        let s = serde_json::to_string(&p).unwrap();
        assert!(s.contains("\"agentModels\""));
        assert!(s.contains("\"claude\""));
        assert!(s.contains("\"model\":\"claude-opus-4-7\""));
        let back: AppConfigPrefs = serde_json::from_str(&s).unwrap();
        assert_eq!(
            back.agent_models.as_ref().unwrap().get("claude").unwrap().model.as_deref(),
            Some("claude-opus-4-7")
        );
    }

    #[test]
    fn disabled_lists_round_trip() {
        let p = AppConfigPrefs {
            disabled_skills: Some(vec!["pricing-page".into()]),
            disabled_design_systems: Some(vec!["claude".into(), "linear-app".into()]),
            ..Default::default()
        };
        let s = serde_json::to_string(&p).unwrap();
        assert!(s.contains("\"disabledSkills\":[\"pricing-page\"]"));
        assert!(s.contains("\"disabledDesignSystems\":[\"claude\",\"linear-app\"]"));
    }

    #[test]
    fn response_envelope_round_trip() {
        let r = AppConfigResponse {
            config: AppConfigPrefs {
                onboarding_completed: Some(true),
                ..Default::default()
            },
        };
        let s = serde_json::to_string(&r).unwrap();
        assert_eq!(s, r#"{"config":{"onboardingCompleted":true}}"#);
    }
}
