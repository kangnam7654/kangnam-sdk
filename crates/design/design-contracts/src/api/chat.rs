//! `/api/chat` — agent run / message / persisted-event shapes. Mirrors
//! `@open-design/contracts/src/api/chat.ts`.
//!
//! `PersistedAgentEvent` is the persisted form of [`crate::sse::DaemonAgentPayload`]
//! — what gets stored back into the chat history once an agent step
//! completes. The two enums look similar but the persisted one drops
//! transient markers (`thinking_start`) and rolls usage tokens up.

use serde::{Deserialize, Serialize};

use crate::api::comments::{
    PreviewCommentMember, PreviewCommentPosition, PreviewCommentSelectionKind,
};
use crate::api::files::ProjectFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ChatRole {
    User,
    Assistant,
}

/// Run lifecycle. Distinct from [`crate::tasks::TaskState`] —
/// `canceled` (single-l) here, `cancelled` (double-l) on TaskState.
/// Upstream historical inconsistency mirrored verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ChatRunStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Canceled,
}

/// POST body to the chat endpoint. Most fields optional — the daemon
/// fills in defaults (active agent, active project, …) when omitted.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub agent_id: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub project_id: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub conversation_id: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub assistant_message_id: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub client_request_id: Option<Option<String>>,
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
    pub attachments: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment_attachments: Option<Vec<ChatCommentAttachment>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub model: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub reasoning: Option<Option<String>>,
}

