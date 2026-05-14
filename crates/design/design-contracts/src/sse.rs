//! Server-Sent Events transport types. Mirrors
//! `@open-design/contracts/src/sse/{common,chat,proxy}.ts`.
//!
//! Wire format
//! - `id?`, `event`, `data` — these are the three SSE fields the upstream
//!   web client reads. `data` is JSON; the rest is text.
//! - The chat event family discriminates `data.type` for agent payloads
//!   (text_delta / thinking_delta / tool_use / live_artifact / …) so the
//!   SSE event name (`agent`) is stable while the payload shape evolves.

use serde::{Deserialize, Serialize};

use crate::errors::SseErrorPayload;

/// Generic SSE transport envelope. Mirrors `SseTransportEvent<Name, P>`.
///
/// `event` is the SSE event name (e.g. `"start"`, `"agent"`, `"end"`).
/// `data` is the typed payload. `id` is optional (web client uses it
/// for `Last-Event-ID` resumption when the server provides one).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SseEvent<P> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub event: String,
    pub data: P,
}

// ─── Live-artifact SSE payloads ─────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum LiveArtifactSseAction {
    Created,
    Updated,
    Deleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum LiveArtifactRefreshSsePhase {
    Started,
    Succeeded,
    Failed,
}

/// `type: "live_artifact"` agent SSE payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveArtifactSsePayload {
    pub action: LiveArtifactSseAction,
    pub project_id: String,
    pub artifact_id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_status: Option<String>,
}

/// `type: "live_artifact_refresh"` agent SSE payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveArtifactRefreshSsePayload {
    pub phase: LiveArtifactRefreshSsePhase,
    pub project_id: String,
    pub artifact_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refreshed_source_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ─── DaemonAgentPayload (the `agent` SSE event payload) ─────────────────

