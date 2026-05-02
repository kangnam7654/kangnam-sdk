use kangnam_router::{ChatMessage, LlmRequestOptions, create_provider};

#[tokio::test]
async fn dummy_provider_accepts_options_and_ignores_them() {
    let provider = create_provider("dummy", "", "", "").expect("dummy creates");
    let messages = vec![ChatMessage::user("hi")];
    let options = LlmRequestOptions {
        allow_web_search: true,
        max_turns: Some(3),
        ..Default::default()
    };
    let resp = provider
        .chat_with_options_dyn("system", &messages, &options, &serde_json::json!({}))
        .await
        .expect("dummy responds");
    assert!(!resp.rendered_text.is_empty());
}

#[tokio::test]
async fn dummy_stream_with_options_forwards_correctly() {
    use futures::StreamExt;
    let provider = create_provider("dummy", "", "", "").expect("dummy creates");
    let messages = vec![ChatMessage::user("hi")];
    let options = LlmRequestOptions::default();
    let result_json = serde_json::json!({});
    let mut stream =
        provider.chat_stream_with_options_dyn("sys", &messages, &options, &result_json);
    let first = stream.next().await;
    assert!(first.is_some(), "stream must yield at least one event");
}
