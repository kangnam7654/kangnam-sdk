use std::pin::Pin;
use std::sync::Arc;

use futures::stream::BoxStream;
use kangnam_harness_core::ToolCtx;
use kangnam_harness_llm_tool_runner::LlmAgent;
use kangnam_router::{
    ChatMessage, LlmError, LlmProviderDyn, LlmRequestOptions, LlmResponse, LlmStreamEvent,
    ProviderCapabilities,
};
use serde_json::Value;

use crate::guard::{
    assistant_turn_count, normalize_history, redact_pii, safety_rejection, validate_request,
};
use crate::persona::build_system_prompt;
use crate::tools::consult_tools;
use crate::types::{
    ConsultCapabilities, ConsultConfig, ConsultError, ConsultRequest, ConsultResponse,
};

const MAX_TURNS_REACHED: &str = "세션의 최대 대화 횟수에 도달했습니다. 새로운 세션을 시작해주세요.";

pub struct AiConsultSession {
    provider: Arc<dyn LlmProviderDyn>,
    config: ConsultConfig,
}

impl AiConsultSession {
    pub fn new(provider: Box<dyn LlmProviderDyn>) -> Self {
        let provider: Arc<dyn LlmProviderDyn> = provider.into();
        Self {
            provider,
            config: ConsultConfig::default(),
        }
    }

    #[must_use]
    pub fn with_config(mut self, config: ConsultConfig) -> Self {
        self.config = config;
        self
    }

    pub async fn respond(&self, request: ConsultRequest) -> Result<ConsultResponse, ConsultError> {
        validate_request(&request, &self.config)?;

        let (redacted_message, redacted) = redact_pii(&request.user_message);

        if assistant_turn_count(&request.history) >= self.config.max_turns_per_session {
            return Ok(ConsultResponse {
                text: MAX_TURNS_REACHED.to_string(),
                messages: Vec::new(),
                tool_invocations: Vec::new(),
                redacted,
            });
        }

        if let Some(rejection) = safety_rejection(&redacted_message) {
            return Ok(ConsultResponse {
                text: rejection,
                messages: Vec::new(),
                tool_invocations: Vec::new(),
                redacted,
            });
        }

        let messages = normalize_history(
            &request.history,
            redacted_message,
            self.config.max_history_messages,
        );
        if messages.is_empty() {
            return Err(ConsultError::InvalidHistory(
                "normalized history must not be empty".into(),
            ));
        }

        let capabilities = ConsultCapabilities {
            birth_profile: request.birth_profile.clone(),
        };
        let ctx = ToolCtx::new(request.session_id, capabilities);

        let mut agent = LlmAgent::new(Box::new(SharedProvider(self.provider.clone())), ctx)
            .with_system_prompt(build_system_prompt(request.birth_profile.as_ref()))
            .with_max_iterations(self.config.max_agent_iterations);
        if let Some(max_context_tokens) = self.config.context_window_tokens {
            agent = agent.with_context_window_tokens(max_context_tokens);
        }
        for (tool, description) in consult_tools() {
            agent = agent.with_boxed_tool(tool, description);
        }

        let run = agent.run_messages(messages).await?;
        Ok(ConsultResponse {
            text: run.final_text,
            messages: run.messages,
            tool_invocations: run.tool_invocations,
            redacted,
        })
    }
}

struct SharedProvider(Arc<dyn LlmProviderDyn>);

impl LlmProviderDyn for SharedProvider {
    fn provider_key(&self) -> &'static str {
        self.0.provider_key()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.0.capabilities()
    }

    fn context_window_tokens(&self) -> Option<usize> {
        self.0.context_window_tokens()
    }

    fn render_dyn(
        &self,
        system_prompt: &str,
        user_input: &str,
        result_json: &Value,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + '_>> {
        self.0.render_dyn(system_prompt, user_input, result_json)
    }

    fn chat_dyn(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
        result_json: &Value,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + '_>> {
        self.0.chat_dyn(system_prompt, messages, result_json)
    }

    fn chat_stream_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        result_json: &'a Value,
    ) -> BoxStream<'a, LlmStreamEvent> {
        self.0.chat_stream_dyn(system_prompt, messages, result_json)
    }

    fn chat_with_options_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        options: &'a LlmRequestOptions,
        result_json: &'a Value,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + 'a>> {
        self.0
            .chat_with_options_dyn(system_prompt, messages, options, result_json)
    }

    fn chat_stream_with_options_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        options: &'a LlmRequestOptions,
        result_json: &'a Value,
    ) -> BoxStream<'a, LlmStreamEvent> {
        self.0
            .chat_stream_with_options_dyn(system_prompt, messages, options, result_json)
    }
}

#[cfg(test)]
mod tests {
    use kangnam_harness_llm_tool_runner::test_util::{MockLlmProvider, Step};
    use serde_json::json;

    use super::*;
    use crate::types::{BirthProfile, ConsultMessage, ConsultRole};

    fn profile() -> BirthProfile {
        BirthProfile {
            birth_date: "1990-05-15".into(),
            birth_time: Some("14:30".into()),
            calendar_type: Some("solar".into()),
            gender: None,
        }
    }

