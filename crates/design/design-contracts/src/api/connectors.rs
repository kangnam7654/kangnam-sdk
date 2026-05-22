//! `/api/connectors` — MCP-style connector catalog, status, and tool
//! execution. Mirrors `@open-design/contracts/src/api/connectors.ts`.

use serde::{Deserialize, Serialize};

use crate::api::live_artifacts::{BoundedJsonObject, BoundedJsonValue};
// `crate::locked_true!` is re-exported via #[macro_export]; no use needed.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ConnectorStatus {
    Available,
    Connected,
    Error,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ConnectorToolSideEffect {
    Read,
    Write,
    Destructive,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ConnectorToolApproval {
    Auto,
    Confirm,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorToolSafety {
    pub side_effect: ConnectorToolSideEffect,
    pub approval: ConnectorToolApproval,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorToolDetail {
    pub name: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_schema_json: Option<BoundedJsonObject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema_json: Option<BoundedJsonObject>,
    pub safety: ConnectorToolSafety,
    pub refresh_eligible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ConnectorAuthProvider {
    Local,
    None,
    Oauth,
    Composio,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectorAuthDetail {
    pub provider: ConnectorAuthProvider,
    pub configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorDetail {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub category: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: ConnectorStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_label: Option<String>,
    pub tools: Vec<ConnectorToolDetail>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub featured_tool_names: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_approval: Option<ConnectorToolApproval>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<ConnectorAuthDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectorListResponse {
    pub connectors: Vec<ConnectorDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorStatusSummary {
    pub status: ConnectorStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectorStatusResponse {
    pub statuses: std::collections::HashMap<String, ConnectorStatusSummary>,
}

/// `'composio'` literal upstream — locked single-variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ConnectorDiscoveryProvider {
    Composio,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorDiscoveryMeta {
    pub provider: ConnectorDiscoveryProvider,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_requested: Option<bool>,
}

/// `extends ConnectorListResponse` upstream — adds an optional meta block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectorDiscoveryResponse {
    pub connectors: Vec<ConnectorDetail>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<ConnectorDiscoveryMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectorDetailResponse {
    pub connector: ConnectorDetail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ConnectorConnectAuthKind {
    RedirectRequired,
    Pending,
    Connected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorConnectAuth {
    pub kind: ConnectorConnectAuthKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorConnectResponse {
    pub connector: ConnectorDetail,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<ConnectorConnectAuth>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorExecuteRequest {
    pub connector_id: String,
    pub tool_name: String,
    pub input: BoundedJsonObject,
}

crate::locked_true!(
    /// `ok: true` literal upstream — the response envelope carries
    /// extra fields beyond the `ok` flag, so the `ok` field itself uses
    /// this locked-true marker (vs the standalone [`crate::common::OkResponse`]
    /// envelope used when `{ ok: true }` is the entire response body).
    pub struct ConnectorExecuteOk;,
    field_name = "ok"
);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorExecuteResponse {
    pub ok: ConnectorExecuteOk,
    pub connector_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_label: Option<String>,
    pub tool_name: String,
    pub safety: ConnectorToolSafety,
    pub output: BoundedJsonValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_execution_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BoundedJsonObject>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_lowercase() {
        assert_eq!(
            serde_json::to_string(&ConnectorStatus::Connected).unwrap(),
            "\"connected\""
        );
        assert_eq!(
            serde_json::to_string(&ConnectorStatus::Disabled).unwrap(),
            "\"disabled\""
        );
    }

    #[test]
    fn side_effect_lowercase() {
        let s: ConnectorToolSideEffect = serde_json::from_str("\"destructive\"").unwrap();
        assert_eq!(s, ConnectorToolSideEffect::Destructive);
    }

    #[test]
    fn auth_provider_lowercase() {
        // 'none' must still parse (different from a missing field).
        let p: ConnectorAuthProvider = serde_json::from_str("\"none\"").unwrap();
        assert_eq!(p, ConnectorAuthProvider::None);
        assert_eq!(
            serde_json::to_string(&ConnectorAuthProvider::Oauth).unwrap(),
            "\"oauth\""
        );
    }

    #[test]
    fn tool_safety_camel_case() {
        let s = ConnectorToolSafety {
            side_effect: ConnectorToolSideEffect::Read,
            approval: ConnectorToolApproval::Auto,
            reason: "read-only".into(),
        };
        let j = serde_json::to_string(&s).unwrap();
        assert!(j.contains("\"sideEffect\":\"read\""));
        assert!(j.contains("\"approval\":\"auto\""));
    }

    #[test]
    fn detail_round_trip_with_nested_tools() {
        let d = ConnectorDetail {
            id: "c1".into(),
            name: "Slack".into(),
            provider: "slack".into(),
            category: "messaging".into(),
            description: None,
            status: ConnectorStatus::Connected,
            account_label: Some("acme.slack.com".into()),
            tools: vec![ConnectorToolDetail {
                name: "send_message".into(),
                title: "Send a message".into(),
                description: None,
                input_schema_json: None,
                output_schema_json: None,
                safety: ConnectorToolSafety {
                    side_effect: ConnectorToolSideEffect::Write,
                    approval: ConnectorToolApproval::Confirm,
                    reason: "writes to channel".into(),
                },
                refresh_eligible: false,
            }],
            featured_tool_names: Some(vec!["send_message".into()]),
            minimum_approval: Some(ConnectorToolApproval::Confirm),
            last_error: None,
            auth: Some(ConnectorAuthDetail {
                provider: ConnectorAuthProvider::Oauth,
                configured: true,
            }),
        };
        let s = serde_json::to_string(&d).unwrap();
        assert!(s.contains("\"accountLabel\":\"acme.slack.com\""));
        assert!(s.contains("\"refreshEligible\":false"));
        assert!(s.contains("\"featuredToolNames\":[\"send_message\"]"));
        assert!(s.contains("\"minimumApproval\":\"confirm\""));
        let back: ConnectorDetail = serde_json::from_str(&s).unwrap();
        assert_eq!(back.id, "c1");
        assert_eq!(back.tools.len(), 1);
    }

    #[test]
    fn discovery_response_meta_optional() {
        let r = ConnectorDiscoveryResponse {
            connectors: vec![],
            meta: Some(ConnectorDiscoveryMeta {
                provider: ConnectorDiscoveryProvider::Composio,
                refresh_requested: Some(true),
            }),
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"provider\":\"composio\""));
        assert!(s.contains("\"refreshRequested\":true"));
    }

    #[test]
    fn connect_response_with_redirect_auth() {
        let r = ConnectorConnectResponse {
            connector: ConnectorDetail {
                id: "c1".into(),
                name: "n".into(),
                provider: "p".into(),
                category: "c".into(),
                description: None,
                status: ConnectorStatus::Available,
                account_label: None,
                tools: vec![],
                featured_tool_names: None,
                minimum_approval: None,
                last_error: None,
                auth: None,
            },
            auth: Some(ConnectorConnectAuth {
                kind: ConnectorConnectAuthKind::RedirectRequired,
                redirect_url: Some("https://oauth.example.invalid/cb".into()),
                provider_connection_id: None,
                expires_at: None,
            }),
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"kind\":\"redirect_required\""));
        assert!(s.contains("\"redirectUrl\":"));
    }

    #[test]
    fn execute_request_round_trip() {
        let mut input = BoundedJsonObject::new();
        input.insert("channel".into(), serde_json::json!("#general"));
        let r = ConnectorExecuteRequest {
            connector_id: "c1".into(),
            tool_name: "send_message".into(),
            input,
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"connectorId\":\"c1\""));
        assert!(s.contains("\"toolName\":\"send_message\""));
        assert!(s.contains("\"channel\":\"#general\""));
    }

    #[test]
    fn execute_response_ok_locked_to_true() {
        let resp = ConnectorExecuteResponse {
            ok: ConnectorExecuteOk,
            connector_id: "c1".into(),
            account_label: None,
            tool_name: "ping".into(),
            safety: ConnectorToolSafety {
                side_effect: ConnectorToolSideEffect::Read,
                approval: ConnectorToolApproval::Auto,
                reason: "ok".into(),
            },
            output: serde_json::json!({"pong": true}),
            output_summary: None,
            provider_execution_id: None,
            metadata: None,
        };
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"ok\":true"));
        // false fails to deserialize.
        let bad = s.replace("\"ok\":true", "\"ok\":false");
        let err = serde_json::from_str::<ConnectorExecuteResponse>(&bad).unwrap_err();
        assert!(err.to_string().contains("ok must be true"));
    }

    #[test]
    fn status_response_uses_string_keyed_map() {
        let mut map = std::collections::HashMap::new();
        map.insert(
            "c1".to_string(),
            ConnectorStatusSummary {
                status: ConnectorStatus::Connected,
                account_label: Some("alice".into()),
                last_error: None,
            },
        );
        let r = ConnectorStatusResponse { statuses: map };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"c1\""));
        assert!(s.contains("\"status\":\"connected\""));
        assert!(s.contains("\"accountLabel\":\"alice\""));
    }

    #[test]
    fn connect_auth_kind_snake_case() {
        for (slug, kind) in [
            (
                "redirect_required",
                ConnectorConnectAuthKind::RedirectRequired,
            ),
            ("pending", ConnectorConnectAuthKind::Pending),
            ("connected", ConnectorConnectAuthKind::Connected),
        ] {
            let q = format!("\"{slug}\"");
            let parsed: ConnectorConnectAuthKind = serde_json::from_str(&q).unwrap();
            assert_eq!(parsed, kind);
        }
    }
}
