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

const GEMINI_DOCS_URL: &str = "https://ai.google.dev/gemini-api/docs/models";
const CLAUDE_DOCS_URL: &str = "https://platform.claude.com/docs/en/docs/about-claude/models";
const DOCS_USER_AGENT: &str =
    "Mozilla/5.0 (compatible; canvas-slide-designer/1.0; +https://github.com/kangnam7653/design-canvas)";

/// Curated Gemini model names.
///
/// Verified against <https://ai.google.dev/gemini-api/docs/models> on 2026-04-21.
/// Stable `generateContent` models first, then the 3.x preview family. Kept
/// deliberately short — users can always "직접 입력" for anything we missed.
const GEMINI_CURATED: &[&str] = &[
    "gemini-3.1-pro",
    "gemini-3-flash",
    "gemini-2.5-pro",
    "gemini-2.5-flash-lite",
    "gemini-2.5-flash",
    "gemini-2.0-flash-lite",
    "gemini-2.0-flash",
    "gemini-3.1-pro-preview",
    "gemini-3.1-flash-lite-preview",
    "gemini-3-pro-preview",
    "gemini-3-flash-preview",
    "gemini-2.5-flash-preview-09-2025",
    "gemini-2.5-flash-lite-preview-09-2025",
];

/// Curated Claude model names.
///
/// Verified against <https://platform.claude.com/docs/en/docs/about-claude/models>
/// on 2026-04-21. Aliases (`opus`/`sonnet`/`haiku`) are Claude Code CLI-specific
/// shortcuts that always point at the latest release. Dotted API aliases
/// (`claude-opus-4-7` etc.) are what the Anthropic API accepts directly.
///
/// Current (2026-04): Opus 4.7, Sonnet 4.6, Haiku 4.5. Older named aliases
/// kept so users on pinned legacy versions can select them.
const CLAUDE_CURATED: &[&str] = &[
    "opus",
    "sonnet",
    "haiku",
    "claude-sonnet-4-6",
    "claude-sonnet-4-5",
    "claude-sonnet-4-20250514",
    "claude-sonnet-4-0",
    "claude-opus-4-7",
    "claude-opus-4-6",
    "claude-opus-4-5",
    "claude-opus-4-20250514",
    "claude-opus-4-1",
    "claude-opus-4-0",
    "claude-haiku-4-5",
    "claude-sonnet-4-5-20250929",
    "claude-opus-4-5-20251101",
    "claude-opus-4-1-20250805",
    "claude-haiku-4-5-20251001",
];

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
        AiProvider::GeminiCli => fetch_gemini_or_curated().await,
        AiProvider::ClaudeCli => fetch_anthropic_or_curated().await,
        AiProvider::LmStudio => {
            let endpoint = endpoint
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or(ModelListError::MissingEndpoint)?;
            let models = fetch_lm_studio_models(endpoint).await?;
            Ok(ModelList {
                models,
                source: ModelSource::Live,
            })
        }
    }
}

// --- Gemini ---------------------------------------------------------------

fn gemini_api_key() -> Option<String> {
    std::env::var("GEMINI_API_KEY")
        .ok()
        .or_else(|| std::env::var("GOOGLE_API_KEY").ok())
        .filter(|s| !s.trim().is_empty())
}

async fn fetch_gemini_or_curated() -> Result<ModelList, ModelListError> {
    // 1. Official API (requires key)
    if let Some(key) = gemini_api_key() {
        match fetch_gemini_live(&key).await {
            Ok(models) if !models.is_empty() => {
                return Ok(ModelList {
                    models,
                    source: ModelSource::Live,
                });
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(error = %e, "gemini live API failed; falling back to docs scrape");
            }
        }
    }

    // 2. Scrape public docs page (no auth needed)
    match scrape_gemini_docs().await {
        Ok(models) if !models.is_empty() => {
            return Ok(ModelList {
                models,
                source: ModelSource::Live,
            });
        }
        Ok(_) => {}
        Err(e) => {
            tracing::warn!(error = %e, "gemini docs scrape failed; using curated");
        }
    }

    // 3. Curated
    Ok(ModelList {
        models: GEMINI_CURATED.iter().map(|s| s.to_string()).collect(),
        source: ModelSource::Curated,
    })
}

