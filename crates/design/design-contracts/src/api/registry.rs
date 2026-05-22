//! `/api/registry` — agents, skills, design systems, codex pets. Mirrors
//! `@open-design/contracts/src/api/registry.ts`.
//!
//! These are the catalog projections the web client uses for picker UIs
//! and detail views — not the source-of-truth catalog (which lives in
//! `kangnam-design-skill::DesignSkill`, `kangnam-design-system::DesignSystem`,
//! …). The daemon converts internal records into these summary shapes
//! before serving.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentModelOption {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub bin: String,
    pub available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub version: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<AgentModelOption>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_options: Option<Vec<AgentModelOption>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentsResponse {
    pub agents: Vec<AgentInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum SkillMode {
    Prototype,
    Deck,
    Template,
    DesignSystem,
    Image,
    Video,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum SkillSurface {
    Web,
    Image,
    Video,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum SkillPlatform {
    Desktop,
    Mobile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum SkillFidelity {
    Wireframe,
    HighFidelity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub triggers: Vec<String>,
    pub mode: SkillMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<SkillSurface>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub platform: Option<Option<SkillPlatform>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub scenario: Option<Option<String>>,
    pub preview_type: String,
    pub design_system_required: bool,
    pub default_for: Vec<String>,
    pub upstream: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub featured: Option<Option<u32>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub fidelity: Option<Option<SkillFidelity>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub speaker_notes: Option<Option<bool>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub animations: Option<Option<bool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub craft_requires: Option<Vec<String>>,
    pub has_body: bool,
    pub example_prompt: String,
}

/// `extends SkillSummary` upstream — adds the SKILL.md body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillDetail {
    #[serde(flatten)]
    pub summary: SkillSummary,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillsResponse {
    pub skills: Vec<SkillSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillResponse {
    pub skill: SkillDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DesignSystemSummary {
    pub id: String,
    pub title: String,
    pub category: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swatches: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<SkillSurface>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DesignSystemDetail {
    #[serde(flatten)]
    pub summary: DesignSystemSummary,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DesignSystemsResponse {
    pub design_systems: Vec<DesignSystemSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DesignSystemResponse {
    pub design_system: DesignSystemDetail,
}

/// `'daemon'` literal upstream — locked single-variant enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum HealthService {
    Daemon,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthResponse {
    /// Locked to `true` via the [`crate::api::connectors::ConnectorExecuteOk`]
    /// pattern — but inlined here so this module stays freestanding.
    pub ok: HealthOk,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<HealthService>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

crate::locked_true!(
    /// `ok: true` literal on `/api/health` — see [`crate::locked_true`].
    pub struct HealthOk;,
    field_name = "ok"
);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexPetSummary {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub spritesheet_url: String,
    pub spritesheet_ext: String,
    pub hatched_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexPetsResponse {
    pub pets: Vec<CodexPetSummary>,
    pub root_dir: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum SyncCommunityPetsSource {
    All,
    Petshare,
    Hatchery,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncCommunityPetsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SyncCommunityPetsSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncCommunityPetsResponse {
    pub wrote: u32,
    pub skipped: u32,
    pub failed: u32,
    pub total: u32,
    pub root_dir: String,
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_mode_kebab_case() {
        assert_eq!(
            serde_json::to_string(&SkillMode::DesignSystem).unwrap(),
            "\"design-system\""
        );
        let m: SkillMode = serde_json::from_str("\"prototype\"").unwrap();
        assert_eq!(m, SkillMode::Prototype);
    }

    #[test]
    fn skill_fidelity_kebab_case() {
        assert_eq!(
            serde_json::to_string(&SkillFidelity::HighFidelity).unwrap(),
            "\"high-fidelity\""
        );
    }

    #[test]
    fn agent_info_double_option_version() {
        let json = r#"{"id":"a","name":"n","bin":"b","available":true,"version":null}"#;
        let a: AgentInfo = serde_json::from_str(json).unwrap();
        assert_eq!(a.version, Some(None));
        let back = serde_json::to_string(&a).unwrap();
        assert!(back.contains("\"version\":null"));
    }

    #[test]
    fn skill_summary_full_round_trip() {
        let s = SkillSummary {
            id: "web-prototype".into(),
            name: "Web Prototype".into(),
            description: "x".into(),
            triggers: vec!["mockup".into()],
            mode: SkillMode::Prototype,
            surface: Some(SkillSurface::Web),
            platform: Some(Some(SkillPlatform::Desktop)),
            scenario: Some(None), // explicitly null
            preview_type: "html".into(),
            design_system_required: true,
            default_for: vec!["design".into()],
            upstream: Some("open-design".into()),
            featured: Some(Some(10)),
            fidelity: Some(Some(SkillFidelity::HighFidelity)),
            speaker_notes: None,
            animations: None,
            craft_requires: Some(vec!["typography".into(), "color".into()]),
            has_body: true,
            example_prompt: "Make me a landing page".into(),
        };
        let j = serde_json::to_string(&s).unwrap();
        assert!(j.contains("\"mode\":\"prototype\""));
        assert!(j.contains("\"surface\":\"web\""));
        assert!(j.contains("\"platform\":\"desktop\""));
        assert!(j.contains("\"scenario\":null"));
        assert!(j.contains("\"craftRequires\":[\"typography\",\"color\"]"));
        assert!(j.contains("\"designSystemRequired\":true"));
        assert!(j.contains("\"defaultFor\":[\"design\"]"));
        assert!(j.contains("\"hasBody\":true"));
        let back: SkillSummary = serde_json::from_str(&j).unwrap();
        assert_eq!(back.id, "web-prototype");
        assert_eq!(back.scenario, Some(None));
    }

    #[test]
    fn skill_detail_flattens_summary() {
        let d = SkillDetail {
            summary: SkillSummary {
                id: "x".into(),
                name: "X".into(),
                description: "d".into(),
                triggers: vec![],
                mode: SkillMode::Deck,
                surface: None,
                platform: None,
                scenario: None,
                preview_type: "html".into(),
                design_system_required: false,
                default_for: vec![],
                upstream: None,
                featured: None,
                fidelity: None,
                speaker_notes: None,
                animations: None,
                craft_requires: None,
                has_body: true,
                example_prompt: "p".into(),
            },
            body: "# Body".into(),
        };
        let j = serde_json::to_string(&d).unwrap();
        // Both summary and body appear at the top level.
        assert!(j.contains("\"id\":\"x\""));
        assert!(j.contains("\"body\":\"# Body\""));
    }

    #[test]
    fn design_systems_response_camel_case() {
        let r = DesignSystemsResponse {
            design_systems: vec![DesignSystemSummary {
                id: "linear-app".into(),
                title: "Linear".into(),
                category: "tools".into(),
                summary: "s".into(),
                swatches: Some(vec!["#5e6ad2".into()]),
                surface: Some(SkillSurface::Web),
            }],
        };
        let j = serde_json::to_string(&r).unwrap();
        assert!(j.contains("\"designSystems\""));
        assert!(j.contains("\"id\":\"linear-app\""));
    }

    #[test]
    fn health_response_locked_ok_and_service() {
        let r = HealthResponse {
            ok: HealthOk,
            service: Some(HealthService::Daemon),
            version: Some("1.0.0".into()),
        };
        let s = serde_json::to_string(&r).unwrap();
        assert_eq!(s, r#"{"ok":true,"service":"daemon","version":"1.0.0"}"#);
        // ok=false rejected.
        let bad = r#"{"ok":false,"service":"daemon"}"#;
        let err = serde_json::from_str::<HealthResponse>(bad).unwrap_err();
        assert!(err.to_string().contains("ok must be true"));
    }

    #[test]
    fn codex_pet_summary_camel_case() {
        let p = CodexPetSummary {
            id: "kit".into(),
            display_name: "Kit".into(),
            description: "tiny dragon".into(),
            spritesheet_url: "/pets/kit.png".into(),
            spritesheet_ext: "png".into(),
            hatched_at: 1_700_000_000_000,
            bundled: Some(false),
        };
        let s = serde_json::to_string(&p).unwrap();
        assert!(s.contains("\"displayName\":\"Kit\""));
        assert!(s.contains("\"spritesheetUrl\":\"/pets/kit.png\""));
        assert!(s.contains("\"spritesheetExt\":\"png\""));
        assert!(s.contains("\"hatchedAt\":1700000000000"));
    }

    #[test]
    fn sync_community_pets_request_optional_fields() {
        let req = SyncCommunityPetsRequest {
            source: Some(SyncCommunityPetsSource::Petshare),
            force: Some(true),
        };
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains("\"source\":\"petshare\""));
        assert!(s.contains("\"force\":true"));
        let empty: SyncCommunityPetsRequest = serde_json::from_str("{}").unwrap();
        assert!(empty.source.is_none() && empty.force.is_none());
    }

    #[test]
    fn sync_community_pets_response_round_trip() {
        let r = SyncCommunityPetsResponse {
            wrote: 3,
            skipped: 5,
            failed: 0,
            total: 8,
            root_dir: "/Users/u/.codex/pets".into(),
            errors: vec![],
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"rootDir\""));
        assert!(s.contains("\"errors\":[]"));
    }
}
