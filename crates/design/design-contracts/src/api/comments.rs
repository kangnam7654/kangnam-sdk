//! `/api/projects/:id/comments` — preview-pane comment shapes. Mirrors
//! `@open-design/contracts/src/api/comments.ts`.

use serde::{Deserialize, Serialize};

use crate::common::OkResponse;

/// Lifecycle of one preview comment, from initial annotation through
/// resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PreviewCommentStatus {
    Open,
    Attached,
    Applying,
    /// Daemon thinks it has applied the change but wants the user to
    /// double-check before resolving.
    NeedsReview,
    Resolved,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct PreviewCommentPosition {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Discriminator for "is this a single element or a pod (group)?"
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum PreviewCommentSelectionKind {
    Element,
    Pod,
}

/// One member of a pod selection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewCommentMember {
    pub element_id: String,
    pub selector: String,
    pub label: String,
    pub text: String,
    pub position: PreviewCommentPosition,
    pub html_hint: String,
}

/// Selection target carried alongside a new comment — captures
/// everything the daemon needs to relocate the element on a re-render.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewCommentTarget {
    pub file_path: String,
    pub element_id: String,
    pub selector: String,
    pub label: String,
    pub text: String,
    pub position: PreviewCommentPosition,
    pub html_hint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_kind: Option<PreviewCommentSelectionKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pod_members: Option<Vec<PreviewCommentMember>>,
}

/// Persisted preview comment as returned by `GET /comments`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewComment {
    pub id: String,
    pub project_id: String,
    pub conversation_id: String,
    pub file_path: String,
    pub element_id: String,
    pub selector: String,
    pub label: String,
    pub text: String,
    pub position: PreviewCommentPosition,
    pub html_hint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_kind: Option<PreviewCommentSelectionKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pod_members: Option<Vec<PreviewCommentMember>>,
    pub note: String,
    pub status: PreviewCommentStatus,
    pub created_at: u64,
    pub updated_at: u64,
}

/// POST/PUT body to upsert a preview comment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreviewCommentUpsertRequest {
    pub target: PreviewCommentTarget,
    pub note: String,
}

/// PATCH body to flip just the status flag (e.g. user resolves).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreviewCommentStatusRequest {
    pub status: PreviewCommentStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreviewCommentResponse {
    pub comment: PreviewComment,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreviewCommentsResponse {
    pub comments: Vec<PreviewComment>,
}

/// `extends OkResponse` upstream — `{ok: true}`.
pub type PreviewCommentDeleteResponse = OkResponse;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_snake_case_round_trip() {
        assert_eq!(
            serde_json::to_string(&PreviewCommentStatus::NeedsReview).unwrap(),
            "\"needs_review\""
        );
        let s: PreviewCommentStatus = serde_json::from_str("\"applying\"").unwrap();
        assert_eq!(s, PreviewCommentStatus::Applying);
    }

    #[test]
    fn selection_kind_lowercase() {
        assert_eq!(
            serde_json::to_string(&PreviewCommentSelectionKind::Pod).unwrap(),
            "\"pod\""
        );
    }

    #[test]
    fn comment_camel_case_round_trip() {
        let c = PreviewComment {
            id: "c1".into(),
            project_id: "p1".into(),
            conversation_id: "conv1".into(),
            file_path: "index.html".into(),
            element_id: "el1".into(),
            selector: ".hero h1".into(),
            label: "Hero".into(),
            text: "Welcome".into(),
            position: PreviewCommentPosition {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 24.0,
            },
            html_hint: "<h1>".into(),
            selection_kind: None,
            member_count: None,
            pod_members: None,
            note: "Make it bigger".into(),
            status: PreviewCommentStatus::Open,
            created_at: 1_700_000_000_000,
            updated_at: 1_700_000_005_000,
        };
        let s = serde_json::to_string(&c).unwrap();
        assert!(s.contains("\"projectId\":\"p1\""));
        assert!(s.contains("\"filePath\":\"index.html\""));
        assert!(s.contains("\"elementId\":\"el1\""));
        assert!(s.contains("\"htmlHint\":\"<h1>\""));
        assert!(s.contains("\"createdAt\":1700000000000"));
        // Optional fields omitted.
        assert!(!s.contains("selectionKind"));
        assert!(!s.contains("memberCount"));
        assert!(!s.contains("podMembers"));
        let back: PreviewComment = serde_json::from_str(&s).unwrap();
        assert_eq!(back.id, "c1");
    }

    #[test]
    fn pod_target_carries_members() {
        let t = PreviewCommentTarget {
            file_path: "i.html".into(),
            element_id: "e1".into(),
            selector: ".pod".into(),
            label: "Pod".into(),
            text: "Block".into(),
            position: PreviewCommentPosition {
                x: 0.0,
                y: 0.0,
                width: 200.0,
                height: 200.0,
            },
            html_hint: "<div class=pod>".into(),
            selection_kind: Some(PreviewCommentSelectionKind::Pod),
            member_count: Some(3),
            pod_members: Some(vec![PreviewCommentMember {
                element_id: "e1.a".into(),
                selector: ".a".into(),
                label: "A".into(),
                text: "x".into(),
                position: PreviewCommentPosition {
                    x: 0.0,
                    y: 0.0,
                    width: 50.0,
                    height: 20.0,
                },
                html_hint: "<a>".into(),
            }]),
        };
        let s = serde_json::to_string(&t).unwrap();
        assert!(s.contains("\"selectionKind\":\"pod\""));
        assert!(s.contains("\"memberCount\":3"));
        assert!(s.contains("\"podMembers\""));
        let back: PreviewCommentTarget = serde_json::from_str(&s).unwrap();
        assert_eq!(
            back.selection_kind,
            Some(PreviewCommentSelectionKind::Pod)
        );
    }

    #[test]
    fn status_request_round_trip() {
        let r = PreviewCommentStatusRequest {
            status: PreviewCommentStatus::Resolved,
        };
        assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"status":"resolved"}"#);
    }

    #[test]
    fn delete_response_alias_is_ok_response() {
        let r: PreviewCommentDeleteResponse = OkResponse;
        let s = serde_json::to_string(&r).unwrap();
        assert_eq!(s, r#"{"ok":true}"#);
    }
}