/// Scrape the public Gemini docs page for model IDs. Looks for strings of the
/// form `gemini-X.Y-<variant>` (dotted version) — these are the IDs users pass
/// to `gemini --model`. Hyphenated variants appearing in non-ID contexts are
/// ignored, and `*-deprecated` entries are filtered out.
async fn scrape_gemini_docs() -> Result<Vec<String>, ModelListError> {
    let html = fetch_docs_html(GEMINI_DOCS_URL).await?;
    Ok(parse_gemini_ids_from_html(&html))
}

fn parse_gemini_ids_from_html(html: &str) -> Vec<String> {
    // `gemini-` + major(.minor?) + `-` + kebab tail. Anchor on word boundaries
    // so we don't pick up partial matches inside other strings.
    let re = regex_lite::Regex::new(r"\bgemini-\d+(?:\.\d+)?-[a-z0-9-]+\b").unwrap();
    // Matches URL-anchor dash-version forms like `gemini-2-5-pro` (the docs
    // page renders anchors with dashes instead of the canonical dotted minor).
    let dash_version_re = regex_lite::Regex::new(r"^gemini-\d+-\d").unwrap();
    let mut seen = std::collections::BTreeSet::new();
    let mut ordered: Vec<String> = Vec::new();
    for m in re.find_iter(html) {
        let s = m.as_str();
        // Filter: skip anything flagged deprecated.
        if s.contains("deprecated") {
            continue;
        }
        // Skip ids that aren't `generateContent` chat models: TTS, image-only,
        // native-audio, computer-use, and any `-live-` variant (covers both
        // `gemini-live-*` and `*-flash-live-*`, which are audio endpoints).
        if s.contains("-tts")
            || s.contains("-live-")
            || s.contains("-image-preview")
            || s.ends_with("-image")
            || s.contains("-native-audio")
            || s.contains("-computer-use")
        {
            continue;
        }
        // Skip URL-anchor dash-version forms (`gemini-2-5-pro` duplicates
        // `gemini-2.5-pro`; the real API takes the dotted form).
        if dash_version_re.is_match(s) {
            continue;
        }
        if seen.insert(s.to_string()) {
            ordered.push(s.to_string());
        }
    }
    // Prefer stable (no "-preview") first, newer-generation first within each.
    ordered.sort_by(|a, b| {
        let a_preview = a.contains("-preview") || a.contains("-exp");
        let b_preview = b.contains("-preview") || b.contains("-exp");
        a_preview.cmp(&b_preview).then_with(|| b.cmp(a))
    });
    ordered
}

async fn fetch_gemini_live(api_key: &str) -> Result<Vec<String>, ModelListError> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={api_key}"
    );
    let client = http_client()?;
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| ModelListError::Request(e.to_string()))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(ModelListError::Request(format!("HTTP {status}")));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ModelListError::Protocol(format!("invalid JSON: {e}")))?;
    parse_gemini_models(&body)
}

fn parse_gemini_models(body: &serde_json::Value) -> Result<Vec<String>, ModelListError> {
    let arr = body
        .get("models")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ModelListError::Protocol("missing `models` array".into()))?;

    // Each entry has `name: "models/gemini-3.1-pro"`. We strip the prefix and
    // only keep ones that support `generateContent` (i.e. chat models, not
    // embedding-only ones).
    let ids: Vec<String> = arr
        .iter()
        .filter(|entry| {
            entry
                .get("supportedGenerationMethods")
                .and_then(|v| v.as_array())
                .is_none_or(|methods| {
                    methods
                        .iter()
                        .any(|m| m.as_str() == Some("generateContent"))
                })
        })
        .filter_map(|entry| entry.get("name").and_then(|v| v.as_str()))
        .map(|name| name.strip_prefix("models/").unwrap_or(name).to_string())
        .collect();
    Ok(ids)
}

// --- Anthropic ------------------------------------------------------------

fn anthropic_api_key() -> Option<String> {
    std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty())
}

async fn fetch_anthropic_or_curated() -> Result<ModelList, ModelListError> {
    // 1. Official API (requires key)
    if let Some(key) = anthropic_api_key() {
        match fetch_anthropic_live(&key).await {
            Ok(models) if !models.is_empty() => {
                return Ok(ModelList {
                    models: prepend_cli_aliases(models),
                    source: ModelSource::Live,
                });
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(error = %e, "anthropic live API failed; falling back to docs scrape");
            }
        }
    }

    // 2. Scrape public docs page
    match scrape_anthropic_docs().await {
        Ok(models) if !models.is_empty() => {
            return Ok(ModelList {
                models: prepend_cli_aliases(models),
                source: ModelSource::Live,
            });
        }
        Ok(_) => {}
        Err(e) => {
            tracing::warn!(error = %e, "anthropic docs scrape failed; using curated");
        }
    }

    // 3. Curated
    Ok(ModelList {
        models: CLAUDE_CURATED.iter().map(|s| s.to_string()).collect(),
        source: ModelSource::Curated,
    })
}

