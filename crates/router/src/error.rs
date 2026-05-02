//! Typed error enum for LLM provider calls. Every variant carries a
//! `provider` name so logs and metrics can aggregate per-provider.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("missing configuration for provider '{provider}': {reason}")]
    MissingConfig { provider: String, reason: String },

    #[error("authentication failed for provider '{provider}'")]
    Auth { provider: String },

    #[error("rate limited by provider '{provider}'")]
    RateLimit {
        provider: String,
        retry_after_secs: Option<u32>,
    },

    #[error("upstream error from '{provider}': status={status}")]
    Upstream {
        provider: String,
        status: u16,
        body: String,
    },

    #[error("network error to '{provider}': {msg}")]
    Network { provider: String, msg: String },

    #[error("failed to parse response from '{provider}': {reason}")]
    Parse { provider: String, reason: String },

    #[error("other error from '{provider}': {message}")]
    Other { provider: String, message: String },
}

/// Parse the `Retry-After` header from a response (seconds as integer).
/// Used by providers that want to surface `LlmError::RateLimit { retry_after_secs }`.
pub(crate) fn parse_retry_after(resp: &reqwest::Response) -> Option<u32> {
    resp.headers()
        .get("retry-after")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u32>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_includes_provider_for_every_variant() {
        let cases = [
            LlmError::MissingConfig {
                provider: "claude".into(),
                reason: "x".into(),
            },
            LlmError::Auth {
                provider: "codex".into(),
            },
            LlmError::RateLimit {
                provider: "gemini".into(),
                retry_after_secs: Some(30),
            },
            LlmError::Upstream {
                provider: "copilot".into(),
                status: 500,
                body: "err".into(),
            },
            LlmError::Network {
                provider: "claude".into(),
                msg: "timeout".into(),
            },
            LlmError::Parse {
                provider: "gemini".into(),
                reason: "bad json".into(),
            },
            LlmError::Other {
                provider: "dummy".into(),
                message: "misc".into(),
            },
        ];
        for e in cases {
            let s = e.to_string();
            let providers = ["claude", "codex", "gemini", "copilot", "dummy"];
            assert!(
                providers.iter().any(|p| s.contains(p)),
                "Display output missing provider name: {s}"
            );
        }
    }
}