    fn request(message: &str) -> ConsultRequest {
        ConsultRequest {
            session_id: "consult-test".into(),
            user_message: message.into(),
            history: vec![],
            birth_profile: Some(profile()),
        }
    }

    #[tokio::test]
    async fn respond_returns_terminal_text() {
        let mock = MockLlmProvider::new(vec![Step::text("차분히 살펴볼게요.")]);
        let session = AiConsultSession::new(Box::new(mock));
        let response = session.respond(request("연애운이 궁금해요")).await.unwrap();

        assert_eq!(response.text, "차분히 살펴볼게요.");
        assert!(!response.redacted);
        assert_eq!(response.messages.len(), 2);
    }

    #[tokio::test]
    async fn safety_guard_does_not_call_llm() {
        let mock = MockLlmProvider::new(vec![Step::text("unused")]);
        let observer = mock.clone();
        let session = AiConsultSession::new(Box::new(mock));
        let response = session.respond(request("죽고 싶어요")).await.unwrap();

        assert!(response.text.contains("전문가의 도움"));
        assert!(observer.observed().is_empty());
    }

    #[tokio::test]
    async fn max_turn_guard_does_not_call_llm() {
        let mock = MockLlmProvider::new(vec![Step::text("unused")]);
        let observer = mock.clone();
        let session = AiConsultSession::new(Box::new(mock)).with_config(ConsultConfig {
            max_turns_per_session: 1,
            ..ConsultConfig::default()
        });
        let mut req = request("다음 질문");
        req.history = vec![ConsultMessage {
            role: ConsultRole::Assistant,
            text: "이미 답변".into(),
        }];

        let response = session.respond(req).await.unwrap();
        assert!(response.text.contains("최대 대화 횟수"));
        assert!(observer.observed().is_empty());
    }

    #[tokio::test]
    async fn redacts_before_llm_request() {
        let mock = MockLlmProvider::new(vec![Step::text("확인했어요.")]);
        let observer = mock.clone();
        let session = AiConsultSession::new(Box::new(mock));
        let response = session
            .respond(request("제 번호는 010-1234-5678 입니다"))
            .await
            .unwrap();

        assert!(response.redacted);
        let observed = observer.observed();
        let sent = observed[0].messages.last().unwrap().text_content();
        assert!(sent.contains("[전화번호 삭제]"));
        assert!(!sent.contains("010"));
    }

    #[tokio::test]
    async fn dispatches_saju_tool_then_returns_final_text() {
        let mock = MockLlmProvider::new(vec![
            Step::tool_call("call_saju", "consult_saju_context", json!({})),
            Step::text("일간 흐름을 바탕으로 답변드릴게요."),
        ]);
        let session = AiConsultSession::new(Box::new(mock));
        let response = session
            .respond(request("제 성향이 궁금해요"))
            .await
            .unwrap();

        assert_eq!(response.text, "일간 흐름을 바탕으로 답변드릴게요.");
        assert_eq!(response.tool_invocations.len(), 1);
        assert_eq!(
            response.tool_invocations[0].call.name,
            "consult_saju_context"
        );
        assert!(!response.tool_invocations[0].is_error);
    }

    #[tokio::test]
    async fn dispatches_tarot_tool_then_returns_final_text() {
        let mock = MockLlmProvider::new(vec![
            Step::tool_call(
                "call_tarot",
                "consult_tarot_draw",
                json!({"reading_type": "tarot_one"}),
            ),
            Step::text("카드 흐름을 참고하면 지금은 신중함이 좋아요."),
        ]);
        let session = AiConsultSession::new(Box::new(mock));
        let response = session
            .respond(request("오늘 선택을 해도 될까요"))
            .await
            .unwrap();

        assert_eq!(response.tool_invocations.len(), 1);
        assert_eq!(response.tool_invocations[0].call.name, "consult_tarot_draw");
        assert!(response.text.contains("신중함"));
    }

    #[tokio::test]
    async fn long_history_is_compacted_before_llm_request() {
        let mock = MockLlmProvider::new(vec![Step::text("요약해서 이어갈게요.")]);
        let observer = mock.clone();
        let session = AiConsultSession::new(Box::new(mock)).with_config(ConsultConfig {
            max_history_messages: 10,
            context_window_tokens: Some(260),
            ..ConsultConfig::default()
        });
        let mut req = request("최근 질문은 유지해주세요");
        req.history = vec![
            ConsultMessage {
                role: ConsultRole::User,
                text: "오래된 사용자 이야기 ".repeat(200),
            },
            ConsultMessage {
                role: ConsultRole::Assistant,
                text: "오래된 답변 ".repeat(200),
            },
            ConsultMessage {
                role: ConsultRole::User,
                text: "중간 질문".into(),
            },
            ConsultMessage {
                role: ConsultRole::Assistant,
                text: "중간 답변".into(),
            },
        ];

        let response = session.respond(req).await.unwrap();

        assert_eq!(response.text, "요약해서 이어갈게요.");
        let observed = observer.observed();
        assert_eq!(observed.len(), 1);
        assert!(
            observed[0].messages[0]
                .text_content()
                .contains("compressed")
        );
        assert_eq!(
            observed[0].messages.last().unwrap().text_content(),
            "최근 질문은 유지해주세요"
        );
    }
}
