//! Integration tests for GeminiProvider against a mocked HTTP server.
//!
//! Path note: wiremock's `path()` matcher operates on the path component only
//! (no query string). The production URL embeds `?alt=sse` but we pass the
//! bare URL to `new_with_base_url` — the provider POSTs to it as-is and
//! wiremock matches on `/v1internal:streamGenerateContent`.

use kangnam_router::{LlmError, LlmProviderDyn, gemini::GeminiProvider};
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

const GEMINI_PATH: &str = "/v1internal:streamGenerateContent";

fn make_provider(base_url: String) -> GeminiProvider {
    GeminiProvider::new_with_base_url(
        "test-api-key".into(),
        "gemini-2.0-flash-exp".into(),
        base_url,
    )
}

#[tokio::test]
async fn gemini_401_maps_to_auth_error() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(GEMINI_PATH))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .mount(&mock)
        .await;

    let provider = make_provider(format!("{}{}", mock.uri(), GEMINI_PATH));
    let err = provider
        .render_dyn("sys", "hi", &json!({}))
        .await
        .unwrap_err();

    assert!(
        matches!(err, LlmError::Auth { ref provider } if provider == "gemini"),
        "expected Auth, got: {err:?}"
    );
}

#[tokio::test]
async fn gemini_429_maps_to_rate_limit_with_retry_after() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(GEMINI_PATH))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "7")
                .set_body_string("rate limited"),
        )
        .mount(&mock)
        .await;

    let provider = make_provider(format!("{}{}", mock.uri(), GEMINI_PATH));
    let err = provider
        .render_dyn("sys", "hi", &json!({}))
        .await
        .unwrap_err();

    match err {
        LlmError::RateLimit {
            provider,
            retry_after_secs,
        } => {
            assert_eq!(provider, "gemini");
            assert_eq!(retry_after_secs, Some(7));
        }
        other => panic!("expected RateLimit, got: {other:?}"),
    }
}

#[tokio::test]
async fn gemini_500_maps_to_upstream() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(GEMINI_PATH))
        .respond_with(ResponseTemplate::new(500).set_body_string("server error"))
        .mount(&mock)
        .await;

    let provider = make_provider(format!("{}{}", mock.uri(), GEMINI_PATH));
    let err = provider
        .render_dyn("sys", "hi", &json!({}))
        .await
        .unwrap_err();

    match err {
        LlmError::Upstream {
            provider,
            status,
            body: _,
        } => {
            assert_eq!(provider, "gemini");
            assert_eq!(status, 500);
        }
        other => panic!("expected Upstream, got: {other:?}"),
    }
}

/// See `closed_loopback_addr` in `claude_integration.rs` for rationale — bind
/// then drop to get a guaranteed-refused port, more portable than hard-coding
/// port 1 across CI environments.
fn closed_loopback_addr() -> std::net::SocketAddr {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral loopback port");
    let addr = listener.local_addr().expect("local_addr");
    drop(listener);
    addr
}

#[tokio::test]
async fn gemini_network_error_maps_to_network_variant() {
    let addr = closed_loopback_addr();
    let provider = GeminiProvider::new_with_base_url(
        "test-api-key".into(),
        "gemini-2.0-flash-exp".into(),
        format!("http://{addr}{GEMINI_PATH}"),
    );

    let err = provider
        .render_dyn("sys", "hi", &json!({}))
        .await
        .unwrap_err();
    assert!(
        matches!(err, LlmError::Network { ref provider, .. } if provider == "gemini"),
        "expected Network, got: {err:?}"
    );
}
