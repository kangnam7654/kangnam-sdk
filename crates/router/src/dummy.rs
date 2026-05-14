use std::time::Duration;

use futures::stream::BoxStream;
use serde_json::Value;

use super::{LlmProviderDyn, LlmResponse, LlmStreamEvent};

pub struct DummyLlmProvider;

impl LlmProviderDyn for DummyLlmProvider {
    fn provider_key(&self) -> &'static str {
        "dummy"
    }

    fn capabilities(&self) -> crate::ProviderCapabilities {
        crate::ProviderCapabilities::dummy()
    }

    fn render_dyn(
        &self,
        _system_prompt: &str,
        _user_input: &str,
        result_json: &Value,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<LlmResponse, crate::LlmError>> + Send + '_>,
    > {
        let summary = result_json
            .get("advice")
            .and_then(|v| v.as_str())
            .unwrap_or("오늘의 운세입니다.");
        let score = result_json
            .get("overall")
            .and_then(|v| v.get("score"))
            .and_then(|v| v.as_i64())
            .unwrap_or(75);

        let text =
            format!("오늘의 종합 운세 점수는 {score}점입니다.\n\n{summary}\n\n좋은 하루 되세요!");

        Box::pin(async move {
            Ok(LlmResponse {
                rendered_text: text,
                model: "dummy-v1".into(),
                ..Default::default()
            })
        })
    }

    fn chat_stream_dyn<'a>(
        &'a self,
        _system_prompt: &'a str,
        _messages: &'a [super::ChatMessage],
        result_json: &'a Value,
    ) -> BoxStream<'a, LlmStreamEvent> {
        let summary = result_json
            .get("advice")
            .and_then(|v| v.as_str())
            .unwrap_or("오늘의 운세입니다.")
            .to_string();
        let score = result_json
            .get("overall")
            .and_then(|v| v.get("score"))
            .and_then(|v| v.as_i64())
            .unwrap_or(75);

        let full_text =
            format!("오늘의 종합 운세 점수는 {score}점입니다.\n\n{summary}\n\n좋은 하루 되세요!");

        Box::pin(async_stream::stream! {
            let chars: Vec<char> = full_text.chars().collect();
            let mut i = 0;
            while i < chars.len() {
                let end = (i + 5).min(chars.len());
                let chunk: String = chars[i..end].iter().collect();
                i = end;
                yield LlmStreamEvent::Delta { text: chunk };
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let rendered = full_text.clone();
            yield LlmStreamEvent::End {
                total: LlmResponse {
                    rendered_text: rendered,
                    model: "dummy-v1".into(),
                    ..Default::default()
                },
            };
        })
    }
}

pub fn make(
    _api_key: &str,
    _model: &str,
    _base_url: &str,
) -> Result<Box<dyn crate::LlmProviderDyn>, crate::LlmError> {
    Ok(Box::new(DummyLlmProvider))
}

pub async fn list_models(_api_key: &str) -> Result<Vec<crate::ListModel>, crate::LlmError> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;

    #[tokio::test]
    async fn test_stream_chunks() {
        let provider = DummyLlmProvider;
        let result_json = serde_json::json!({
            "advice": "테스트 운세",
            "overall": { "score": 80 }
        });
        let mut stream = provider.chat_stream_dyn("", &[], &result_json);

        let mut delta_texts = Vec::new();
        let mut end_text = None;

        while let Some(event) = stream.next().await {
            match event {
                LlmStreamEvent::Delta { text } => {
                    assert!(!text.is_empty(), "delta chunk must not be empty");
                    assert!(text.chars().count() <= 5, "delta chunk must be <= 5 chars");
                    delta_texts.push(text);
                }
                LlmStreamEvent::End { total } => {
                    end_text = Some(total.rendered_text);
                }
                LlmStreamEvent::Error { message } => {
                    panic!("Unexpected error from DummyLlmProvider stream: {message}");
                }
                _ => {}
            }
        }

        assert!(!delta_texts.is_empty(), "must have at least one delta");
        let joined = delta_texts.join("");
        let end = end_text.expect("must have End event");
        assert_eq!(joined, end, "joined deltas must match End.rendered_text");
        assert!(end.contains("80점"), "End text must contain score");
    }
}
