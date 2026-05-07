//! Anthropic / Claude model discovery: curated fallback, live API, and
//! public-docs scrape.

use super::{http, ModelList, ModelListError, ModelSource};

const CLAUDE_DOCS_URL: &str =
    "https://platform.claude.com/docs/en/docs/about-claude/models";

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

fn api_key() -> Option<String> {
    std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty())
}

pub(super) async fn fetch_or_curated() -> Result<ModelList, ModelListError> {
    // 1. Official API (requires key)
    if let Some(key) = api_key() {
        match fetch_live(&key).await {
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
    match scrape_docs().await {
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
pub(super) fn prepend_cli_aliases(models: Vec<String>) -> Vec<String> {
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
async fn scrape_docs() -> Result<Vec<String>, ModelListError> {
    let html = http::fetch_docs_html(CLAUDE_DOCS_URL).await?;
    Ok(parse_ids_from_html(&html))
}

pub(super) fn parse_ids_from_html(html: &str) -> Vec<String> {
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

async fn fetch_live(api_key: &str) -> Result<Vec<String>, ModelListError> {
    let client = http::http_client()?;
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
    parse_api_response(&body)
}

pub(super) fn parse_api_response(body: &serde_json::Value) -> Result<Vec<String>, ModelListError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curated_list_is_populated() {
        assert!(!CLAUDE_CURATED.is_empty());
        assert!(CLAUDE_CURATED.contains(&"opus"));
        assert!(CLAUDE_CURATED.contains(&"sonnet"));
        assert!(CLAUDE_CURATED.contains(&"haiku"));
    }

    #[test]
    fn parse_ids_from_html_sorts_aliases_before_dated() {
        let html = r#"
            <td>claude-opus-4-7</td>
            <td>claude-sonnet-4-6</td>
            <td>claude-haiku-4-5</td>
            <td>claude-haiku-4-5-20251001</td>
            <td>claude-sonnet-4-5-20250929</td>
            <td>claude-opus-4-1-20250805</td>
        "#;
        let ids = parse_ids_from_html(html);
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
    fn parse_api_response_extracts_ids() {
        let body = serde_json::json!({
            "data": [
                {"id": "claude-opus-4-7", "type": "model"},
                {"id": "claude-sonnet-4-7", "type": "model"}
            ]
        });
        let ids = parse_api_response(&body).unwrap();
        assert_eq!(ids, vec!["claude-opus-4-7", "claude-sonnet-4-7"]);
    }
}
