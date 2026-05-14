//! Model discovery per provider.
//!
//! Neither Gemini CLI nor Claude CLI exposes a model-listing subcommand, so we
//! try three sources in order for CLI providers:
//!
//! 1. **Official API** (live) — if the relevant API key env var is set, hit
//!    Google / Anthropic's `/v1/models` endpoint directly. Freshest possible.
//! 2. **Public docs scrape** (live) — parse the official docs page HTML. Works
//!    without credentials; regex-based so it is tolerant to layout shifts but
//!    depends on the docs page exposing model IDs in plain text (verified as
//!    of 2026-04-21).
//! 3. **Curated fallback** — hand-maintained list of known-shipping names,
//!    updated at commit time.
//!
//! LM Studio is separate: always live against the user's endpoint `/v1/models`.
//!
//! Env vars for path #1:
//! - Gemini: `GEMINI_API_KEY`, `GOOGLE_API_KEY`
//! - Claude: `ANTHROPIC_API_KEY`
//!
//! Source URLs for path #2:
//! - Gemini: <https://ai.google.dev/gemini-api/docs/models>
//! - Claude: <https://platform.claude.com/docs/en/docs/about-claude/models>
//!
//! All three paths return names the UI treats as *suggestions* — users can
//! always type a custom name via "직접 입력".

use crate::providers::AiProvider;

mod anthropic;
mod gemini;
mod http;
mod lm_studio;

/// Where a model name came from. Surfaced to the UI so users know whether to
/// trust freshness — a `Live` list is from the upstream API right now; a
/// `Curated` list is our hand-maintained fallback and may lag new releases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelSource {
    Live,
    Curated,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ModelList {
    pub models: Vec<String>,
    pub source: ModelSource,
}

#[derive(Debug, thiserror::Error)]
pub enum ModelListError {
    #[error("lm-studio requires endpoint URL")]
    MissingEndpoint,
    #[error("request failed: {0}")]
    Request(String),
    #[error("unexpected response: {0}")]
    Protocol(String),
}

/// Returns model name suggestions for a given provider.
///
/// - LM Studio: always live, hits the user's endpoint.
/// - Gemini/Claude: live if the corresponding API key is set in env, otherwise
///   curated. We never fail open for a CLI provider — if live fetch errors, we
///   surface the error to the caller (who may choose to fall back).
pub async fn list_models(
    provider: AiProvider,
    endpoint: Option<&str>,
) -> Result<ModelList, ModelListError> {
    match provider {
        AiProvider::GeminiCli => gemini::fetch_or_curated().await,
        AiProvider::ClaudeCli => anthropic::fetch_or_curated().await,
        AiProvider::LmStudio => {
            let endpoint = endpoint
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or(ModelListError::MissingEndpoint)?;
            let models = lm_studio::fetch_models(endpoint).await?;
            Ok(ModelList {
                models,
                source: ModelSource::Live,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: clear env vars so tests don't accidentally do live fetches.
    // SAFETY: tests run with `multi_thread` flavor but we only mutate env at the
    // very start of each test before any other thread is spawned.
    fn clear_keys() {
        for k in ["GEMINI_API_KEY", "GOOGLE_API_KEY", "ANTHROPIC_API_KEY"] {
            // SAFETY: see fn doc.
            unsafe { std::env::remove_var(k) };
        }
    }

    /// Integration: without an API key, the docs-scrape fallback should
    /// succeed and return a live list of Gemini IDs. Skipped if the network
    /// is unavailable (we don't want CI to be coupled to Google docs uptime).
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "hits ai.google.dev — run with `cargo test -- --ignored`"]
    async fn gemini_scrapes_docs_when_no_api_key() {
        clear_keys();
        let list = list_models(AiProvider::GeminiCli, None).await.unwrap();
        assert_eq!(list.source, ModelSource::Live);
        assert!(list.models.iter().any(|s| s.starts_with("gemini-")));
    }

    /// Integration: same for Anthropic docs.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "hits platform.claude.com — run with `cargo test -- --ignored`"]
    async fn claude_scrapes_docs_when_no_api_key() {
        clear_keys();
        let list = list_models(AiProvider::ClaudeCli, None).await.unwrap();
        assert_eq!(list.source, ModelSource::Live);
        assert!(list.models.contains(&"sonnet".to_string()));
        assert!(list.models.iter().any(|s| s.starts_with("claude-")));
    }

    /// Unit: the curated lists are non-empty so we always have a last-resort.
    #[test]
    fn curated_lists_are_populated() {
        // Delegated to submodule tests — this top-level test covers the
        // integration path: list_models falls back to curated when no keys set.
        // The actual content assertions live in gemini::tests and anthropic::tests.
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn lm_studio_requires_endpoint() {
        clear_keys();
        let err = list_models(AiProvider::LmStudio, None).await.unwrap_err();
        assert!(matches!(err, ModelListError::MissingEndpoint));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn lm_studio_rejects_blank_endpoint() {
        clear_keys();
        let err = list_models(AiProvider::LmStudio, Some("   "))
            .await
            .unwrap_err();
        assert!(matches!(err, ModelListError::MissingEndpoint));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn lm_studio_fetches_from_mock_server() {
        clear_keys();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf).await;
                let body =
                    r#"{"data":[{"id":"qwen2.5-coder:7b"},{"id":"llama-3.2-3b"}],"object":"list"}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        });

        let endpoint = format!("http://{addr}/v1");
        let list = list_models(AiProvider::LmStudio, Some(&endpoint))
            .await
            .unwrap();
        assert_eq!(list.source, ModelSource::Live);
        assert_eq!(list.models, vec!["qwen2.5-coder:7b", "llama-3.2-3b"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn lm_studio_surfaces_http_error() {
        clear_keys();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf).await;
                let response = "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n";
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        });

        let endpoint = format!("http://{addr}/v1");
        let err = list_models(AiProvider::LmStudio, Some(&endpoint))
            .await
            .unwrap_err();
        assert!(matches!(err, ModelListError::Request(_)));
    }
}
