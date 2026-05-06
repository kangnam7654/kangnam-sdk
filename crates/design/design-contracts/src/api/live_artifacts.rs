//! `/api/projects/:id/live-artifacts` — live-artifact lifecycle. Mirrors
//! `@open-design/contracts/src/api/live-artifacts.ts`.
//!
//! `BoundedJsonObject` and `BoundedJsonValue` originate here in the
//! upstream package — they're used by both this module and
//! [`crate::api::connectors`]. Re-exported from the top-level
//! [`crate::api`] for callers that don't want to remember the source
//! module.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Recursive bounded-JSON value. Mirror of upstream's `BoundedJsonValue`
/// — same shape as `serde_json::Value` but with the runtime invariant
/// that the value fits within
/// [`crate::common::LIVE_ARTIFACT_BOUNDED_JSON_CONSTRAINTS`].
///
/// We type-alias to `serde_json::Value` rather than redefining the
/// recursive enum: validation is a runtime check (the upstream daemon
/// runs a custom walker), not part of the type. Callers that need the
/// constraint enforced should pass values through a validator pass
/// after deserialization.
pub type BoundedJsonValue = serde_json::Value;

/// Object-only bounded-JSON variant. Wire-compatible with
/// `serde_json::Map<String, Value>` and `HashMap<String, Value>`.
pub type BoundedJsonObject = HashMap<String, BoundedJsonValue>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum LiveArtifactStatus {
    Active,
    Archived,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum LiveArtifactRefreshStatus {
    Never,
    Idle,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum LiveArtifactPreviewType {
    Html,
    Jsx,
    Markdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LiveArtifactSourceType {
    LocalFile,
    DaemonTool,
    ConnectorTool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LiveArtifactConnectorApprovalPolicy {
    ReadOnlyAuto,
    ManualRefreshGrantedForReadOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LiveArtifactRefreshPermission {
    None,
    ManualRefreshGrantedForReadOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LiveArtifactOutputTransform {
    Identity,
    CompactTable,
    MetricSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LiveArtifactProvenanceGenerator {
    Agent,
    RefreshRunner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LiveArtifactProvenanceSourceType {
    Connector,
    LocalFile,
    UserInput,
    Derived,
}

/// Inline preview info — which renderer the web client should mount.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LiveArtifactPreview {
    pub r#type: LiveArtifactPreviewType,
    pub entry: String,
}

/// HTML-template document format. The upstream `format` field is
/// frozen to the literal string `"html_template_v1"` — we mirror with
/// a tag-locked enum that has only one variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LiveArtifactDocumentFormat {
    /// `"html_template_v1"`
    HtmlTemplateV1,
}

/// Document body — the canonical files written under the artifact's
/// directory. `template_path`, `generated_preview_path`, `data_path`
/// are constants; the real flexibility lives in `data_json` / source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveArtifactDocument {
    pub format: LiveArtifactDocumentFormat,
    /// Always `"template.html"` — keep as `String` to round-trip the
    /// upstream literal-string discriminant verbatim.
    pub template_path: String,
    /// Always `"index.html"`.
    pub generated_preview_path: String,
    /// Always `"data.json"`.
    pub data_path: String,
    /// Derived cache hydrated from `data_path` in API responses;
    /// `data.json` on disk is the canonical source of truth.
    pub data_json: BoundedJsonObject,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_schema_json: Option<BoundedJsonObject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_json: Option<LiveArtifactSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveArtifactSourceConnector {
    pub connector_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_label: Option<String>,
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<LiveArtifactConnectorApprovalPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LiveArtifactOutputMappingPath {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveArtifactOutputMapping {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_paths: Option<Vec<LiveArtifactOutputMappingPath>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<LiveArtifactOutputTransform>,
}

/// Source descriptor — where the artifact data came from and how to
/// refresh it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveArtifactSource {
    pub r#type: LiveArtifactSourceType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    pub input: BoundedJsonObject,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector: Option<LiveArtifactSourceConnector>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_mapping: Option<LiveArtifactOutputMapping>,
    pub refresh_permission: LiveArtifactRefreshPermission,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LiveArtifactProvenanceSource {
    pub label: String,
    pub r#type: LiveArtifactProvenanceSourceType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveArtifactProvenance {
    pub generated_at: String,
    pub generated_by: LiveArtifactProvenanceGenerator,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub sources: Vec<LiveArtifactProvenanceSource>,
}

/// Schema-version newtype — locks the on-wire integer to `1` (mirrors
/// the [`crate::api::artifacts::ManifestVersion`] approach).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveArtifactSchemaVersion;

impl Serialize for LiveArtifactSchemaVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u32(1)
    }
}

impl<'de> Deserialize<'de> for LiveArtifactSchemaVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = u32::deserialize(deserializer)?;
        if v == 1 {
            Ok(LiveArtifactSchemaVersion)
        } else {
            Err(serde::de::Error::custom(format!(
                "unsupported LiveArtifact schemaVersion: {v}, expected 1"
            )))
        }
    }
}

/// One live artifact — the fully materialized record. `created_at` /
/// `updated_at` / `last_refreshed_at` are ISO-8601 strings (matching
/// upstream).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveArtifact {
    pub schema_version: LiveArtifactSchemaVersion,
    pub id: String,
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by_run_id: Option<String>,
    pub title: String,
    pub slug: String,
    pub status: LiveArtifactStatus,
    pub pinned: bool,
    pub preview: LiveArtifactPreview,
    pub refresh_status: LiveArtifactRefreshStatus,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_refreshed_at: Option<String>,
    pub document: LiveArtifactDocument,
}

/// Daemon-owned input fields — listed in upstream as a derived TS type
/// to enforce that `Create` / `Update` requests don't smuggle them in.
/// We mirror the list as a `&[&str]` for runtime guards.
pub const DAEMON_OWNED_INPUT_FIELDS: &[&str] = &[
    "id",
    "projectId",
    "createdAt",
    "updatedAt",
    "createdByRunId",
    "schemaVersion",
    "refreshStatus",
    "lastRefreshedAt",
];

/// Create-input shape. Server-managed fields (`id`, `createdAt`, …) are
/// not modeled here; if a caller provides them in JSON they're simply
/// ignored (the daemon enforces the upstream `LiveArtifactRejectDaemonOwnedInputFields`
/// guard at the validator pass).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveArtifactCreateInput {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<LiveArtifactStatus>,
    pub preview: LiveArtifactPreview,
    pub document: LiveArtifactDocument,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveArtifactUpdateInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<LiveArtifactStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<LiveArtifactPreview>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<LiveArtifactDocument>,
}

/// Summary projection — `Omit<LiveArtifact, 'document'> & { hasDocument }`.
/// Returned by list endpoints to keep payloads light.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveArtifactSummary {
    pub schema_version: LiveArtifactSchemaVersion,
    pub id: String,
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by_run_id: Option<String>,
    pub title: String,
    pub slug: String,
    pub status: LiveArtifactStatus,
    pub pinned: bool,
    pub preview: LiveArtifactPreview,
    pub refresh_status: LiveArtifactRefreshStatus,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_refreshed_at: Option<String>,
    pub has_document: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LiveArtifactListResponse {
    pub artifacts: Vec<LiveArtifactSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LiveArtifactDetailResponse {
    pub artifact: LiveArtifact,
}

/// Sentinel `'succeeded'` literal upstream — the `refresh.status` field
/// is locked to that single value. Modeled with a frozen single-variant
/// enum so any future evolution is a deliberate schema bump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum LiveArtifactRefreshTerminalStatus {
    Succeeded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveArtifactRefreshSummary {
    pub id: String,
    pub status: LiveArtifactRefreshTerminalStatus,
    pub refreshed_source_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LiveArtifactRefreshResponse {
    pub artifact: LiveArtifact,
    pub refresh: LiveArtifactRefreshSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum LiveArtifactRefreshStepStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LiveArtifactRefreshErrorRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// `'document'` literal upstream — locked single-variant enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum LiveArtifactRefreshSourceTypeTag {
    Document,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveArtifactRefreshSourceMetadata {
    pub source_type: LiveArtifactRefreshSourceTypeTag,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector: Option<LiveArtifactSourceConnector>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveArtifactRefreshLogEntry {
    pub schema_version: LiveArtifactSchemaVersion,
    pub project_id: String,
    pub artifact_id: String,
    pub refresh_id: String,
    pub sequence: u32,
    pub step: String,
    pub status: LiveArtifactRefreshStepStatus,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<LiveArtifactRefreshSourceMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<LiveArtifactRefreshErrorRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BoundedJsonObject>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LiveArtifactRefreshLogResponse {
    pub refreshes: Vec<LiveArtifactRefreshLogEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_enums_lowercase() {
        assert_eq!(
            serde_json::to_string(&LiveArtifactStatus::Archived).unwrap(),
            "\"archived\""
        );
        assert_eq!(
            serde_json::to_string(&LiveArtifactRefreshStatus::Running).unwrap(),
            "\"running\""
        );
    }

    #[test]
    fn enum_snake_case_variants() {
        assert_eq!(
            serde_json::to_string(&LiveArtifactSourceType::ConnectorTool).unwrap(),
            "\"connector_tool\""
        );
        assert_eq!(
            serde_json::to_string(&LiveArtifactConnectorApprovalPolicy::ManualRefreshGrantedForReadOnly)
                .unwrap(),
            "\"manual_refresh_granted_for_read_only\""
        );
        assert_eq!(
            serde_json::to_string(&LiveArtifactProvenanceGenerator::RefreshRunner).unwrap(),
            "\"refresh_runner\""
        );
    }

    #[test]
    fn schema_version_locked_to_one() {
        let lit = serde_json::to_string(&LiveArtifactSchemaVersion).unwrap();
        assert_eq!(lit, "1");
        let parsed: LiveArtifactSchemaVersion = serde_json::from_str("1").unwrap();
        assert_eq!(parsed, LiveArtifactSchemaVersion);
        let err = serde_json::from_str::<LiveArtifactSchemaVersion>("2").unwrap_err();
        assert!(err.to_string().contains("expected 1"));
    }

    #[test]
    fn document_round_trip_camel_case() {
        let mut data = BoundedJsonObject::new();
        data.insert("greeting".into(), serde_json::json!("hello"));
        let d = LiveArtifactDocument {
            format: LiveArtifactDocumentFormat::HtmlTemplateV1,
            template_path: "template.html".into(),
            generated_preview_path: "index.html".into(),
            data_path: "data.json".into(),
            data_json: data,
            data_schema_json: None,
            source_json: None,
        };
        let s = serde_json::to_string(&d).unwrap();
        assert!(s.contains("\"format\":\"html_template_v1\""));
        assert!(s.contains("\"templatePath\":\"template.html\""));
        assert!(s.contains("\"generatedPreviewPath\":\"index.html\""));
        assert!(s.contains("\"dataJson\":{\"greeting\":\"hello\"}"));
        let back: LiveArtifactDocument = serde_json::from_str(&s).unwrap();
        assert_eq!(back.template_path, "template.html");
    }

    #[test]
    fn source_full_round_trip() {
        let mut input = BoundedJsonObject::new();
        input.insert("q".into(), serde_json::json!(42));
        let src = LiveArtifactSource {
            r#type: LiveArtifactSourceType::DaemonTool,
            tool_name: Some("Read".into()),
            input,
            connector: Some(LiveArtifactSourceConnector {
                connector_id: "conn1".into(),
                account_label: Some("alice".into()),
                tool_name: "GetDoc".into(),
                approval_policy: Some(LiveArtifactConnectorApprovalPolicy::ReadOnlyAuto),
            }),
            output_mapping: Some(LiveArtifactOutputMapping {
                data_paths: Some(vec![LiveArtifactOutputMappingPath {
                    from: "$.x".into(),
                    to: "data.x".into(),
                }]),
                transform: Some(LiveArtifactOutputTransform::CompactTable),
            }),
            refresh_permission: LiveArtifactRefreshPermission::ManualRefreshGrantedForReadOnly,
        };
        let s = serde_json::to_string(&src).unwrap();
        assert!(s.contains("\"type\":\"daemon_tool\""));
        assert!(s.contains("\"toolName\":\"Read\""));
        assert!(s.contains("\"approvalPolicy\":\"read_only_auto\""));
        assert!(s.contains("\"transform\":\"compact_table\""));
        assert!(s.contains("\"refreshPermission\":\"manual_refresh_granted_for_read_only\""));
        let back: LiveArtifactSource = serde_json::from_str(&s).unwrap();
        assert_eq!(back.r#type, LiveArtifactSourceType::DaemonTool);
    }

    #[test]
    fn provenance_keeps_ref_field_via_raw_keyword() {
        let p = LiveArtifactProvenance {
            generated_at: "2026-05-06T00:00:00Z".into(),
            generated_by: LiveArtifactProvenanceGenerator::Agent,
            notes: None,
            sources: vec![LiveArtifactProvenanceSource {
                label: "uploads/data.csv".into(),
                r#type: LiveArtifactProvenanceSourceType::LocalFile,
                r#ref: Some("sha256:abc".into()),
            }],
        };
        let s = serde_json::to_string(&p).unwrap();
        // r#type and r#ref serialize as bare "type" / "ref" upstream.
        assert!(s.contains("\"type\":\"local_file\""));
        assert!(s.contains("\"ref\":\"sha256:abc\""));
    }

    #[test]
    fn create_input_omits_daemon_owned_fields() {
        let p = LiveArtifactPreview {
            r#type: LiveArtifactPreviewType::Html,
            entry: "index.html".into(),
        };
        let d = LiveArtifactDocument {
            format: LiveArtifactDocumentFormat::HtmlTemplateV1,
            template_path: "template.html".into(),
            generated_preview_path: "index.html".into(),
            data_path: "data.json".into(),
            data_json: BoundedJsonObject::new(),
            data_schema_json: None,
            source_json: None,
        };
        let c = LiveArtifactCreateInput {
            title: "Hello".into(),
            slug: None,
            session_id: None,
            pinned: None,
            status: None,
            preview: p,
            document: d,
        };
        let s = serde_json::to_string(&c).unwrap();
        // None of the daemon-owned fields should appear in the create input.
        for field in DAEMON_OWNED_INPUT_FIELDS {
            assert!(
                !s.contains(&format!("\"{field}\"")),
                "create input leaked {field}"
            );
        }
    }

    #[test]
    fn refresh_terminal_status_locked_to_succeeded() {
        let r = LiveArtifactRefreshSummary {
            id: "r1".into(),
            status: LiveArtifactRefreshTerminalStatus::Succeeded,
            refreshed_source_count: 3,
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"status\":\"succeeded\""));
        assert!(s.contains("\"refreshedSourceCount\":3"));
        // Anything else fails to deserialize.
        let bad = s.replace("\"status\":\"succeeded\"", "\"status\":\"failed\"");
        let err = serde_json::from_str::<LiveArtifactRefreshSummary>(&bad).unwrap_err();
        assert!(err.to_string().contains("succeeded"));
    }

    #[test]
    fn refresh_log_entry_round_trip() {
        let e = LiveArtifactRefreshLogEntry {
            schema_version: LiveArtifactSchemaVersion,
            project_id: "p1".into(),
            artifact_id: "a1".into(),
            refresh_id: "r1".into(),
            sequence: 0,
            step: "fetch".into(),
            status: LiveArtifactRefreshStepStatus::Succeeded,
            started_at: "t0".into(),
            finished_at: Some("t1".into()),
            duration_ms: Some(120),
            source: Some(LiveArtifactRefreshSourceMetadata {
                source_type: LiveArtifactRefreshSourceTypeTag::Document,
                tool_name: None,
                connector: None,
            }),
            error: None,
            metadata: None,
            created_at: "t0".into(),
        };
        let s = serde_json::to_string(&e).unwrap();
        assert!(s.contains("\"schemaVersion\":1"));
        assert!(s.contains("\"sourceType\":\"document\""));
        assert!(!s.contains("error"));
        assert!(!s.contains("metadata"));
        let back: LiveArtifactRefreshLogEntry = serde_json::from_str(&s).unwrap();
        assert_eq!(back.refresh_id, "r1");
    }

    #[test]
    fn summary_replaces_document_with_has_document_flag() {
        let summ = LiveArtifactSummary {
            schema_version: LiveArtifactSchemaVersion,
            id: "a1".into(),
            project_id: "p1".into(),
            session_id: None,
            created_by_run_id: None,
            title: "T".into(),
            slug: "t".into(),
            status: LiveArtifactStatus::Active,
            pinned: false,
            preview: LiveArtifactPreview {
                r#type: LiveArtifactPreviewType::Html,
                entry: "index.html".into(),
            },
            refresh_status: LiveArtifactRefreshStatus::Idle,
            created_at: "t0".into(),
            updated_at: "t0".into(),
            last_refreshed_at: None,
            has_document: true,
        };
        let s = serde_json::to_string(&summ).unwrap();
        assert!(s.contains("\"hasDocument\":true"));
        // Summary must not embed the document body.
        assert!(!s.contains("\"document\""));
    }

    #[test]
    fn daemon_owned_fields_list_matches_upstream() {
        // Spot-check: must list 8 items.
        assert_eq!(DAEMON_OWNED_INPUT_FIELDS.len(), 8);
        for expected in [
            "id",
            "projectId",
            "createdAt",
            "updatedAt",
            "createdByRunId",
            "schemaVersion",
            "refreshStatus",
            "lastRefreshedAt",
        ] {
            assert!(
                DAEMON_OWNED_INPUT_FIELDS.contains(&expected),
                "missing {expected}"
            );
        }
    }
}