/// Discriminated agent SSE payload. The `type` tag identifies which
/// concrete shape follows. Forward-compat: `#[non_exhaustive]` plus a
/// `Raw { line }` catch-all that captures unknown future variants
/// without losing data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum DaemonAgentPayload {
    /// Coarse agent status update — what the agent is doing right now.
    #[serde(rename_all = "camelCase")]
    Status {
        label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ttft_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// Streaming user-facing text token.
    TextDelta {
        delta: String,
    },
    /// Streaming thinking-mode token (visible only when the user enabled
    /// extended thinking).
    ThinkingDelta {
        delta: String,
    },
    /// Marker — thinking block is starting; subsequent ThinkingDelta
    /// events belong to it until the next Status / TextDelta.
    ThinkingStart,
    LiveArtifact(LiveArtifactSsePayload),
    LiveArtifactRefresh(LiveArtifactRefreshSsePayload),
    /// Tool call started.
    ToolUse(ToolUsePayload),
    /// Tool call result — links back to the originating ToolUse via
    /// `tool_use_id`.
    ToolResult(ToolResultPayload),
    /// Token / cost / wall-clock summary at the end of an agent step.
    Usage(UsagePayload),
    /// Catch-all line the daemon couldn't classify (raw stdout that
    /// didn't parse as a known event). Preserves wire bytes verbatim.
    Raw {
        line: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUsePayload {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResultPayload {
    pub tool_use_id: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsagePayload {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TokenUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
}

// ─── Chat-channel SSE event payloads ────────────────────────────────────

pub const CHAT_SSE_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSseStartPayload {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    pub bin: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<u32>,
    /// Legacy daemon-internal absolute cwd. Kept for compatibility
    /// during W2 adoption.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSseChunkPayload {
    pub chunk: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatSseEndStatus {
    Succeeded,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSseEndPayload {
    pub code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ChatSseEndStatus>,
}

/// Discriminated chat-channel SSE event. Each variant carries the
/// matching payload for the upstream `event:` SSE field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data", rename_all = "lowercase")]
pub enum ChatSseEvent {
    Start(ChatSseStartPayload),
    Agent(DaemonAgentPayload),
    Stdout(ChatSseChunkPayload),
    Stderr(ChatSseChunkPayload),
    Error(SseErrorPayload),
    End(ChatSseEndPayload),
}

// ─── Proxy-channel SSE event payloads ───────────────────────────────────

use crate::api::proxy::{ProxyStreamDeltaPayload, ProxyStreamEndPayload, ProxyStreamStartPayload};

pub const PROXY_SSE_PROTOCOL_VERSION: u32 = 1;

/// Discriminated proxy-channel SSE event. Each variant carries the
/// matching typed payload from [`crate::api::proxy`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data", rename_all = "lowercase")]
pub enum ProxySseEvent {
    Start(ProxyStreamStartPayload),
    Delta(ProxyStreamDeltaPayload),
    Error(SseErrorPayload),
    End(ProxyStreamEndPayload),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_artifact_action_lowercase() {
        let a: LiveArtifactSseAction = serde_json::from_str("\"created\"").unwrap();
        assert_eq!(a, LiveArtifactSseAction::Created);
        let s = serde_json::to_string(&LiveArtifactSseAction::Deleted).unwrap();
        assert_eq!(s, "\"deleted\"");
    }

    #[test]
    fn agent_payload_text_delta_round_trip() {
        let p = DaemonAgentPayload::TextDelta { delta: "hi".into() };
        let s = serde_json::to_string(&p).unwrap();
        assert_eq!(s, r#"{"type":"text_delta","delta":"hi"}"#);
        let back: DaemonAgentPayload = serde_json::from_str(&s).unwrap();
        match back {
            DaemonAgentPayload::TextDelta { delta } => assert_eq!(delta, "hi"),
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn agent_payload_status_camel_case_optional_fields() {
        let p = DaemonAgentPayload::Status {
            label: "rendering".into(),
            model: Some("claude-opus-4-7".into()),
            ttft_ms: Some(840),
            detail: None,
        };
        let s = serde_json::to_string(&p).unwrap();
        assert!(s.contains("\"type\":\"status\""));
        assert!(s.contains("\"ttftMs\":840"));
        assert!(!s.contains("detail"));
    }

    #[test]
    fn agent_payload_thinking_start_unit_variant() {
        let p = DaemonAgentPayload::ThinkingStart;
        let s = serde_json::to_string(&p).unwrap();
        assert_eq!(s, r#"{"type":"thinking_start"}"#);
    }

    #[test]
    fn agent_payload_tool_use_camel_case() {
        let p = DaemonAgentPayload::ToolUse(ToolUsePayload {
            id: "t1".into(),
            name: "Read".into(),
            input: serde_json::json!({"path": "/tmp/x"}),
        });
        let s = serde_json::to_string(&p).unwrap();
        // tag + flattened payload
        assert!(s.contains("\"type\":\"tool_use\""));
        assert!(s.contains("\"name\":\"Read\""));
    }

    #[test]
    fn agent_payload_tool_result_with_is_error() {
        let p = DaemonAgentPayload::ToolResult(ToolResultPayload {
            tool_use_id: "t1".into(),
            content: "denied".into(),
            is_error: Some(true),
        });
        let s = serde_json::to_string(&p).unwrap();
        assert!(s.contains("\"toolUseId\":\"t1\""));
        assert!(s.contains("\"isError\":true"));
    }

    #[test]
    fn agent_payload_live_artifact_round_trip() {
        let p = DaemonAgentPayload::LiveArtifact(LiveArtifactSsePayload {
            action: LiveArtifactSseAction::Updated,
            project_id: "p1".into(),
            artifact_id: "a1".into(),
            title: "Hero".into(),
            refresh_status: None,
        });
        let s = serde_json::to_string(&p).unwrap();
        assert!(s.contains("\"type\":\"live_artifact\""));
        assert!(s.contains("\"action\":\"updated\""));
        assert!(s.contains("\"projectId\":\"p1\""));
        let back: DaemonAgentPayload = serde_json::from_str(&s).unwrap();
        matches!(back, DaemonAgentPayload::LiveArtifact(_));
    }

    #[test]
    fn agent_payload_raw_catchall() {
        let p = DaemonAgentPayload::Raw {
            line: "??? unknown".into(),
        };
        let s = serde_json::to_string(&p).unwrap();
        assert_eq!(s, r#"{"type":"raw","line":"??? unknown"}"#);
    }

    #[test]
    fn chat_sse_event_round_trip_text_delta_inside_agent() {
        let evt = ChatSseEvent::Agent(DaemonAgentPayload::TextDelta { delta: "h".into() });
        let s = serde_json::to_string(&evt).unwrap();
        // event: "agent", data: { type: "text_delta", delta: "h" }
        assert!(s.contains("\"event\":\"agent\""));
        assert!(s.contains("\"data\":{\"type\":\"text_delta\",\"delta\":\"h\"}"));
        let back: ChatSseEvent = serde_json::from_str(&s).unwrap();
        match back {
            ChatSseEvent::Agent(DaemonAgentPayload::TextDelta { delta }) => {
                assert_eq!(delta, "h");
            }
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn chat_sse_event_end_with_status() {
        let evt = ChatSseEvent::End(ChatSseEndPayload {
            code: Some(0),
            signal: None,
            status: Some(ChatSseEndStatus::Succeeded),
        });
        let s = serde_json::to_string(&evt).unwrap();
        assert!(s.contains("\"event\":\"end\""));
        assert!(s.contains("\"status\":\"succeeded\""));
        assert!(!s.contains("signal"));
    }

    #[test]
    fn chat_sse_event_error_uses_shared_payload() {
        let evt = ChatSseEvent::Error(SseErrorPayload {
            message: "boom".into(),
            error: None,
        });
        let s = serde_json::to_string(&evt).unwrap();
        assert!(s.contains("\"event\":\"error\""));
        assert!(s.contains("\"message\":\"boom\""));
    }

    #[test]
    fn proxy_sse_event_round_trip_typed_payload() {
        let evt = ProxySseEvent::Start(ProxyStreamStartPayload {
            model: Some("gpt-4".into()),
        });
        let s = serde_json::to_string(&evt).unwrap();
        assert!(s.contains("\"event\":\"start\""));
        assert!(s.contains("\"model\":\"gpt-4\""));
        let back: ProxySseEvent = serde_json::from_str(&s).unwrap();
        match back {
            ProxySseEvent::Start(p) => assert_eq!(p.model.as_deref(), Some("gpt-4")),
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn proxy_sse_event_delta_and_end_round_trip() {
        let d = ProxySseEvent::Delta(ProxyStreamDeltaPayload {
            delta: "tok".into(),
        });
        let s = serde_json::to_string(&d).unwrap();
        assert!(s.contains("\"event\":\"delta\""));
        assert!(s.contains("\"delta\":\"tok\""));
        let e = ProxySseEvent::End(ProxyStreamEndPayload { code: Some(200) });
        let s2 = serde_json::to_string(&e).unwrap();
        assert!(s2.contains("\"event\":\"end\""));
        assert!(s2.contains("\"code\":200"));
    }

    #[test]
    fn protocol_version_constants_are_one() {
        assert_eq!(CHAT_SSE_PROTOCOL_VERSION, 1);
        assert_eq!(PROXY_SSE_PROTOCOL_VERSION, 1);
    }

    #[test]
    fn sse_envelope_omits_optional_id() {
        let e = SseEvent {
            id: None,
            event: "ping".into(),
            data: serde_json::json!({"k": 1}),
        };
        let s = serde_json::to_string(&e).unwrap();
        assert!(!s.contains("\"id\""));
        assert!(s.contains("\"event\":\"ping\""));
    }

    #[test]
    fn token_usage_skips_none() {
        let u = TokenUsage {
            input_tokens: Some(120),
            output_tokens: None,
        };
        let s = serde_json::to_string(&u).unwrap();
        assert_eq!(s, r#"{"input_tokens":120}"#);
    }
}
