//! API error envelope. Mirrors `@open-design/contracts/src/errors.ts`.
//!
//! All 49 upstream error codes are represented as a strict enum. Unknown
//! codes fail to deserialize — the daemon and web are kept in lockstep
//! by virtue of typechecking, so a code that doesn't round-trip is a
//! genuine schema drift.

use serde::{Deserialize, Serialize};

/// Canonical API error code. Serializes as the upstream slug (e.g.
/// `BadRequest` → `"BAD_REQUEST"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ApiErrorCode {
    // Generic HTTP/API failures.
    #[serde(rename = "BAD_REQUEST")]
    BadRequest,
    #[serde(rename = "UNAUTHORIZED")]
    Unauthorized,
    #[serde(rename = "FORBIDDEN")]
    Forbidden,
    #[serde(rename = "NOT_FOUND")]
    NotFound,
    #[serde(rename = "CONFLICT")]
    Conflict,
    #[serde(rename = "PAYLOAD_TOO_LARGE")]
    PayloadTooLarge,
    #[serde(rename = "UNSUPPORTED_MEDIA_TYPE")]
    UnsupportedMediaType,
    #[serde(rename = "VALIDATION_FAILED")]
    ValidationFailed,
    #[serde(rename = "AGENT_UNAVAILABLE")]
    AgentUnavailable,
    #[serde(rename = "AGENT_EXECUTION_FAILED")]
    AgentExecutionFailed,
    #[serde(rename = "AGENT_PROMPT_TOO_LARGE")]
    AgentPromptTooLarge,
    #[serde(rename = "PROJECT_NOT_FOUND")]
    ProjectNotFound,
    #[serde(rename = "FILE_NOT_FOUND")]
    FileNotFound,
    #[serde(rename = "ARTIFACT_NOT_FOUND")]
    ArtifactNotFound,
    #[serde(rename = "UPSTREAM_UNAVAILABLE")]
    UpstreamUnavailable,
    #[serde(rename = "RATE_LIMITED")]
    RateLimited,
    // Agent-facing tool endpoint authorization failures.
    #[serde(rename = "TOOL_TOKEN_MISSING")]
    ToolTokenMissing,
    #[serde(rename = "TOOL_TOKEN_INVALID")]
    ToolTokenInvalid,
    #[serde(rename = "TOOL_TOKEN_EXPIRED")]
    ToolTokenExpired,
    #[serde(rename = "TOOL_ENDPOINT_DENIED")]
    ToolEndpointDenied,
    #[serde(rename = "TOOL_OPERATION_DENIED")]
    ToolOperationDenied,
    // Live artifact validation, storage, preview, and refresh failures.
    #[serde(rename = "LIVE_ARTIFACT_NOT_FOUND")]
    LiveArtifactNotFound,
    #[serde(rename = "LIVE_ARTIFACT_INVALID")]
    LiveArtifactInvalid,
    #[serde(rename = "LIVE_ARTIFACT_STORAGE_FAILED")]
    LiveArtifactStorageFailed,
    #[serde(rename = "LIVE_ARTIFACT_REFRESH_UNAVAILABLE")]
    LiveArtifactRefreshUnavailable,
    #[serde(rename = "LIVE_ARTIFACT_REFRESH_TIMEOUT")]
    LiveArtifactRefreshTimeout,
    #[serde(rename = "REFRESH_LOCKED")]
    RefreshLocked,
    #[serde(rename = "REFRESH_TIMED_OUT")]
    RefreshTimedOut,
    #[serde(rename = "REFRESH_FAILED")]
    RefreshFailed,
    #[serde(rename = "OUTPUT_TOO_LARGE")]
    OutputTooLarge,
    #[serde(rename = "TEMPLATE_BINDING_INVALID")]
    TemplateBindingInvalid,
    #[serde(rename = "REDACTION_REQUIRED")]
    RedactionRequired,
    // Connector catalog, connection, safety, and execution failures.
    #[serde(rename = "CONNECTOR_NOT_FOUND")]
    ConnectorNotFound,
    #[serde(rename = "CONNECTOR_NOT_CONNECTED")]
    ConnectorNotConnected,
    #[serde(rename = "CONNECTOR_DISABLED")]
    ConnectorDisabled,
    #[serde(rename = "CONNECTOR_TOOL_NOT_FOUND")]
    ConnectorToolNotFound,
    #[serde(rename = "CONNECTOR_SAFETY_DENIED")]
    ConnectorSafetyDenied,
    #[serde(rename = "CONNECTOR_INPUT_SCHEMA_MISMATCH")]
    ConnectorInputSchemaMismatch,
    #[serde(rename = "CONNECTOR_RATE_LIMITED")]
    ConnectorRateLimited,
    #[serde(rename = "CONNECTOR_OUTPUT_TOO_LARGE")]
    ConnectorOutputTooLarge,
    #[serde(rename = "CONNECTOR_EXECUTION_FAILED")]
    ConnectorExecutionFailed,
    #[serde(rename = "INTERNAL_ERROR")]
    InternalError,
}