/// Prepend Claude Code CLI-specific short aliases (`opus` / `sonnet` / `haiku`)
/// so users always see them as shortcuts, even when the source only returns
/// full API IDs. Deduplicates while preserving order.
fn prepend_cli_aliases(models: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = ["opus", "sonnet", "haiku"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    out.extend(models);
    let mut seen = std::collections::HashSet::new();
    out.retain(|m| seen.insert(m.clone()));
    out
}

/// Scrape the Anthropic public docs for Claude model IDs. Looks for
/// `claude-<opus|sonnet|haiku>-<X>-<Y>` with an optional `-YYYYMMDD` suffix.
async fn scrape_anthropic_docs() -> Result<Vec<String>, ModelListError> {
    let html = fetch_docs_html(CLAUDE_DOCS_URL).await?;
    Ok(parse_claude_ids_from_html(&html))
}

fn parse_claude_ids_from_html(html: &str) -> Vec<String> {
    let re =
        regex_lite::Regex::new(r"\bclaude-(?:opus|sonnet|haiku)-\d+-\d+(?:-\d{8})?\b").unwrap();
    let mut seen = std::collections::BTreeSet::new();
    let mut ordered: Vec<String> = Vec::new();
    for m in re.find_iter(html) {
        let s = m.as_str();
        if seen.insert(s.to_string()) {
            ordered.push(s.to_string());
        }
    }
    // Sort: aliases (no date suffix) before dated snapshots, newest first.
    ordered.sort_by(|a, b| {
        let a_dated = a.matches('-').count() >= 4; // claude-xxx-A-B-YYYYMMDD => 5 dashes
        let b_dated = b.matches('-').count() >= 4;
        a_dated.cmp(&b_dated).then_with(|| b.cmp(a))
    });
    ordered
}

async fn fetch_anthropic_live(api_key: &str) -> Result<Vec<String>, ModelListError> {
    let client = http_client()?;
    let resp = client
        .get("https://api.anthropic.com/v1/models")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
        .map_err(|e| ModelListError::Request(e.to_string()))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(ModelListError::Request(format!("HTTP {status}")));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ModelListError::Protocol(format!("invalid JSON: {e}")))?;
    parse_anthropic_models(&body)
}

fn parse_anthropic_models(body: &serde_json::Value) -> Result<Vec<String>, ModelListError> {
    let arr = body
        .get("data")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ModelListError::Protocol("missing `data` array".into()))?;
    let ids: Vec<String> = arr
        .iter()
        .filter_map(|entry| entry.get("id").and_then(|v| v.as_str()).map(String::from))
        .collect();
    Ok(ids)
}

// --- LM Studio ------------------------------------------------------------

async fn fetch_lm_studio_models(endpoint: &str) -> Result<Vec<String>, ModelListError> {
    let url = models_url(endpoint);
    let client = http_client()?;
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| ModelListError::Request(e.to_string()))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(ModelListError::Request(format!("HTTP {status}")));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ModelListError::Protocol(format!("invalid JSON: {e}")))?;
    parse_lm_studio_response(&body)
}

/// Normalize `endpoint` (e.g. `http://h/v1/chat/completions`, `http://h/v1`, `http://h/`)
/// to the `/v1/models` form expected by OpenAI-compatible servers.
fn models_url(endpoint: &str) -> String {
    let trimmed = endpoint.trim_end_matches('/');
    if let Some(stripped) = trimmed.strip_suffix("/chat/completions") {
        return format!("{stripped}/models");
    }
    if trimmed.ends_with("/models") {
        return trimmed.to_string();
    }
    if trimmed.ends_with("/v1") {
        return format!("{trimmed}/models");
    }
    format!("{trimmed}/v1/models")
}

fn parse_lm_studio_response(body: &serde_json::Value) -> Result<Vec<String>, ModelListError> {
    let data = body
        .get("data")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ModelListError::Protocol("missing `data` array".into()))?;

    let ids: Vec<String> = data
        .iter()
        .filter_map(|entry| entry.get("id").and_then(|v| v.as_str()).map(String::from))
        .collect();

    Ok(ids)
}

// --- Shared ---------------------------------------------------------------

