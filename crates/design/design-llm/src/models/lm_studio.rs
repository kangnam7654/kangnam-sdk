//! LM Studio (and any OpenAI-compatible server) model listing.

use super::{ModelListError, http};

/// Fetch the model list from an OpenAI-compatible `/v1/models` endpoint.
pub(super) async fn fetch_models(endpoint: &str) -> Result<Vec<String>, ModelListError> {
    let url = models_url(endpoint);
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
    parse_response(&body)
}

/// Normalize `endpoint` (e.g. `http://h/v1/chat/completions`, `http://h/v1`, `http://h/`)
/// to the `/v1/models` form expected by OpenAI-compatible servers.
pub(super) fn models_url(endpoint: &str) -> String {
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

pub(super) fn parse_response(body: &serde_json::Value) -> Result<Vec<String>, ModelListError> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parse_response_extracts_ids_in_order() {
        let body = serde_json::json!({
            "data": [
                {"id": "qwen2.5-coder:7b", "object": "model"},
                {"id": "llama-3.2-3b", "object": "model"}
            ],
            "object": "list"
        });
        let ids = parse_response(&body).unwrap();
        assert_eq!(ids, vec!["qwen2.5-coder:7b", "llama-3.2-3b"]);
    }

    #[test]
    fn parse_response_rejects_missing_data() {
        let body = serde_json::json!({ "object": "list" });
        assert!(matches!(
            parse_response(&body).unwrap_err(),
            ModelListError::Protocol(_)
        ));
    }

    #[test]
    fn parse_response_skips_entries_without_id() {
        let body = serde_json::json!({
            "data": [
                {"id": "a"},
                {"object": "model"},
                {"id": "b"}
            ]
        });
        let ids = parse_response(&body).unwrap();
        assert_eq!(ids, vec!["a", "b"]);
    }
}
