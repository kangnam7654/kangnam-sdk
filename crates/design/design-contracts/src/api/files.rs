//! `/api/projects/:id/files` — project file system shapes. Mirrors
//! `@open-design/contracts/src/api/files.ts`.

use serde::{Deserialize, Serialize};

use crate::api::artifacts::{ArtifactKind, ArtifactManifest};
use crate::common::OkResponse;

/// Coarse classification of a project file. Drives the file-tree icon
/// + the auto-pick "open with" decision (artifacts → live artifact view,
///   HTML → preview, sketch → sketch view, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ProjectFileKind {
    Html,
    Image,
    Video,
    Audio,
    Sketch,
    Text,
    Code,
    Pdf,
    Document,
    Presentation,
    Spreadsheet,
    Binary,
}

/// `'file' | 'dir'` discriminator returned by directory listings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ProjectFileType {
    File,
    Dir,
}

/// One project-tree entry. `mtime` is Unix epoch milliseconds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectFile {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<ProjectFileType>,
    pub size: u64,
    pub mtime: u64,
    pub kind: ProjectFileKind,
    pub mime: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_kind: Option<ArtifactKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_manifest: Option<ArtifactManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectFilesResponse {
    pub files: Vec<ProjectFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectFileResponse {
    pub file: ProjectFile,
}

/// `extends ProjectFilesResponse` upstream — same shape.
pub type UploadProjectFilesResponse = ProjectFilesResponse;

/// `extends OkResponse` upstream — `{ok: true}`.
pub type DeleteProjectFileResponse = OkResponse;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_file_kind_lowercase() {
        let k: ProjectFileKind = serde_json::from_str("\"presentation\"").unwrap();
        assert_eq!(k, ProjectFileKind::Presentation);
        assert_eq!(
            serde_json::to_string(&ProjectFileKind::Spreadsheet).unwrap(),
            "\"spreadsheet\""
        );
    }

    #[test]
    fn project_file_type_renamed_field() {
        let f = ProjectFile {
            name: "x.html".into(),
            path: None,
            r#type: Some(ProjectFileType::File),
            size: 100,
            mtime: 1_700_000_000_000,
            kind: ProjectFileKind::Html,
            mime: "text/html".into(),
            artifact_kind: None,
            artifact_manifest: None,
        };
        let s = serde_json::to_string(&f).unwrap();
        // The Rust field is `r#type` but serde keeps it as `type` on the wire.
        assert!(s.contains("\"type\":\"file\""));
        let back: ProjectFile = serde_json::from_str(&s).unwrap();
        assert_eq!(back.r#type, Some(ProjectFileType::File));
    }

    #[test]
    fn project_file_round_trip_with_artifact_manifest() {
        use crate::api::artifacts::*;
        let f = ProjectFile {
            name: "deck.html".into(),
            path: Some("artifacts/deck.html".into()),
            r#type: Some(ProjectFileType::File),
            size: 4096,
            mtime: 1_700_000_005_000,
            kind: ProjectFileKind::Html,
            mime: "text/html".into(),
            artifact_kind: Some(ArtifactKind::Deck),
            artifact_manifest: Some(ArtifactManifest {
                version: ManifestVersion,
                kind: ArtifactKind::Deck,
                title: "Deck".into(),
                entry: "deck.html".into(),
                renderer: ArtifactRendererId::DeckHtml,
                status: Some(ArtifactStatus::Complete),
                exports: vec![ArtifactExportKind::Pptx],
                supporting_files: None,
                created_at: None,
                updated_at: None,
                source_skill_id: None,
                design_system_id: None,
                metadata: None,
            }),
        };
        let s = serde_json::to_string(&f).unwrap();
        let back: ProjectFile = serde_json::from_str(&s).unwrap();
        assert_eq!(back.artifact_kind, Some(ArtifactKind::Deck));
        assert_eq!(back.artifact_manifest.unwrap().kind, ArtifactKind::Deck);
    }

    #[test]
    fn list_response_round_trip() {
        let r = ProjectFilesResponse {
            files: vec![ProjectFile {
                name: "a.txt".into(),
                path: None,
                r#type: None,
                size: 0,
                mtime: 0,
                kind: ProjectFileKind::Text,
                mime: "text/plain".into(),
                artifact_kind: None,
                artifact_manifest: None,
            }],
        };
        let s = serde_json::to_string(&r).unwrap();
        let back: ProjectFilesResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(back.files.len(), 1);
        assert_eq!(back.files[0].name, "a.txt");
    }

    #[test]
    fn upload_response_alias_is_files_response() {
        let r: UploadProjectFilesResponse = ProjectFilesResponse { files: vec![] };
        let s = serde_json::to_string(&r).unwrap();
        assert_eq!(s, r#"{"files":[]}"#);
    }

    #[test]
    fn delete_response_alias_is_ok_response() {
        let r: DeleteProjectFileResponse = OkResponse;
        let s = serde_json::to_string(&r).unwrap();
        assert_eq!(s, r#"{"ok":true}"#);
    }
}
