//! `/api/proxy` — model-proxy stream contract (used by the web client to
//! talk to OpenAI-compatible APIs through the daemon). Mirrors
//! `@open-design/contracts/src/api/proxy.ts`.
//!
//! These are also the SSE proxy event payloads referenced by
//! [`crate::sse::ProxySseEvent`] — the umbrella `start` / `delta` / `end`
//! events carry the matching payloads via `data`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ProxyMessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProxyMessage {
    pub role: ProxyMessageRole,
    pub content: String,
}

/// POST body to `/api/proxy/stream`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProxyStreamRequest {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    pub messages: Vec<ProxyMessage>,
    /// Caps the upstream completion length. Daemon defaults to 8192
    /// when unset to preserve pre-existing client behavior.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Azure OpenAI only. Daemon picks a default when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
}

/// SSE `start` payload — emitted once at the head of the stream.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProxyStreamStartPayload {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// SSE `delta` payload — one token chunk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProxyStreamDeltaPayload {
    pub delta: String,
}

/// SSE `end` payload — emitted once at the tail. `code` mirrors HTTP
/// status from the upstream provider when available.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProxyStreamEndPayload {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_lowercase_serde() {
        let r: ProxyMessageRole = serde_json::from_str("\"assistant\"").unwrap();
        assert_eq!(r, ProxyMessageRole::Assistant);
        let s = serde_json::to_string(&ProxyMessageRole::Tool).unwrap();
        assert_eq!(s, "\"tool\"");
    }

    #[test]
    fn proxy_stream_request_camel_case() {
        let r = ProxyStreamRequest {
            base_url: "https://api.example.invalid/v1".into(),
            api_key: "sk-123".into(),
            model: "gpt-4".into(),
            system_prompt: None,
            messages: vec![ProxyMessage {
                role: ProxyMessageRole::User,
                content: "hi".into(),
            }],
            max_tokens: Some(2048),
            api_version: None,
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"baseUrl\""));
        assert!(s.contains("\"apiKey\":\"sk-123\""));
        assert!(s.contains("\"maxTokens\":2048"));
        assert!(!s.contains("apiVersion"));
        assert!(!s.contains("systemPrompt"));
        let back: ProxyStreamRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn delta_and_end_payloads_round_trip() {
        let d = ProxyStreamDeltaPayload {
            delta: "hello".into(),
        };
        assert_eq!(serde_json::to_string(&d).unwrap(), r#"{"delta":"hello"}"#);
        let e = ProxyStreamEndPayload { code: Some(200) };
        assert_eq!(serde_json::to_string(&e).unwrap(), r#"{"code":200}"#);
        let e2 = ProxyStreamEndPayload::default();
        assert_eq!(serde_json::to_string(&e2).unwrap(), r#"{}"#);
    }
}
