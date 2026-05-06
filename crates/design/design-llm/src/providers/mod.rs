//! Concrete [`AiClient`](crate::client::AiClient) implementations, plus the
//! [`AiProvider`] enum and [`AiProviderConfig`] that pick one at runtime
//! from a stored project setting.
//!
//! The three supported providers as of v1:
//! - **Gemini CLI** (`gemini-cli`) — shells out to `gemini -p`, streams `stream-json` lines.
//! - **Claude CLI** (`claude-cli`) — shells out to `claude -p`, streams `stream-json` with
//!   `--include-partial-messages`. Image attachments drop in v1 (known limitation).
//! - **LM Studio** (`lm-studio`) — any OpenAI-compatible `/v1/chat/completions` server
//!   (LM Studio, Ollama, vLLM, llama.cpp) over SSE.
//!
//! The [`fake`] module provides an in-memory [`FakeAiClient`] for tests; it's
//! gated behind the `test-util` feature so production builds don't ship it.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::client::AiClient;

pub mod claude_cli;
pub mod gemini_cli;
pub mod lm_studio;

#[cfg(feature = "test-util")]
pub mod fake;

pub use claude_cli::ClaudeCliClient;
pub use gemini_cli::GeminiCliClient;
pub use lm_studio::LmStudioClient;

#[cfg(feature = "test-util")]
pub use fake::FakeAiClient;

/// The set of AI backends a project can be configured with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AiProvider {
    GeminiCli,
    ClaudeCli,
    LmStudio,
}

impl AiProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            AiProvider::GeminiCli => "gemini-cli",
            AiProvider::ClaudeCli => "claude-cli",
            AiProvider::LmStudio => "lm-studio",
        }
    }

    /// Parse a provider id slug. Returns `None` for unknown ids.
    /// Inherent method (rather than [`std::str::FromStr`] impl) so the
    /// "unknown id is not an error" semantic stays explicit at call sites.
    pub fn from_slug(s: &str) -> Option<Self> {
        match s {
            "gemini-cli" => Some(Self::GeminiCli),
            "claude-cli" => Some(Self::ClaudeCli),
            "lm-studio" => Some(Self::LmStudio),
            _ => None,
        }
    }
}

/// Resolved configuration for building an [`AiClient`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProviderConfig {
    pub provider: AiProvider,
    pub endpoint: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AiProviderError {
    #[error("unknown AI provider: {0}")]
    Unknown(String),
    #[error("provider `{provider}` requires `{field}`")]
    MissingField {
        provider: &'static str,
        field: &'static str,
    },
}

impl AiProviderConfig {
    /// Construct + validate a config from raw string fields, as typically
    /// stored on a `projects` row.
    pub fn from_parts(
        provider: &str,
        endpoint: Option<&str>,
        model: Option<&str>,
    ) -> Result<Self, AiProviderError> {
        let provider = AiProvider::from_slug(provider)
            .ok_or_else(|| AiProviderError::Unknown(provider.to_string()))?;

        let endpoint = endpoint.map(str::trim).filter(|s| !s.is_empty()).map(String::from);
        let model = model.map(str::trim).filter(|s| !s.is_empty()).map(String::from);

        if matches!(provider, AiProvider::LmStudio) && endpoint.is_none() {
            return Err(AiProviderError::MissingField {
                provider: "lm-studio",
                field: "endpoint",
            });
        }

        Ok(Self {
            provider,
            endpoint,
            model,
        })
    }

    /// Instantiate the concrete [`AiClient`] for this config.
    ///
    /// CLI binaries are discovered via the `CANVAS_GEMINI_BIN` /
    /// `CANVAS_CLAUDE_BIN` env vars, falling back to `gemini` / `claude` on
    /// `$PATH`. LM Studio uses the configured `endpoint` as-is (the client
    /// appends `/v1/chat/completions` if missing).
    pub fn resolve(&self) -> Arc<dyn AiClient> {
        match self.provider {
            AiProvider::GeminiCli => {
                let bin = std::env::var("CANVAS_GEMINI_BIN").unwrap_or_else(|_| "gemini".into());
                let mut client = GeminiCliClient::new(bin);
                if let Some(m) = &self.model {
                    client = client.with_args(vec!["-m".into(), m.clone()]);
                }
                Arc::new(client)
            }
            AiProvider::ClaudeCli => {
                let bin = std::env::var("CANVAS_CLAUDE_BIN").unwrap_or_else(|_| "claude".into());
                Arc::new(ClaudeCliClient::new(bin, self.model.clone()))
            }
            AiProvider::LmStudio => Arc::new(LmStudioClient::new(
                self.endpoint.clone().expect("endpoint validated by from_parts"),
                self.model.clone().unwrap_or_else(|| "local-model".to_string()),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_slug_round_trip() {
        for p in [AiProvider::GeminiCli, AiProvider::ClaudeCli, AiProvider::LmStudio] {
            assert_eq!(AiProvider::from_slug(p.as_str()), Some(p));
        }
    }

    #[test]
    fn unknown_provider_is_rejected() {
        let err = AiProviderConfig::from_parts("grok-9000", None, None).unwrap_err();
        assert_eq!(err, AiProviderError::Unknown("grok-9000".into()));
    }

    #[test]
    fn lm_studio_requires_endpoint() {
        let err = AiProviderConfig::from_parts("lm-studio", None, Some("qwen")).unwrap_err();
        assert!(matches!(
            err,
            AiProviderError::MissingField { provider: "lm-studio", field: "endpoint" }
        ));
    }

    #[test]
    fn lm_studio_accepts_endpoint_and_model() {
        let cfg = AiProviderConfig::from_parts(
            "lm-studio",
            Some("http://localhost:1234/v1"),
            Some("qwen2.5:7b"),
        )
        .unwrap();
        assert_eq!(cfg.provider, AiProvider::LmStudio);
        assert_eq!(cfg.endpoint.as_deref(), Some("http://localhost:1234/v1"));
        assert_eq!(cfg.model.as_deref(), Some("qwen2.5:7b"));
    }

    #[test]
    fn cli_providers_do_not_require_endpoint() {
        let cfg = AiProviderConfig::from_parts("gemini-cli", None, None).unwrap();
        assert_eq!(cfg.provider, AiProvider::GeminiCli);
        assert!(cfg.endpoint.is_none());

        let cfg = AiProviderConfig::from_parts("claude-cli", None, Some("opus-4-5")).unwrap();
        assert_eq!(cfg.provider, AiProvider::ClaudeCli);
        assert_eq!(cfg.model.as_deref(), Some("opus-4-5"));
    }

    #[test]
    fn blank_strings_become_none() {
        let cfg = AiProviderConfig::from_parts("gemini-cli", Some("   "), Some("")).unwrap();
        assert!(cfg.endpoint.is_none());
        assert!(cfg.model.is_none());
    }
}
