//! canvas-llm — streaming AI client abstraction + provider implementations
//! (Gemini CLI, Claude CLI, LM Studio / OpenAI-compatible) used by Canvas
//! SDK generators and editors.
//!
//! The core abstraction is [`AiClient::complete`], which returns a
//! `BoxStream` of [`AiChunk`] values. Streaming is first-class: consumers
//! can surface partial text to their UI as the model produces it, and the
//! final `AiChunk::Done { full_text }` provides the concatenated output
//! for downstream parsing (e.g. SlideDoc extraction).

pub mod client;
pub mod models;
pub mod providers;
pub mod streaming_editor;

pub use client::{AiAttachment, AiChunk, AiClient, AiError};
pub use models::{list_models, ModelList, ModelListError, ModelSource};
pub use streaming_editor::{run_stream, strip_code_fence, truncate_for_msg, EditEvent};
pub use providers::{
    AiProvider, AiProviderConfig, AiProviderError, ClaudeCliClient, GeminiCliClient,
    LmStudioClient,
};

#[cfg(feature = "test-util")]
pub use providers::FakeAiClient;

#[cfg(all(test, feature = "test-util"))]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[tokio::test]
    async fn fake_client_emits_canned_chunks_then_done() {
        let client = FakeAiClient::new(vec!["hello", " ", "world"]);
        let mut stream = client.complete("ignored".into(), vec![]);

        let mut got = Vec::new();
        while let Some(item) = stream.next().await {
            got.push(item.expect("no errors"));
        }

        assert_eq!(
            got,
            vec![
                AiChunk::Delta("hello".into()),
                AiChunk::Delta(" ".into()),
                AiChunk::Delta("world".into()),
                AiChunk::Done {
                    full_text: "hello world".into(),
                },
            ]
        );
    }

    #[tokio::test]
    async fn fake_client_with_error_yields_protocol_error() {
        let client = FakeAiClient::failing("kaboom");
        let mut stream = client.complete("ignored".into(), vec![]);

        let first = stream.next().await.expect("one item");
        assert!(matches!(first, Err(AiError::Protocol(ref m)) if m == "kaboom"));
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn fake_client_records_prompts() {
        let client = FakeAiClient::new(vec!["x"]);
        let _ = client
            .complete("first".into(), vec![])
            .collect::<Vec<_>>()
            .await;
        let _ = client
            .complete("second".into(), vec![])
            .collect::<Vec<_>>()
            .await;

        assert_eq!(client.prompts(), vec!["first", "second"]);
    }

    #[tokio::test]
    async fn fake_client_records_attachments() {
        let client = FakeAiClient::new(vec!["ok"]);
        let atts = vec![
            AiAttachment::image("/tmp/a.png", "image/png", "a.png"),
            AiAttachment::image("/tmp/b.jpg", "image/jpeg", "b.jpg"),
        ];
        let _ = client
            .complete("p".into(), atts.clone())
            .collect::<Vec<_>>()
            .await;

        assert_eq!(client.attachments(), vec![atts]);
    }
}
