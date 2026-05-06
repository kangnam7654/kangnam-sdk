//! Test utilities — scripted [`LlmProviderDyn`] for unit-testing
//! [`crate::LlmAgent`] loops without an HTTP server.
//!
//! Gated behind the `test-util` feature so production builds never
//! ship the test harness.
//!
//! # Why scripted?
//!
//! A real LLM is non-deterministic; a real HTTP server has setup cost.
//! For testing the bridge's loop logic — "given this sequence of model
//! responses, did dispatch happen in the right order with the right
//! arguments?" — a scripted provider is the highest-signal testbed.
//!
//! For wire-format correctness use the wiremock-based integration
//! tests in `tests/`. For end-to-end LM Studio verification use the
//! `#[ignore]`-gated live test.

use std::pin::Pin;
use std::sync::{Arc, Mutex};

use futures::stream::BoxStream;
use kangnam_router::{
    ChatMessage, LlmError, LlmProviderDyn, LlmRequestOptions, LlmResponse, ToolCall,
};
use serde_json::Value;

/// One scripted response in a `MockLlmProvider`'s queue.
///
/// The bridge calls `chat_with_options_dyn` once per iteration; each
/// `Step` produces the next response. Construct via [`Step::text`] or
/// [`Step::tool_call`].
#[derive(Debug, Clone)]
pub struct Step {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
}

impl Step {
    /// A terminal text-only assistant turn — bridge will treat this as
    /// the final answer and stop the loop.
    pub fn text(s: impl Into<String>) -> Self {
        Self {
            text: s.into(),
            tool_calls: Vec::new(),
        }
    }

    /// An assistant turn that emits exactly one tool call.
    pub fn tool_call(id: impl Into<String>, name: impl Into<String>, arguments: Value) -> Self {
        Self {
            text: String::new(),
            tool_calls: vec![ToolCall {
                id: id.into(),
                name: name.into(),
                arguments,
            }],
        }
    }

    /// An assistant turn that emits multiple tool calls in parallel.
    pub fn tool_calls(calls: Vec<ToolCall>) -> Self {
        Self {
            text: String::new(),
            tool_calls: calls,
        }
    }
}

/// Captures a single observed model request — what the bridge sent on
/// the wire. Tests assert on these to verify loop history shape.
#[derive(Debug, Clone)]
pub struct ObservedCall {
    pub system_prompt: String,
    pub messages: Vec<ChatMessage>,
    pub tool_count: usize,
}

/// Scripted [`LlmProviderDyn`] for unit tests.
///
/// Pops one [`Step`] from the front of its queue per call. Records
/// every request the bridge issues so tests can assert on history
/// shape and dispatch ordering.
///
/// State is stored behind `Arc<Mutex<…>>` so the mock is cheaply
/// cloneable: tests typically clone once *before* boxing the original
/// into [`crate::LlmAgent::new`], then inspect [`Self::observed`] on
/// the clone after `run()` returns.
///
/// Returns an `LlmError::Network` if called more times than there are
/// scripted steps — that's usually a sign the loop ran longer than the
/// test expected.
#[derive(Clone)]
pub struct MockLlmProvider {
    steps: Arc<Mutex<Vec<Step>>>,
    observed: Arc<Mutex<Vec<ObservedCall>>>,
}

impl MockLlmProvider {
    pub fn new(steps: Vec<Step>) -> Self {
        Self {
            steps: Arc::new(Mutex::new(steps)),
            observed: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Snapshot of every request the bridge has issued so far, in
    /// order.
    pub fn observed(&self) -> Vec<ObservedCall> {
        self.observed.lock().unwrap().clone()
    }

    /// Number of remaining scripted steps. `0` means the next call
    /// will return an exhausted-script error.
    pub fn remaining_steps(&self) -> usize {
        self.steps.lock().unwrap().len()
    }
}

impl MockLlmProvider {
    fn next_response(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
        tool_count: usize,
    ) -> Result<LlmResponse, LlmError> {
        self.observed.lock().unwrap().push(ObservedCall {
            system_prompt: system_prompt.to_string(),
            messages: messages.to_vec(),
            tool_count,
        });

        let mut steps = self.steps.lock().unwrap();
        if steps.is_empty() {
            return Err(LlmError::Network {
                provider: "mock".into(),
                msg: "MockLlmProvider script exhausted — bridge looped longer than expected"
                    .into(),
            });
        }
        let step = steps.remove(0);
        let mut resp = LlmResponse::default();
        resp.rendered_text = step.text;
        resp.model = "mock".into();
        resp.tool_calls = step.tool_calls;
        Ok(resp)
    }
}

impl LlmProviderDyn for MockLlmProvider {
    fn render_dyn(
        &self,
        _system_prompt: &str,
        _user_input: &str,
        _result_json: &Value,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + '_>>
    {
        Box::pin(async move {
            Err(LlmError::Network {
                provider: "mock".into(),
                msg: "MockLlmProvider does not implement render_dyn — use chat_with_options_dyn"
                    .into(),
            })
        })
    }

    fn chat_dyn(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
        _result_json: &Value,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + '_>>
    {
        // Lifetimes follow the trait's elided form: only `&self` outlives the
        // returned future. Snapshot the inputs into owned data so the future
        // doesn't borrow `system_prompt` / `messages`.
        let system_prompt = system_prompt.to_string();
        let messages = messages.to_vec();
        Box::pin(async move { self.next_response(&system_prompt, &messages, 0) })
    }

    fn chat_with_options_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        options: &'a LlmRequestOptions,
        _result_json: &'a Value,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + 'a>> {
        Box::pin(async move { self.next_response(system_prompt, messages, options.tools.len()) })
    }

    fn chat_stream_dyn<'a>(
        &'a self,
        _system_prompt: &'a str,
        _messages: &'a [ChatMessage],
        _result_json: &'a Value,
    ) -> BoxStream<'a, kangnam_router::LlmStreamEvent> {
        Box::pin(async_stream::stream! {
            yield kangnam_router::LlmStreamEvent::Error {
                message: "MockLlmProvider does not stream — exercise chat_with_options_dyn".into(),
            };
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn mock_provider_returns_scripted_text_step() {
        let mock = MockLlmProvider::new(vec![Step::text("hello")]);
        let resp = mock
            .chat_with_options_dyn(
                "sys",
                &[ChatMessage::user("hi")],
                &LlmRequestOptions::default(),
                &json!({}),
            )
            .await
            .unwrap();
        assert_eq!(resp.rendered_text, "hello");
        assert!(resp.tool_calls.is_empty());
    }

    #[tokio::test]
    async fn mock_provider_records_observed_calls() {
        let mock = MockLlmProvider::new(vec![Step::text("ok")]);
        let _ = mock
            .chat_with_options_dyn(
                "sysprompt",
                &[ChatMessage::user("u1")],
                &LlmRequestOptions::default(),
                &json!({}),
            )
            .await
            .unwrap();
        let obs = mock.observed();
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].system_prompt, "sysprompt");
        assert_eq!(obs[0].messages.len(), 1);
    }

    #[tokio::test]
    async fn mock_provider_errors_when_script_exhausted() {
        let mock = MockLlmProvider::new(vec![]);
        let err = mock
            .chat_with_options_dyn(
                "",
                &[],
                &LlmRequestOptions::default(),
                &json!({}),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, LlmError::Network { .. }));
    }
}