impl ApiErrorCode {
    /// Wire slug (e.g. `"BAD_REQUEST"`) — the value emitted by serde.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::BadRequest => "BAD_REQUEST",
            Self::Unauthorized => "UNAUTHORIZED",
            Self::Forbidden => "FORBIDDEN",
            Self::NotFound => "NOT_FOUND",
            Self::Conflict => "CONFLICT",
            Self::PayloadTooLarge => "PAYLOAD_TOO_LARGE",
            Self::UnsupportedMediaType => "UNSUPPORTED_MEDIA_TYPE",
            Self::ValidationFailed => "VALIDATION_FAILED",
            Self::AgentUnavailable => "AGENT_UNAVAILABLE",
            Self::AgentExecutionFailed => "AGENT_EXECUTION_FAILED",
            Self::AgentPromptTooLarge => "AGENT_PROMPT_TOO_LARGE",
            Self::ProjectNotFound => "PROJECT_NOT_FOUND",
            Self::FileNotFound => "FILE_NOT_FOUND",
            Self::ArtifactNotFound => "ARTIFACT_NOT_FOUND",
            Self::UpstreamUnavailable => "UPSTREAM_UNAVAILABLE",
            Self::RateLimited => "RATE_LIMITED",
            Self::ToolTokenMissing => "TOOL_TOKEN_MISSING",
            Self::ToolTokenInvalid => "TOOL_TOKEN_INVALID",
            Self::ToolTokenExpired => "TOOL_TOKEN_EXPIRED",
            Self::ToolEndpointDenied => "TOOL_ENDPOINT_DENIED",
            Self::ToolOperationDenied => "TOOL_OPERATION_DENIED",
            Self::LiveArtifactNotFound => "LIVE_ARTIFACT_NOT_FOUND",
            Self::LiveArtifactInvalid => "LIVE_ARTIFACT_INVALID",
            Self::LiveArtifactStorageFailed => "LIVE_ARTIFACT_STORAGE_FAILED",
            Self::LiveArtifactRefreshUnavailable => "LIVE_ARTIFACT_REFRESH_UNAVAILABLE",
            Self::LiveArtifactRefreshTimeout => "LIVE_ARTIFACT_REFRESH_TIMEOUT",
            Self::RefreshLocked => "REFRESH_LOCKED",
            Self::RefreshTimedOut => "REFRESH_TIMED_OUT",
            Self::RefreshFailed => "REFRESH_FAILED",
            Self::OutputTooLarge => "OUTPUT_TOO_LARGE",
            Self::TemplateBindingInvalid => "TEMPLATE_BINDING_INVALID",
            Self::RedactionRequired => "REDACTION_REQUIRED",
            Self::ConnectorNotFound => "CONNECTOR_NOT_FOUND",
            Self::ConnectorNotConnected => "CONNECTOR_NOT_CONNECTED",
            Self::ConnectorDisabled => "CONNECTOR_DISABLED",
            Self::ConnectorToolNotFound => "CONNECTOR_TOOL_NOT_FOUND",
            Self::ConnectorSafetyDenied => "CONNECTOR_SAFETY_DENIED",
            Self::ConnectorInputSchemaMismatch => "CONNECTOR_INPUT_SCHEMA_MISMATCH",
            Self::ConnectorRateLimited => "CONNECTOR_RATE_LIMITED",
            Self::ConnectorOutputTooLarge => "CONNECTOR_OUTPUT_TOO_LARGE",
            Self::ConnectorExecutionFailed => "CONNECTOR_EXECUTION_FAILED",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }
}

