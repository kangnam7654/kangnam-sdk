//! Shared HTTP helpers used by every model-discovery strategy.

pub(super) const DOCS_USER_AGENT: &str = "Mozilla/5.0 (compatible; canvas-slide-designer/1.0; +https://github.com/kangnam7653/design-canvas)";

pub(super) fn http_client() -> Result<reqwest::Client, super::ModelListError> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| super::ModelListError::Request(e.to_string()))
}

/// Fetch an HTML docs page with a polite User-Agent. Used by the scrape
/// fallbacks for Gemini / Claude model lists.
pub(super) async fn fetch_docs_html(url: &str) -> Result<String, super::ModelListError> {
    let client = http_client()?;
    let resp = client
        .get(url)
        .header("user-agent", DOCS_USER_AGENT)
        .send()
        .await
        .map_err(|e| super::ModelListError::Request(e.to_string()))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(super::ModelListError::Request(format!("HTTP {status}")));
    }
    resp.text()
        .await
        .map_err(|e| super::ModelListError::Request(e.to_string()))
}
