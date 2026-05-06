//! `/api/projects` — project CRUD, conversations, deployments,
//! preflight. Mirrors `@open-design/contracts/src/api/projects.ts`.
//!
//! This is the largest single file in the upstream contracts package
//! and the most user-visible (every project / conversation / deploy
//! action funnels through these shapes).

use serde::{Deserialize, Serialize};

use crate::api::chat::ChatMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ProjectKind {
    Prototype,
    Deck,
    Template,
    Other,
    Image,
    Video,
    Audio,
}

/// Aspect ratio for image/video/audio projects. Locked to the upstream
/// 5-value enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MediaAspect {
    #[serde(rename = "1:1")]
    Square,
    #[serde(rename = "16:9")]
    Wide,
    #[serde(rename = "9:16")]
    Tall,
    #[serde(rename = "4:3")]
    Standard,
    #[serde(rename = "3:4")]
    StandardTall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum AudioKind {
    Music,
    Speech,
    Sfx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProjectDisplayStatus {
    NotStarted,
    Queued,
    Running,
    AwaitingInput,
    Succeeded,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStatusInfo {
    pub value: ProjectDisplayStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromptTemplateMetadataSource {
    pub repo: String,
    pub license: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum PromptTemplateMetadataSurface {
    Image,
    Video,
}

/// Subset of a curated `PromptTemplate` kept on the project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromptTemplateMetadata {
    pub id: String,
    pub surface: PromptTemplateMetadataSurface,
    pub title: String,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect: Option<MediaAspect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<PromptTemplateMetadataSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ProjectFidelity {
    Wireframe,
    HighFidelity,
}

/// `'live-artifact'` literal upstream — locked single-variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ProjectIntent {
    LiveArtifact,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMetadata {
    pub kind: Option<ProjectKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<ProjectIntent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fidelity: Option<ProjectFidelity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_notes: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animations: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspiration_design_system_ids: Option<Vec<String>>,
    /// Free-form upstream — `'claude-design' | string`. Captured verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_file_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_aspect: Option<MediaAspect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_length: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_aspect: Option<MediaAspect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_kind: Option<AudioKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_duration: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_template: Option<PromptTemplateMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked_dirs: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub skill_id: Option<String>,
    pub design_system_id: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ProjectStatusInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ProjectMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectTemplateFile {
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTemplate {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_project_id: Option<String>,
    pub files: Vec<ProjectTemplateFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub project_id: String,
    pub title: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectRequest {
    pub name: String,
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
    pub pending_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ProjectMetadata>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub pending_prompt: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub metadata: Option<Option<ProjectMetadata>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectsResponse {
    pub projects: Vec<Project>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectResponse {
    pub project: Project,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectResponse {
    #[serde(flatten)]
    pub project_response: ProjectResponse,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversationsResponse {
    pub conversations: Vec<Conversation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversationResponse {
    pub conversation: Conversation,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateConversationRequest {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub title: Option<Option<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateConversationRequest {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub title: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessagesResponse {
    pub messages: Vec<ChatMessage>,
}

// ─── Deployments ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum DeployProviderId {
    VercelSelf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum DeploymentStatus {
    Deploying,
    PreparingLink,
    Ready,
    LinkDelayed,
    Protected,
    Failed,
}

/// `'preview'` literal upstream — locked single-variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum DeployTarget {
    Preview,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeployConfigResponse {
    pub provider_id: DeployProviderId,
    pub configured: bool,
    pub token_mask: String,
    pub team_id: String,
    pub team_slug: String,
    pub target: DeployTarget,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDeployConfigRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentInfo {
    pub id: String,
    pub project_id: String,
    pub file_name: String,
    pub provider_id: DeployProviderId,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment_id: Option<String>,
    pub deployment_count: u32,
    pub target: DeployTarget,
    pub status: DeploymentStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reachable_at: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectDeploymentsResponse {
    pub deployments: Vec<DeploymentInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeployProjectFileRequest {
    pub file_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<DeployProviderId>,
}

/// `extends DeploymentInfo` upstream — same shape.
pub type DeployProjectFileResponse = DeploymentInfo;

/// `extends DeploymentInfo` upstream — same shape.
pub type CheckDeploymentLinkResponse = DeploymentInfo;

// ─── Preflight ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum DeployPreflightWarningCode {
    BrokenReference,
    InvalidReference,
    LargeAsset,
    LargeBundle,
    LargeHtml,
    ExternalScript,
    ExternalStylesheet,
    NoDoctype,
    NoViewport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeployPreflightWarning {
    pub code: DeployPreflightWarningCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeployPreflightFile {
    pub path: String,
    pub size: u64,
    pub mime: String,
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeployPreflightRequest {
    pub file_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<DeployProviderId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeployPreflightResponse {
    pub provider_id: DeployProviderId,
    pub entry: String,
    pub files: Vec<DeployPreflightFile>,
    pub total_files: u32,
    pub total_bytes: u64,
    pub warnings: Vec<DeployPreflightWarning>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_kind_lowercase() {
        assert_eq!(
            serde_json::to_string(&ProjectKind::Prototype).unwrap(),
            "\"prototype\""
        );
        assert_eq!(serde_json::to_string(&ProjectKind::Audio).unwrap(), "\"audio\"");
    }

    #[test]
    fn media_aspect_uses_colon_format() {
        for (slug, val) in [
            ("1:1", MediaAspect::Square),
            ("16:9", MediaAspect::Wide),
            ("9:16", MediaAspect::Tall),
            ("4:3", MediaAspect::Standard),
            ("3:4", MediaAspect::StandardTall),
        ] {
            let q = format!("\"{slug}\"");
            let parsed: MediaAspect = serde_json::from_str(&q).unwrap();
            assert_eq!(parsed, val);
            assert_eq!(serde_json::to_string(&val).unwrap(), q);
        }
    }

    #[test]
    fn project_display_status_snake_case() {
        let s: ProjectDisplayStatus = serde_json::from_str("\"awaiting_input\"").unwrap();
        assert_eq!(s, ProjectDisplayStatus::AwaitingInput);
        assert_eq!(
            serde_json::to_string(&ProjectDisplayStatus::NotStarted).unwrap(),
            "\"not_started\""
        );
    }

    #[test]
    fn project_status_info_camel_case() {
        let s = ProjectStatusInfo {
            value: ProjectDisplayStatus::Running,
            updated_at: Some(1_700_000_000_000),
            run_id: Some("r1".into()),
        };
        let j = serde_json::to_string(&s).unwrap();
        assert!(j.contains("\"value\":\"running\""));
        assert!(j.contains("\"updatedAt\":1700000000000"));
        assert!(j.contains("\"runId\":\"r1\""));
    }

    #[test]
    fn prompt_template_metadata_round_trip() {
        let m = PromptTemplateMetadata {
            id: "x".into(),
            surface: PromptTemplateMetadataSurface::Image,
            title: "T".into(),
            prompt: "p".into(),
            summary: None,
            category: None,
            tags: Some(vec!["anime".into()]),
            model: Some("gpt-image-2".into()),
            aspect: Some(MediaAspect::Square),
            source: Some(PromptTemplateMetadataSource {
                repo: "r".into(),
                license: "MIT".into(),
                author: None,
                url: None,
            }),
        };
        let j = serde_json::to_string(&m).unwrap();
        assert!(j.contains("\"surface\":\"image\""));
        assert!(j.contains("\"aspect\":\"1:1\""));
    }

    #[test]
    fn project_metadata_round_trip_partial() {
        let m = ProjectMetadata {
            kind: Some(ProjectKind::Deck),
            intent: Some(ProjectIntent::LiveArtifact),
            fidelity: Some(ProjectFidelity::HighFidelity),
            speaker_notes: Some(true),
            animations: Some(false),
            template_id: Some("t1".into()),
            template_label: None,
            inspiration_design_system_ids: Some(vec!["linear-app".into()]),
            imported_from: Some("claude-design".into()),
            entry_file: None,
            source_file_name: None,
            image_model: None,
            image_aspect: None,
            image_style: None,
            video_model: None,
            video_length: None,
            video_aspect: None,
            audio_kind: None,
            audio_model: None,
            audio_duration: None,
            voice: None,
            prompt_template: None,
            linked_dirs: Some(vec!["/tmp/code".into()]),
        };
        let j = serde_json::to_string(&m).unwrap();
        assert!(j.contains("\"kind\":\"deck\""));
        assert!(j.contains("\"intent\":\"live-artifact\""));
        assert!(j.contains("\"fidelity\":\"high-fidelity\""));
        assert!(j.contains("\"speakerNotes\":true"));
        assert!(j.contains("\"animations\":false"));
        assert!(j.contains("\"importedFrom\":\"claude-design\""));
        assert!(j.contains("\"linkedDirs\":[\"/tmp/code\"]"));
        // Skipped fields stay absent.
        assert!(!j.contains("imageModel"));
        assert!(!j.contains("audioKind"));
    }

    #[test]
    fn project_round_trip_with_optional_fields() {
        let p = Project {
            id: "p1".into(),
            name: "My Project".into(),
            skill_id: Some("web-prototype".into()),
            design_system_id: None,
            created_at: 1_700_000_000_000,
            updated_at: 1_700_000_005_000,
            status: Some(ProjectStatusInfo {
                value: ProjectDisplayStatus::Running,
                updated_at: None,
                run_id: None,
            }),
            pending_prompt: None,
            metadata: None,
        };
        let j = serde_json::to_string(&p).unwrap();
        assert!(j.contains("\"skillId\":\"web-prototype\""));
        assert!(j.contains("\"designSystemId\":null"));
        assert!(j.contains("\"createdAt\":1700000000000"));
        assert!(!j.contains("pendingPrompt"));
        let back: Project = serde_json::from_str(&j).unwrap();
        assert_eq!(back.id, "p1");
    }

    #[test]
    fn create_request_double_option_clears_skill() {
        let json = r#"{"name":"P","skillId":null}"#;
        let r: CreateProjectRequest = serde_json::from_str(json).unwrap();
        assert_eq!(r.skill_id, Some(None));
        let back = serde_json::to_string(&r).unwrap();
        assert!(back.contains("\"skillId\":null"));
    }

    #[test]
    fn create_response_flattens_project() {
        let r = CreateProjectResponse {
            project_response: ProjectResponse {
                project: Project {
                    id: "p1".into(),
                    name: "n".into(),
                    skill_id: None,
                    design_system_id: None,
                    created_at: 0,
                    updated_at: 0,
                    status: None,
                    pending_prompt: None,
                    metadata: None,
                },
            },
            conversation_id: Some("c1".into()),
        };
        let j = serde_json::to_string(&r).unwrap();
        assert!(j.contains("\"project\""));
        assert!(j.contains("\"conversationId\":\"c1\""));
    }

    #[test]
    fn deploy_provider_kebab_case() {
        assert_eq!(
            serde_json::to_string(&DeployProviderId::VercelSelf).unwrap(),
            "\"vercel-self\""
        );
    }

    #[test]
    fn deployment_status_kebab_case() {
        for (slug, st) in [
            ("deploying", DeploymentStatus::Deploying),
            ("preparing-link", DeploymentStatus::PreparingLink),
            ("ready", DeploymentStatus::Ready),
            ("link-delayed", DeploymentStatus::LinkDelayed),
            ("protected", DeploymentStatus::Protected),
            ("failed", DeploymentStatus::Failed),
        ] {
            let q = format!("\"{slug}\"");
            let parsed: DeploymentStatus = serde_json::from_str(&q).unwrap();
            assert_eq!(parsed, st);
        }
    }

    #[test]
    fn deployment_info_camel_case_round_trip() {
        let d = DeploymentInfo {
            id: "d1".into(),
            project_id: "p1".into(),
            file_name: "index.html".into(),
            provider_id: DeployProviderId::VercelSelf,
            url: "https://example.invalid".into(),
            deployment_id: Some("vd1".into()),
            deployment_count: 1,
            target: DeployTarget::Preview,
            status: DeploymentStatus::Ready,
            status_message: None,
            reachable_at: Some(1_700_000_010_000),
            created_at: 1_700_000_000_000,
            updated_at: 1_700_000_010_000,
        };
        let j = serde_json::to_string(&d).unwrap();
        assert!(j.contains("\"projectId\":\"p1\""));
        assert!(j.contains("\"fileName\":\"index.html\""));
        assert!(j.contains("\"providerId\":\"vercel-self\""));
        assert!(j.contains("\"deploymentId\":\"vd1\""));
        assert!(j.contains("\"target\":\"preview\""));
        assert!(j.contains("\"status\":\"ready\""));
        assert!(j.contains("\"reachableAt\":1700000010000"));
    }

    #[test]
    fn preflight_warning_codes_kebab_case() {
        for (slug, code) in [
            ("broken-reference", DeployPreflightWarningCode::BrokenReference),
            ("invalid-reference", DeployPreflightWarningCode::InvalidReference),
            ("large-asset", DeployPreflightWarningCode::LargeAsset),
            ("large-bundle", DeployPreflightWarningCode::LargeBundle),
            ("large-html", DeployPreflightWarningCode::LargeHtml),
            ("external-script", DeployPreflightWarningCode::ExternalScript),
            ("external-stylesheet", DeployPreflightWarningCode::ExternalStylesheet),
            ("no-doctype", DeployPreflightWarningCode::NoDoctype),
            ("no-viewport", DeployPreflightWarningCode::NoViewport),
        ] {
            let q = format!("\"{slug}\"");
            let parsed: DeployPreflightWarningCode = serde_json::from_str(&q).unwrap();
            assert_eq!(parsed, code);
        }
    }

    #[test]
    fn preflight_response_round_trip() {
        let r = DeployPreflightResponse {
            provider_id: DeployProviderId::VercelSelf,
            entry: "index.html".into(),
            files: vec![DeployPreflightFile {
                path: "index.html".into(),
                size: 4096,
                mime: "text/html".into(),
                source_path: "/p1/index.html".into(),
            }],
            total_files: 1,
            total_bytes: 4096,
            warnings: vec![DeployPreflightWarning {
                code: DeployPreflightWarningCode::NoViewport,
                message: "missing viewport meta".into(),
                path: Some("index.html".into()),
                url: None,
                size: None,
            }],
        };
        let j = serde_json::to_string(&r).unwrap();
        assert!(j.contains("\"providerId\":\"vercel-self\""));
        assert!(j.contains("\"totalFiles\":1"));
        assert!(j.contains("\"totalBytes\":4096"));
        assert!(j.contains("\"code\":\"no-viewport\""));
        assert!(j.contains("\"sourcePath\":\"/p1/index.html\""));
        // The file's `size` is a required field — appears.
        assert!(j.contains("\"size\":4096"));
        // The warning's optional `url`/`size` were None — should be omitted
        // (nothing else has `url`).
        assert!(!j.contains("\"url\""));
    }

    #[test]
    fn audio_kind_lowercase() {
        assert_eq!(serde_json::to_string(&AudioKind::Sfx).unwrap(), "\"sfx\"");
    }

    #[test]
    fn template_alias_round_trip_via_deployment_info() {
        let d: DeployProjectFileResponse = DeploymentInfo {
            id: "d1".into(),
            project_id: "p1".into(),
            file_name: "x.html".into(),
            provider_id: DeployProviderId::VercelSelf,
            url: "u".into(),
            deployment_id: None,
            deployment_count: 0,
            target: DeployTarget::Preview,
            status: DeploymentStatus::Deploying,
            status_message: None,
            reachable_at: None,
            created_at: 0,
            updated_at: 0,
        };
        let j = serde_json::to_string(&d).unwrap();
        let back: CheckDeploymentLinkResponse = serde_json::from_str(&j).unwrap();
        assert_eq!(back.id, "d1");
    }
}