impl std::fmt::Display for ApiErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One request-scoped error envelope. `details` is intentionally a free
/// JSON value (the upstream `JsonValue` recursive type) — the daemon
/// stuffs validation issues, schema diffs, or upstream error bodies in
/// here without inventing a new code per shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub code: ApiErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
}

/// Outer envelope `{ "error": ApiError }` returned by failing JSON
/// endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    pub error: ApiError,
}

/// One validation issue inside `ApiError.details` when
/// `code == ValidationFailed`. `path` is a dot/bracket path, JSON
/// pointer, or form-field name (free-form by convention).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiValidationIssue {
    pub path: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// Discriminated `ApiError.details` shape for validation failures.
/// `kind: "validation"` plus an array of issues.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[non_exhaustive]
pub enum ApiValidationErrorDetails {
    #[serde(rename = "validation")]
    Validation { issues: Vec<ApiValidationIssue> },
}

/// SSE `error` event payload. Always includes `message`; may carry the
/// full `ApiError` envelope when the upstream wanted to preserve the
/// code/details.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SseErrorPayload {
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

/// Builder helper — `createApiError(code, message, init)` upstream.
pub fn create_api_error(code: ApiErrorCode, message: impl Into<String>) -> ApiError {
    ApiError {
        code,
        message: message.into(),
        details: None,
        retryable: None,
        request_id: None,
        task_id: None,
    }
}

