//! Integration tests for CodexProvider against a mocked HTTP server.

use kangnam_router::{LlmError, LlmProviderDyn, codex::CodexProvider};
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

const CODEX_PATH: &str = "/backend-api/codex/responses";

fn make_provider(base_url: String) -> CodexProvider {
    CodexProvider::new_with_base_url("test-api-key".into(), "gpt-4o".into(), base_url)
}

#[tokio::test]
async fn codex_401_maps_to_auth_error() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(CODEX_PATH))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .mount(&mock)
        .await;

    let provider = make_provider(format!("{}{}", mock.uri(), CODEX_PATH));
    let err = provider
        .render_dyn("sys", "hi", &json!({}))
        .await
        .unwrap_err();

    assert!(
        matches!(err, LlmError::Auth { ref provider } if provider == "codex"),
        "expected Auth, got: {err:?}"
    );
}

#[tokio::test]
async fn codex_429_maps_to_rate_limit_with_retry_after() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(CODEX_PATH))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "17")
                .set_body_string("rate limited"),
        )
        .mount(&mock)
        .await;

    let provider = make_provider(format!("{}{}", mock.uri(), CODEX_PATH));
    let err = provider
        .render_dyn("sys", "hi", &json!({}))
        .await
        .unwrap_err();

    match err {
        LlmError::RateLimit {
            provider,
            retry_after_secs,
        } => {
            assert_eq!(provider, "codex");
            assert_eq!(retry_after_secs, Some(17));
        }
        other => panic!("expected RateLimit, got: {other:?}"),
    }
}

#[tokio::test]
async fn codex_500_maps_to_upstream() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(CODEX_PATH))
        .respond_with(ResponseTemplate::new(500).set_body_string("server error"))
        .mount(&mock)
        .await;

    let provider = make_provider(format!("{}{}", mock.uri(), CODEX_PATH));
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
            assert_eq!(provider, "codex");
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
async fn codex_network_error_maps_to_network_variant() {
    let addr = closed_loopback_addr();
    let provider = CodexProvider::new_with_base_url(
        "test-api-key".into(),
        "gpt-4o".into(),
        format!("http://{addr}{CODEX_PATH}"),
    );

    let err = provider
        .render_dyn("sys", "hi", &json!({}))
        .await
        .unwrap_err();
    assert!(
        matches!(err, LlmError::Network { ref provider, .. } if provider == "codex"),
        "expected Network, got: {err:?}"
    );
}
