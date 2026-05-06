//! `/api/artifacts` — artifact kind / renderer / export-kind enums plus
//! the `ArtifactManifest` shape that the daemon writes alongside each
//! saved artifact. Mirrors `@open-design/contracts/src/api/artifacts.ts`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// What kind of artifact this is — drives renderer pick and export
/// menu. Wire format is the kebab-case slug.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ArtifactKind {
    Html,
    Deck,
    ReactComponent,
    MarkdownDocument,
    Svg,
    Diagram,
    CodeSnippet,
    MiniApp,
    DesignSystem,
}

/// Which renderer the web client should mount. Distinct from
/// [`ArtifactKind`] because some kinds share renderers (decks → `deck-html`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ArtifactRendererId {
    Html,
    DeckHtml,
    ReactComponent,
    Markdown,
    Svg,
    Diagram,
    Code,
    MiniApp,
    DesignSystem,
}

/// Available export targets for an artifact's `Export` menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ArtifactExportKind {
    Html,
    Pdf,
    Zip,
    Pptx,
    Jsx,
    Md,
    Svg,
    Txt,
}

/// Streaming → complete → error state machine for artifact rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ArtifactStatus {
    Streaming,
    Complete,
    Error,
}

/// On-disk manifest written next to each saved artifact's HTML/JSX file.
/// `version: 1` is hardcoded — the upstream uses a literal `1` to lock
/// the schema; we mirror that with a `u32` field that always serializes
/// as `1` and only deserializes successfully when the value is `1`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactManifest {
    /// Schema version. Always `1` — emit / accept only `1`.
    pub version: ManifestVersion,
    pub kind: ArtifactKind,
    pub title: String,
    /// Relative path to the entry file (e.g. `index.html`,
    /// `Component.jsx`).
    pub entry: String,
    pub renderer: ArtifactRendererId,
    /// Optional for backward compatibility with pre-streaming artifacts.
    /// Daemon/web manifest normalization defaults missing values to
    /// [`ArtifactStatus::Complete`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ArtifactStatus>,
    pub exports: Vec<ArtifactExportKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supporting_files: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_skill_id: Option<String>,
    /// `null` is meaningful here — distinct from "not set" — so we keep
    /// the JS-flavored double-Option via [`crate::serde_helpers::double_option`].
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub design_system_id: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

crate::locked_u32!(
    /// Schema-version newtype — locks the on-wire integer to `1`.
    pub struct ManifestVersion
    ; value = 1
    ; label = "ArtifactManifest version"
);

/// POST body to `/api/projects/:id/artifacts`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SaveArtifactRequest {
    pub identifier: String,
    pub title: String,
    pub html: String,
}

/// Response from `/api/projects/:id/artifacts`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SaveArtifactResponse {
    pub url: String,
    pub path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_kind_kebab_case() {
        assert_eq!(
            serde_json::to_string(&ArtifactKind::ReactComponent).unwrap(),
            "\"react-component\""
        );
        assert_eq!(
            serde_json::to_string(&ArtifactKind::MarkdownDocument).unwrap(),
            "\"markdown-document\""
        );
        assert_eq!(
            serde_json::to_string(&ArtifactKind::DesignSystem).unwrap(),
            "\"design-system\""
        );
    }

    #[test]
    fn renderer_id_kebab_case() {
        assert_eq!(
            serde_json::to_string(&ArtifactRendererId::DeckHtml).unwrap(),
            "\"deck-html\""
        );
        let r: ArtifactRendererId = serde_json::from_str("\"mini-app\"").unwrap();
        assert_eq!(r, ArtifactRendererId::MiniApp);
    }

    #[test]
    fn export_kind_lowercase() {
        assert_eq!(
            serde_json::to_string(&ArtifactExportKind::Pptx).unwrap(),
            "\"pptx\""
        );
    }

    #[test]
    fn status_lowercase() {
        let s: ArtifactStatus = serde_json::from_str("\"streaming\"").unwrap();
        assert_eq!(s, ArtifactStatus::Streaming);
    }

    #[test]
    fn manifest_version_locked_to_one() {
        let m = ArtifactManifest {
            version: ManifestVersion,
            kind: ArtifactKind::Html,
            title: "Hello".into(),
            entry: "index.html".into(),
            renderer: ArtifactRendererId::Html,
            status: Some(ArtifactStatus::Complete),
            exports: vec![ArtifactExportKind::Html, ArtifactExportKind::Pdf],
            supporting_files: None,
            created_at: None,
            updated_at: None,
            source_skill_id: None,
            design_system_id: None,
            metadata: None,
        };
        let s = serde_json::to_string(&m).unwrap();
        assert!(s.contains("\"version\":1"));
        // Wrong version fails to deserialize.
        let bad = s.replace("\"version\":1", "\"version\":2");
        let err = serde_json::from_str::<ArtifactManifest>(&bad).unwrap_err();
        assert!(err.to_string().contains("expected 1"));
    }

    #[test]
    fn manifest_camel_case_optional_fields_skip() {
        let m = ArtifactManifest {
            version: ManifestVersion,
            kind: ArtifactKind::Deck,
            title: "Deck".into(),
            entry: "index.html".into(),
            renderer: ArtifactRendererId::DeckHtml,
            status: None,
            exports: vec![ArtifactExportKind::Pptx],
            supporting_files: Some(vec!["styles.css".into()]),
            created_at: Some("2026-05-06T00:00:00Z".into()),
            updated_at: None,
            source_skill_id: Some("html-ppt-pitch-deck".into()),
            design_system_id: Some(None),
            metadata: None,
        };
        let s = serde_json::to_string(&m).unwrap();
        assert!(s.contains("\"supportingFiles\":[\"styles.css\"]"));
        assert!(s.contains("\"sourceSkillId\":\"html-ppt-pitch-deck\""));
        assert!(s.contains("\"createdAt\""));
        assert!(s.contains("\"designSystemId\":null"));
        assert!(!s.contains("status"));
        assert!(!s.contains("updatedAt"));
        assert!(!s.contains("metadata"));
    }

    #[test]
    fn save_artifact_request_response_round_trip() {
        let req = SaveArtifactRequest {
            identifier: "id-1".into(),
            title: "Title".into(),
            html: "<html />".into(),
        };
        let s = serde_json::to_string(&req).unwrap();
        assert_eq!(
            s,
            r#"{"identifier":"id-1","title":"Title","html":"<html />"}"#
        );
        let resp = SaveArtifactResponse {
            url: "/projects/p/artifacts/id-1.html".into(),
            path: "/abs/p/artifacts/id-1.html".into(),
        };
        let s2 = serde_json::to_string(&resp).unwrap();
        let back: SaveArtifactResponse = serde_json::from_str(&s2).unwrap();
        assert_eq!(back, resp);
    }
}