/// Stricter variant where the four "we know who you are" fields are
/// non-optional (`extends ChatRequest` upstream tightens them with
/// `string`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatRunCreateRequest {
    /// Inline-flattened ChatRequest fields, with the four IDs tightened
    /// to required. Kept inline (not via `#[serde(flatten)]`) to keep
    /// field order deterministic on the wire.
    pub agent_id: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    pub project_id: String,
    pub conversation_id: String,
    pub assistant_message_id: String,
    pub client_request_id: String,
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
    pub attachments: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment_attachments: Option<Vec<ChatCommentAttachment>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub model: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub reasoning: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatRunCreateResponse {
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatRunStatusResponse {
    pub id: String,
    pub project_id: Option<String>,
    pub conversation_id: Option<String>,
    pub assistant_message_id: Option<String>,
    pub agent_id: Option<String>,
    pub status: ChatRunStatus,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatRunListResponse {
    pub runs: Vec<ChatRunStatusResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatRunCancelResponse {
    pub ok: bool,
}

/// Inline file/image attached to a chat message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatAttachment {
    pub path: String,
    pub name: String,
    pub kind: ChatAttachmentKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ChatAttachmentKind {
    Image,
    File,
}

/// Comment dragged into a chat composer as inline context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatCommentAttachment {
    pub id: String,
    pub order: u32,
    pub file_path: String,
    pub element_id: String,
    pub selector: String,
    pub label: String,
    pub comment: String,
    pub current_text: String,
    pub page_position: PreviewCommentPosition,
    pub html_hint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_kind: Option<PreviewCommentSelectionKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pod_members: Option<Vec<PreviewCommentMember>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<ChatCommentAttachmentSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ChatCommentAttachmentSource {
    SavedComment,
    BoardBatch,
}

/// Persisted form of an agent SSE payload — what the daemon writes
/// into chat history once a step completes. Distinct from the live
/// SSE `DaemonAgentPayload`: drops `thinking_start` (a transient
/// marker) and flattens token usage onto the variant struct itself.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum PersistedAgentEvent {
    Status {
        label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    Text {
        text: String,
    },
    Thinking {
        text: String,
    },
    #[serde(rename_all = "camelCase")]
    LiveArtifact {
        action: PersistedLiveArtifactAction,
        project_id: String,
        artifact_id: String,
        title: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        refresh_status: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    LiveArtifactRefresh {
        phase: PersistedLiveArtifactRefreshPhase,
        project_id: String,
        artifact_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        refresh_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        refreshed_source_count: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename_all = "camelCase")]
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    /// Token / cost / wall-clock summary. Fields are flattened on this
    /// variant (no nested `usage` object), matching upstream.
    #[serde(rename_all = "camelCase")]
    Usage {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input_tokens: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output_tokens: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cost_usd: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
    Raw {
        line: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum PersistedLiveArtifactAction {
    Created,
    Updated,
    Deleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum PersistedLiveArtifactRefreshPhase {
    Started,
    Succeeded,
    Failed,
}

/// One persisted chat message — what `GET /chat/messages` returns.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: String,
    pub role: ChatRole,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<PersistedAgentEvent>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_status: Option<ChatRunStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run_event_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<ChatAttachment>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment_attachments: Option<Vec<ChatCommentAttachment>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub produced_files: Option<Vec<ProjectFile>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_lowercase() {
        let r: ChatRole = serde_json::from_str("\"user\"").unwrap();
        assert_eq!(r, ChatRole::User);
        assert_eq!(
            serde_json::to_string(&ChatRole::Assistant).unwrap(),
            "\"assistant\""
        );
    }

    #[test]
    fn run_status_lowercase() {
        for (slug, st) in [
            ("queued", ChatRunStatus::Queued),
            ("running", ChatRunStatus::Running),
            ("succeeded", ChatRunStatus::Succeeded),
            ("failed", ChatRunStatus::Failed),
            ("canceled", ChatRunStatus::Canceled),
        ] {
            let q = format!("\"{slug}\"");
            let parsed: ChatRunStatus = serde_json::from_str(&q).unwrap();
            assert_eq!(parsed, st);
        }
    }

    #[test]
    fn chat_request_optional_fields_omitted() {
        let r = ChatRequest {
            agent_id: "a".into(),
            message: "hello".into(),
            ..Default::default()
        };
        let s = serde_json::to_string(&r).unwrap();
        assert_eq!(s, r#"{"agentId":"a","message":"hello"}"#);
    }

    #[test]
    fn chat_request_double_option_null_round_trip() {
        let json = r#"{"agentId":"a","message":"m","projectId":null}"#;
        let r: ChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(r.project_id, Some(None));
        let back = serde_json::to_string(&r).unwrap();
        assert!(back.contains("\"projectId\":null"));
    }

    #[test]
    fn run_create_response_round_trip() {
        let r = ChatRunCreateResponse {
            run_id: "r1".into(),
        };
        assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"runId":"r1"}"#);
    }

    #[test]
    fn run_status_response_camel_case() {
        let r = ChatRunStatusResponse {
            id: "r1".into(),
            project_id: Some("p1".into()),
            conversation_id: None,
            assistant_message_id: None,
            agent_id: None,
            status: ChatRunStatus::Running,
            created_at: 1_700_000_000_000,
            updated_at: 1_700_000_005_000,
            exit_code: None,
            signal: None,
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"projectId\":\"p1\""));
        assert!(s.contains("\"createdAt\":1700000000000"));
        assert!(s.contains("\"conversationId\":null"));
        assert!(!s.contains("exitCode"));
    }

    #[test]
    fn persisted_status_event_round_trip() {
        let e = PersistedAgentEvent::Status {
            label: "thinking".into(),
            detail: None,
        };
        let s = serde_json::to_string(&e).unwrap();
        assert_eq!(s, r#"{"kind":"status","label":"thinking"}"#);
        let back: PersistedAgentEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn persisted_text_thinking_round_trip() {
        let t = PersistedAgentEvent::Text { text: "hi".into() };
        assert_eq!(
            serde_json::to_string(&t).unwrap(),
            r#"{"kind":"text","text":"hi"}"#
        );
        let th = PersistedAgentEvent::Thinking { text: "uh".into() };
        assert_eq!(
            serde_json::to_string(&th).unwrap(),
            r#"{"kind":"thinking","text":"uh"}"#
        );
    }

    #[test]
    fn persisted_live_artifact_camel_case_payload() {
        let e = PersistedAgentEvent::LiveArtifact {
            action: PersistedLiveArtifactAction::Updated,
            project_id: "p1".into(),
            artifact_id: "a1".into(),
            title: "Hero".into(),
            refresh_status: Some("ok".into()),
        };
        let s = serde_json::to_string(&e).unwrap();
        assert!(s.contains("\"kind\":\"live_artifact\""));
        assert!(s.contains("\"action\":\"updated\""));
        assert!(s.contains("\"projectId\":\"p1\""));
        assert!(s.contains("\"refreshStatus\":\"ok\""));
    }

    #[test]
    fn persisted_tool_use_result_round_trip() {
        let u = PersistedAgentEvent::ToolUse {
            id: "t1".into(),
            name: "Read".into(),
            input: serde_json::json!({"path": "x"}),
        };
        let s = serde_json::to_string(&u).unwrap();
        assert!(s.contains("\"kind\":\"tool_use\""));
        let r = PersistedAgentEvent::ToolResult {
            tool_use_id: "t1".into(),
            content: "data".into(),
            is_error: false,
        };
        let s2 = serde_json::to_string(&r).unwrap();
        // Persisted variant uses camelCase due to per-variant rename
        assert!(s2.contains("\"toolUseId\":\"t1\""));
        assert!(s2.contains("\"isError\":false"));
    }

    #[test]
    fn persisted_usage_flattened_fields() {
        let u = PersistedAgentEvent::Usage {
            input_tokens: Some(120),
            output_tokens: Some(30),
            cost_usd: Some(0.0042),
            duration_ms: Some(1200),
        };
        let s = serde_json::to_string(&u).unwrap();
        assert!(s.contains("\"kind\":\"usage\""));
        assert!(s.contains("\"inputTokens\":120"));
        assert!(s.contains("\"costUsd\":0.0042"));
        assert!(s.contains("\"durationMs\":1200"));
    }

    #[test]
    fn comment_attachment_kebab_source_round_trip() {
        let a = ChatCommentAttachment {
            id: "1".into(),
            order: 0,
            file_path: "i.html".into(),
            element_id: "e".into(),
            selector: ".x".into(),
            label: "X".into(),
            comment: "fix".into(),
            current_text: "before".into(),
            page_position: PreviewCommentPosition {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            html_hint: "<x>".into(),
            selection_kind: None,
            member_count: None,
            pod_members: None,
            source: Some(ChatCommentAttachmentSource::BoardBatch),
        };
        let s = serde_json::to_string(&a).unwrap();
        assert!(s.contains("\"source\":\"board-batch\""));
    }

    #[test]
    fn chat_message_minimal_round_trip() {
        let m = ChatMessage {
            id: "m1".into(),
            role: ChatRole::User,
            content: "hi".into(),
            agent_id: None,
            agent_name: None,
            events: None,
            created_at: None,
            run_id: None,
            run_status: None,
            last_run_event_id: None,
            started_at: None,
            ended_at: None,
            attachments: None,
            comment_attachments: None,
            produced_files: None,
        };
        let s = serde_json::to_string(&m).unwrap();
        assert_eq!(s, r#"{"id":"m1","role":"user","content":"hi"}"#);
    }
}