fn http_client() -> Result<reqwest::Client, ModelListError> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| ModelListError::Request(e.to_string()))
}

/// Fetch an HTML docs page with a polite User-Agent. Used by the scrape
/// fallbacks for Gemini / Claude model lists.
async fn fetch_docs_html(url: &str) -> Result<String, ModelListError> {
    let client = http_client()?;
    let resp = client
        .get(url)
        .header("user-agent", DOCS_USER_AGENT)
        .send()
        .await
        .map_err(|e| ModelListError::Request(e.to_string()))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(ModelListError::Request(format!("HTTP {status}")));
    }
    resp.text()
        .await
        .map_err(|e| ModelListError::Request(e.to_string()))
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

    /// Unit: the curated list is non-empty so we always have a last-resort.
    #[test]
    fn curated_lists_are_populated() {
        assert!(!GEMINI_CURATED.is_empty());
        assert!(GEMINI_CURATED.iter().any(|s| s.starts_with("gemini-2.5")));
        assert!(!CLAUDE_CURATED.is_empty());
        assert!(CLAUDE_CURATED.contains(&"opus"));
        assert!(CLAUDE_CURATED.contains(&"sonnet"));
        assert!(CLAUDE_CURATED.contains(&"haiku"));
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
        let err = list_models(AiProvider::LmStudio, Some("   ")).await.unwrap_err();
        assert!(matches!(err, ModelListError::MissingEndpoint));
    }

    #[test]
    fn models_url_handles_various_endpoint_shapes() {
        assert_eq!(
            models_url("http://host:1234/v1/chat/completions"),
            "http://host:1234/v1/models"
        );
        assert_eq!(
            models_url("http://host:1234/v1"),
            "http://host:1234/v1/models"
        );
        assert_eq!(
            models_url("http://host:1234/"),
            "http://host:1234/v1/models"
        );
        assert_eq!(
            models_url("http://host:1234/v1/models"),
            "http://host:1234/v1/models"
        );
    }

    #[test]
    fn parse_lm_studio_response_extracts_ids_in_order() {
        let body = serde_json::json!({
            "data": [
                {"id": "qwen2.5-coder:7b", "object": "model"},
                {"id": "llama-3.2-3b", "object": "model"}
            ],
            "object": "list"
        });
        let ids = parse_lm_studio_response(&body).unwrap();
        assert_eq!(ids, vec!["qwen2.5-coder:7b", "llama-3.2-3b"]);
    }

    #[test]
    fn parse_lm_studio_response_rejects_missing_data() {
        let body = serde_json::json!({ "object": "list" });
        assert!(matches!(
            parse_lm_studio_response(&body).unwrap_err(),
            ModelListError::Protocol(_)
        ));
    }

    #[test]
    fn parse_lm_studio_response_skips_entries_without_id() {
        let body = serde_json::json!({
            "data": [
                {"id": "a"},
                {"object": "model"},
                {"id": "b"}
            ]
        });
        let ids = parse_lm_studio_response(&body).unwrap();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn parse_gemini_models_strips_prefix_and_filters_generate_content() {
        let body = serde_json::json!({
            "models": [
                {
                    "name": "models/gemini-3.1-pro",
                    "supportedGenerationMethods": ["generateContent", "countTokens"]
                },
                {
                    "name": "models/embedding-001",
                    "supportedGenerationMethods": ["embedContent"]
                },
                {
                    "name": "models/gemini-3.1-flash",
                    "supportedGenerationMethods": ["generateContent"]
                }
            ]
        });
        let ids = parse_gemini_models(&body).unwrap();
        assert_eq!(ids, vec!["gemini-3.1-pro", "gemini-3.1-flash"]);
    }

    #[test]
    fn parse_gemini_ids_from_html_extracts_stable_first_and_skips_tts() {
        // Simulated snippet of the real docs page. Contains a mix of stable,
        // preview, TTS, image, and deprecated ids.
        let html = r#"
            <code>gemini-2.5-pro</code> is our flagship.
            See also <code>gemini-2.5-flash</code> and
            <code>gemini-2.5-flash-lite</code>.
            Preview: <code>gemini-3.1-pro-preview</code>,
            <code>gemini-3-flash-preview</code>.
            Audio: <code>gemini-2.5-flash-tts-preview</code> (skip).
            Image: <code>gemini-3-pro-image-preview</code> (skip).
            Legacy: <code>gemini-2.0-flash-deprecated</code> (skip).
        "#;
        let ids = parse_gemini_ids_from_html(html);
        assert!(ids.contains(&"gemini-2.5-pro".to_string()));
        assert!(ids.contains(&"gemini-2.5-flash".to_string()));
        assert!(ids.contains(&"gemini-2.5-flash-lite".to_string()));
        assert!(ids.contains(&"gemini-3.1-pro-preview".to_string()));
        assert!(ids.contains(&"gemini-3-flash-preview".to_string()));
        // Filters:
        assert!(!ids.iter().any(|s| s.contains("-tts")));
        assert!(!ids.iter().any(|s| s.contains("-image-preview")));
        assert!(!ids.iter().any(|s| s.contains("deprecated")));
        // Stable ids should come before preview ids.
        let pos_stable = ids.iter().position(|s| s == "gemini-2.5-pro").unwrap();
        let pos_preview = ids
            .iter()
            .position(|s| s == "gemini-3.1-pro-preview")
            .unwrap();
        assert!(pos_stable < pos_preview);
    }

    #[test]
    fn parse_gemini_ids_rejects_dash_version_anchors_and_live_variants() {
        // Docs page renders URL anchors with dashes — we must keep the dotted
        // forms and drop the dashed duplicates. Live-audio variants should
        // also be filtered regardless of where `-live-` appears.
        let html = r#"
            <a id="gemini-2-5-pro">gemini-2.5-pro</a>
            <a id="gemini-2-5-flash">gemini-2.5-flash</a>
            <code>gemini-3.1-flash-live-preview</code>
            <code>gemini-live-2.5-flash-preview</code>
            <code>gemini-3-flash-preview</code>
        "#;
        let ids = parse_gemini_ids_from_html(html);
        assert!(ids.contains(&"gemini-2.5-pro".to_string()));
        assert!(ids.contains(&"gemini-2.5-flash".to_string()));
        assert!(ids.contains(&"gemini-3-flash-preview".to_string()));
        // Dash-version duplicates dropped.
        assert!(!ids.iter().any(|s| s == "gemini-2-5-pro"));
        assert!(!ids.iter().any(|s| s == "gemini-2-5-flash"));
        // Live-audio variants dropped.
        assert!(!ids.iter().any(|s| s.contains("-live-")));
    }

    #[test]
    fn parse_claude_ids_from_html_sorts_aliases_before_dated() {
        let html = r#"
            <td>claude-opus-4-7</td>
            <td>claude-sonnet-4-6</td>
            <td>claude-haiku-4-5</td>
            <td>claude-haiku-4-5-20251001</td>
            <td>claude-sonnet-4-5-20250929</td>
            <td>claude-opus-4-1-20250805</td>
        "#;
        let ids = parse_claude_ids_from_html(html);
        assert!(ids.contains(&"claude-opus-4-7".to_string()));
        assert!(ids.contains(&"claude-sonnet-4-6".to_string()));
        assert!(ids.contains(&"claude-haiku-4-5".to_string()));
        assert!(ids.contains(&"claude-haiku-4-5-20251001".to_string()));
        // Aliases (no date) should come before dated snapshots.
        let alias_pos = ids
            .iter()
            .position(|s| s == "claude-haiku-4-5")
            .unwrap();
        let dated_pos = ids
            .iter()
            .position(|s| s == "claude-haiku-4-5-20251001")
            .unwrap();
        assert!(alias_pos < dated_pos);
    }

    #[test]
    fn prepend_cli_aliases_deduplicates() {
        let input = vec!["sonnet".to_string(), "claude-sonnet-4-6".to_string()];
        let out = prepend_cli_aliases(input);
        assert_eq!(
            out,
            vec![
                "opus".to_string(),
                "sonnet".to_string(),
                "haiku".to_string(),
                "claude-sonnet-4-6".to_string(),
            ]
        );
    }

    #[test]
    fn parse_anthropic_models_extracts_ids() {
        let body = serde_json::json!({
            "data": [
                {"id": "claude-opus-4-7", "type": "model"},
                {"id": "claude-sonnet-4-7", "type": "model"}
            ]
        });
        let ids = parse_anthropic_models(&body).unwrap();
        assert_eq!(ids, vec!["claude-opus-4-7", "claude-sonnet-4-7"]);
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
                let body = r#"{"data":[{"id":"qwen2.5-coder:7b"},{"id":"llama-3.2-3b"}],"object":"list"}"#;
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
        let list = list_models(AiProvider::LmStudio, Some(&endpoint)).await.unwrap();
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
