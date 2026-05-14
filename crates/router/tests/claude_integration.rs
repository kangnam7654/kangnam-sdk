//! Integration tests for ClaudeProvider against a mocked HTTP server.

use futures::StreamExt;
use kangnam_router::{
    ChatMessage, LlmError, LlmProviderDyn, LlmStreamEvent, claude::ClaudeProvider,
};
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

fn make_provider(base_url: String) -> ClaudeProvider {
    ClaudeProvider::new_with_base_url(
        "test-api-key".into(),
        "claude-sonnet-4-20250514".into(),
        base_url,
    )
}

#[tokio::test]
async fn claude_401_maps_to_auth_error() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .mount(&mock)
        .await;

    let provider = make_provider(format!("{}/v1/messages", mock.uri()));
    let err = provider
        .render_dyn("sys", "hi", &json!({}))
        .await
        .unwrap_err();

    assert!(
        matches!(err, LlmError::Auth { ref provider } if provider == "claude"),
        "expected Auth, got: {err:?}"
    );
}

#[tokio::test]
async fn claude_429_maps_to_rate_limit_with_retry_after() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "42")
                .set_body_string("rate limited"),
        )
        .mount(&mock)
        .await;

    let provider = make_provider(format!("{}/v1/messages", mock.uri()));
    let err = provider
        .render_dyn("sys", "hi", &json!({}))
        .await
        .unwrap_err();

    match err {
        LlmError::RateLimit {
            provider,
            retry_after_secs,
        } => {
            assert_eq!(provider, "claude");
            assert_eq!(retry_after_secs, Some(42));
        }
        other => panic!("expected RateLimit, got: {other:?}"),
    }
}

#[tokio::test]
async fn claude_500_maps_to_upstream() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(500).set_body_string("server error"))
        .mount(&mock)
        .await;

    let provider = make_provider(format!("{}/v1/messages", mock.uri()));
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
            assert_eq!(provider, "claude");
            assert_eq!(status, 500);
        }
        other => panic!("expected Upstream, got: {other:?}"),
    }
}

/// Bind a TCP listener on an ephemeral port, capture the assigned address,
/// then drop the listener. The port enters TIME_WAIT (or is closed immediately
/// on most OSes), so any subsequent connect attempt gets `ECONNREFUSED` on all
/// platforms we care about. More portable than hard-coding port 1, which on
/// some CI networking configs silently drops SYN packets and forces a 30s
/// timeout instead of an instant refusal.
fn closed_loopback_addr() -> std::net::SocketAddr {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral loopback port");
    let addr = listener.local_addr().expect("local_addr");
    drop(listener);
    addr
}

#[tokio::test]
async fn claude_network_error_maps_to_network_variant() {
    let addr = closed_loopback_addr();
    let provider = ClaudeProvider::new_with_base_url(
        "test-api-key".into(),
        "claude-sonnet-4-20250514".into(),
        format!("http://{addr}/v1/messages"),
    );

    let err = provider
        .render_dyn("sys", "hi", &json!({}))
        .await
        .unwrap_err();
    assert!(
        matches!(err, LlmError::Network { ref provider, .. } if provider == "claude"),
        "expected Network, got: {err:?}"
    );
}

#[tokio::test]
async fn claude_streaming_401_yields_error_with_auth_message() {
    // Streaming path shares the same error taxonomy as non-streaming:
    // HTTP 401 → LlmError::Auth → stringified into LlmStreamEvent::Error { message }.
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .mount(&mock)
        .await;

    let provider = make_provider(format!("{}/v1/messages", mock.uri()));
    let messages = vec![ChatMessage::user("hi")];
    // Bind `result_json` to a local so the `&` borrow outlives the `.await`
    // points below — `chat_stream_dyn` returns a `'a`-bounded stream.
    let result_json = json!({});
    let mut stream = provider.chat_stream_dyn("sys", &messages, &result_json);

    let event = stream.next().await.expect("stream must yield one event");
    match event {
        LlmStreamEvent::Error { message } => {
            // Display for LlmError::Auth is "authentication failed for provider 'claude'".
            assert!(
                message.contains("authentication failed") && message.contains("claude"),
                "expected auth error message, got: {message}"
            );
        }
        LlmStreamEvent::Delta { text } => panic!("expected Error, got Delta: {text}"),
        LlmStreamEvent::End { .. } => panic!("expected Error, got End"),
        _ => panic!("expected Error, got unexpected variant"),
    }

    // No further events after the terminal error.
    assert!(
        stream.next().await.is_none(),
        "stream must terminate after Error"
    );
}

#[tokio::test]
async fn claude_streaming_429_yields_error_with_rate_limit_message() {
    // Verifies the RateLimit branch of the streaming error taxonomy.
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "7")
                .set_body_string("rate limited"),
        )
        .mount(&mock)
        .await;

    let provider = make_provider(format!("{}/v1/messages", mock.uri()));
    let messages = vec![ChatMessage::user("hi")];
    let result_json = json!({});
    let mut stream = provider.chat_stream_dyn("sys", &messages, &result_json);

    let event = stream.next().await.expect("stream must yield one event");
    match event {
        LlmStreamEvent::Error { message } => {
            // Display for LlmError::RateLimit is "rate limited by provider 'claude'".
            assert!(
                message.contains("rate limited") && message.contains("claude"),
                "expected rate limit message, got: {message}"
            );
        }
        LlmStreamEvent::Delta { text } => panic!("expected Error, got Delta: {text}"),
        LlmStreamEvent::End { .. } => panic!("expected Error, got End"),
        _ => panic!("expected Error, got unexpected variant"),
    }
}

#[tokio::test]
async fn claude_oat_400_triggers_haiku_fallback() {
    let mock = MockServer::start().await;

    // First call returns 400 (triggers OAT fallback). Bound with up_to_n_times(1)
    // so the next call falls through to the success mock below.
    // wiremock matches mounted mocks in registration order (FIFO), so registering
    // the 400 mock first guarantees the first call hits it. Once it exhausts its
    // n_times budget, wiremock moves on to the next matching mock — the 200.
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(400).set_body_string("oat error"))
        .up_to_n_times(1)
        .mount(&mock)
        .await;
    // After the 400 mock is exhausted, wiremock falls through to this success
    // mock for the Haiku fallback retry.
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": [{"type": "text", "text": "fallback ok"}],
            "model": "claude-haiku-4-5",
            "usage": {"input_tokens": 10, "output_tokens": 5},
            "stop_reason": "end_turn",
        })))
        .mount(&mock)
        .await;

    // API key starts with "sk-ant-oat" to enter the OAT branch in chat_impl.
    let provider = ClaudeProvider::new_with_base_url(
        "sk-ant-oat-fake".into(),
        "claude-sonnet-4-20250514".into(),
        format!("{}/v1/messages", mock.uri()),
    );

    let resp = provider
        .render_dyn("sys", "hi", &json!({}))
        .await
        .expect("Haiku fallback must succeed");

    assert_eq!(resp.model, "claude-haiku-4-5");
    assert!(resp.rendered_text.contains("fallback ok"));
}
