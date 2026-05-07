//! Gemini model discovery: curated fallback, live API, and public-docs scrape.

use super::{http, ModelList, ModelListError, ModelSource};

const GEMINI_DOCS_URL: &str = "https://ai.google.dev/gemini-api/docs/models";

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

fn api_key() -> Option<String> {
    std::env::var("GEMINI_API_KEY")
        .ok()
        .or_else(|| std::env::var("GOOGLE_API_KEY").ok())
        .filter(|s| !s.trim().is_empty())
}

pub(super) async fn fetch_or_curated() -> Result<ModelList, ModelListError> {
    // 1. Official API (requires key)
    if let Some(key) = api_key() {
        match fetch_live(&key).await {
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
    match scrape_docs().await {
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
async fn scrape_docs() -> Result<Vec<String>, ModelListError> {
    let html = http::fetch_docs_html(GEMINI_DOCS_URL).await?;
    Ok(parse_ids_from_html(&html))
}

pub(super) fn parse_ids_from_html(html: &str) -> Vec<String> {
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

async fn fetch_live(api_key: &str) -> Result<Vec<String>, ModelListError> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={api_key}"
    );
    let client = http::http_client()?;
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
    parse_api_response(&body)
}

pub(super) fn parse_api_response(body: &serde_json::Value) -> Result<Vec<String>, ModelListError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curated_list_is_populated() {
        assert!(!GEMINI_CURATED.is_empty());
        assert!(GEMINI_CURATED.iter().any(|s| s.starts_with("gemini-2.5")));
    }

    #[test]
    fn parse_ids_from_html_extracts_stable_first_and_skips_tts() {
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
        let ids = parse_ids_from_html(html);
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
    fn parse_ids_rejects_dash_version_anchors_and_live_variants() {
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
        let ids = parse_ids_from_html(html);
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
    fn parse_api_response_strips_prefix_and_filters_generate_content() {
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
        let ids = parse_api_response(&body).unwrap();
        assert_eq!(ids, vec!["gemini-3.1-pro", "gemini-3.1-flash"]);
    }
}