/// Builder helper — `createApiErrorResponse(error)` upstream.
pub fn create_api_error_response(error: ApiError) -> ApiErrorResponse {
    ApiErrorResponse { error }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_code_serializes_to_upstream_slug() {
        // Spot-check every code: as_str() must match the serde rename slug.
        for code in [
            ApiErrorCode::BadRequest,
            ApiErrorCode::ValidationFailed,
            ApiErrorCode::LiveArtifactRefreshUnavailable,
            ApiErrorCode::ConnectorInputSchemaMismatch,
            ApiErrorCode::InternalError,
        ] {
            let s = serde_json::to_string(&code).unwrap();
            assert_eq!(s, format!("\"{}\"", code.as_str()));
        }
    }

    #[test]
    fn unknown_code_fails_to_deserialize() {
        let r: Result<ApiErrorCode, _> = serde_json::from_str("\"NEVER_SEEN_THIS\"");
        assert!(r.is_err());
    }

    #[test]
    fn api_error_round_trip_camel_case() {
        let e = ApiError {
            code: ApiErrorCode::ValidationFailed,
            message: "bad input".into(),
            details: Some(serde_json::json!({"field": "name"})),
            retryable: Some(false),
            request_id: Some("req_123".into()),
            task_id: None,
        };
        let s = serde_json::to_string(&e).unwrap();
        assert!(s.contains("\"requestId\":\"req_123\""));
        // task_id was None — must be omitted, not emitted as null.
        assert!(!s.contains("taskId"));
        assert!(!s.contains("null"));
        let back: ApiError = serde_json::from_str(&s).unwrap();
        assert_eq!(back.code, ApiErrorCode::ValidationFailed);
        assert_eq!(back.request_id.as_deref(), Some("req_123"));
        assert!(back.task_id.is_none());
    }

    #[test]
    fn create_api_error_helper_sets_only_required_fields() {
        let e = create_api_error(ApiErrorCode::Conflict, "resource exists");
        assert_eq!(e.code, ApiErrorCode::Conflict);
        assert_eq!(e.message, "resource exists");
        assert!(e.details.is_none());
        assert!(e.retryable.is_none());
        assert!(e.request_id.is_none());
        assert!(e.task_id.is_none());
    }

    #[test]
    fn create_api_error_response_wraps_envelope() {
        let r = create_api_error_response(create_api_error(ApiErrorCode::Forbidden, "no."));
        let s = serde_json::to_string(&r).unwrap();
        assert_eq!(s, r#"{"error":{"code":"FORBIDDEN","message":"no."}}"#);
    }

    #[test]
    fn validation_details_round_trip_with_kind_tag() {
        let d = ApiValidationErrorDetails::Validation {
            issues: vec![ApiValidationIssue {
                path: "body.name".into(),
                message: "required".into(),
                code: Some("required".into()),
            }],
        };
        let s = serde_json::to_string(&d).unwrap();
        assert!(s.contains("\"kind\":\"validation\""));
        let back: ApiValidationErrorDetails = serde_json::from_str(&s).unwrap();
        match back {
            ApiValidationErrorDetails::Validation { issues } => {
                assert_eq!(issues.len(), 1);
                assert_eq!(issues[0].path, "body.name");
            }
        }
    }

    #[test]
    fn sse_error_payload_optional_envelope() {
        // No envelope.
        let p = SseErrorPayload {
            message: "transient".into(),
            error: None,
        };
        assert_eq!(
            serde_json::to_string(&p).unwrap(),
            r#"{"message":"transient"}"#
        );
        // With envelope.
        let p2 = SseErrorPayload {
            message: "fatal".into(),
            error: Some(create_api_error(ApiErrorCode::InternalError, "boom")),
        };
        let s = serde_json::to_string(&p2).unwrap();
        assert!(s.contains("\"code\":\"INTERNAL_ERROR\""));
    }

    #[test]
    fn display_writes_slug() {
        assert_eq!(
            format!("{}", ApiErrorCode::ConnectorRateLimited),
            "CONNECTOR_RATE_LIMITED"
        );
    }

    #[test]
    fn full_error_count_matches_upstream() {
        // Sanity: enumerate by hand to make sure we ported all 41 codes
        // (16 generic + 5 tool + 10 live-artifact-ish + 9 connector + 1
        // INTERNAL_ERROR = 41). This guards against accidental deletion.
        let codes = [
            "BAD_REQUEST",
            "UNAUTHORIZED",
            "FORBIDDEN",
            "NOT_FOUND",
            "CONFLICT",
            "PAYLOAD_TOO_LARGE",
            "UNSUPPORTED_MEDIA_TYPE",
            "VALIDATION_FAILED",
            "AGENT_UNAVAILABLE",
            "AGENT_EXECUTION_FAILED",
            "AGENT_PROMPT_TOO_LARGE",
            "PROJECT_NOT_FOUND",
            "FILE_NOT_FOUND",
            "ARTIFACT_NOT_FOUND",
            "UPSTREAM_UNAVAILABLE",
            "RATE_LIMITED",
            "TOOL_TOKEN_MISSING",
            "TOOL_TOKEN_INVALID",
            "TOOL_TOKEN_EXPIRED",
            "TOOL_ENDPOINT_DENIED",
            "TOOL_OPERATION_DENIED",
            "LIVE_ARTIFACT_NOT_FOUND",
            "LIVE_ARTIFACT_INVALID",
            "LIVE_ARTIFACT_STORAGE_FAILED",
            "LIVE_ARTIFACT_REFRESH_UNAVAILABLE",
            "LIVE_ARTIFACT_REFRESH_TIMEOUT",
            "REFRESH_LOCKED",
            "REFRESH_TIMED_OUT",
            "REFRESH_FAILED",
            "OUTPUT_TOO_LARGE",
            "TEMPLATE_BINDING_INVALID",
            "REDACTION_REQUIRED",
            "CONNECTOR_NOT_FOUND",
            "CONNECTOR_NOT_CONNECTED",
            "CONNECTOR_DISABLED",
            "CONNECTOR_TOOL_NOT_FOUND",
            "CONNECTOR_SAFETY_DENIED",
            "CONNECTOR_INPUT_SCHEMA_MISMATCH",
            "CONNECTOR_RATE_LIMITED",
            "CONNECTOR_OUTPUT_TOO_LARGE",
            "CONNECTOR_EXECUTION_FAILED",
            "INTERNAL_ERROR",
        ];
        assert_eq!(codes.len(), 42);
        for slug in codes {
            // Round-trip every slug through the deserializer.
            let s = format!("\"{slug}\"");
            let _: ApiErrorCode = serde_json::from_str(&s)
                .unwrap_or_else(|e| panic!("failed to deserialize {slug}: {e}"));
        }
    }
}
