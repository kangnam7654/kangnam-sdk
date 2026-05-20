use futures::stream::BoxStream;
use serde_json::Value;
use std::process::Command;

use super::{
    ChatMessage, LlmError, LlmProviderDyn, LlmRequestOptions, LlmResponse, LlmStreamEvent,
    copilot::CopilotProvider,
};

const DEFAULT_MODEL: &str = "claude-sonnet-4.6";

#[derive(Clone)]
pub struct CopilotLocalProvider {
    token_override: Option<String>,
    model: String,
    base_url: Option<String>,
}

impl CopilotLocalProvider {
    pub fn new(token_override: String, model: String, base_url: String) -> Self {
        Self {
            token_override: if token_override.is_empty() {
                None
            } else {
                Some(token_override)
            },
            model: if model.is_empty() {
                DEFAULT_MODEL.to_string()
            } else {
                model
            },
            base_url: if base_url.is_empty() {
                None
            } else {
                Some(base_url)
            },
        }
    }

    fn create_inner(&self) -> Result<CopilotProvider, LlmError> {
        let token = match self.token_override.clone() {
            Some(token) => token,
            None => read_gh_auth_token()?,
        };

        Ok(match self.base_url.clone() {
            Some(base_url) => {
                CopilotProvider::new_with_base_url(token, self.model.clone(), base_url)
            }
            None => CopilotProvider::new(token, self.model.clone()),
        })
    }
}

impl LlmProviderDyn for CopilotLocalProvider {
    fn provider_key(&self) -> &'static str {
        "copilot_local"
    }

    fn capabilities(&self) -> crate::ProviderCapabilities {
        crate::ProviderCapabilities {
            kind: crate::ProviderKind::LocalCli,
            usage_support: crate::UsageSupport::None,
            supports_streaming: true,
            supports_tool_calling: false,
            supports_parallel_tool_calls: false,
            supports_image_input: false,
            supports_image_url: false,
            supports_reasoning_effort: false,
            supports_thinking_budget: false,
            supports_prompt_cache: false,
            supports_model_listing: false,
            supports_web_search: false,
            supports_local_read: false,
            estimates_cost: false,
        }
    }

    fn render_dyn(
        &self,
        system_prompt: &str,
        user_input: &str,
        result_json: &Value,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + '_>,
    > {
        let provider = self.clone();
        let system = system_prompt.to_string();
        let input = user_input.to_string();
        let json = result_json.clone();
        Box::pin(async move {
            let inner = provider.create_inner()?;
            inner
                .render_dyn(&system, &input, &json)
                .await
                .map_err(remap_copilot_error)
        })
    }

    fn chat_dyn(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
        result_json: &Value,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + '_>,
    > {
        let provider = self.clone();
        let system = system_prompt.to_string();
        let messages = messages.to_vec();
        let json = result_json.clone();
        Box::pin(async move {
            let inner = provider.create_inner()?;
            inner
                .chat_dyn(&system, &messages, &json)
                .await
                .map_err(remap_copilot_error)
        })
    }

    fn chat_with_options_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        options: &'a LlmRequestOptions,
        result_json: &'a Value,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<LlmResponse, LlmError>> + Send + 'a>,
    > {
        let provider = self.clone();
        let system = system_prompt.to_string();
        let messages = messages.to_vec();
        let options = options.clone();
        let json = result_json.clone();
        Box::pin(async move {
            let inner = provider.create_inner()?;
            inner
                .chat_with_options_dyn(&system, &messages, &options, &json)
                .await
                .map_err(remap_copilot_error)
        })
    }

    fn chat_stream_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        result_json: &'a Value,
    ) -> BoxStream<'a, LlmStreamEvent> {
        let provider = self.clone();
        let system = system_prompt.to_string();
        let messages = messages.to_vec();
        let json = result_json.clone();
        Box::pin(async_stream::stream! {
            match provider.create_inner() {
                Ok(inner) => {
                    let mut stream = inner.chat_stream_dyn(&system, &messages, &json);
                    while let Some(event) = futures::StreamExt::next(&mut stream).await {
                        yield remap_copilot_stream_event(event);
                    }
                }
                Err(err) => yield LlmStreamEvent::Error { message: err.to_string() },
            }
        })
    }

    fn chat_stream_with_options_dyn<'a>(
        &'a self,
        system_prompt: &'a str,
        messages: &'a [ChatMessage],
        options: &'a LlmRequestOptions,
        result_json: &'a Value,
    ) -> BoxStream<'a, LlmStreamEvent> {
        let provider = self.clone();
        let system = system_prompt.to_string();
        let messages = messages.to_vec();
        let options = options.clone();
        let json = result_json.clone();
        Box::pin(async_stream::stream! {
            match provider.create_inner() {
                Ok(inner) => {
                    let mut stream =
                        inner.chat_stream_with_options_dyn(&system, &messages, &options, &json);
                    while let Some(event) = futures::StreamExt::next(&mut stream).await {
                        yield remap_copilot_stream_event(event);
                    }
                }
                Err(err) => yield LlmStreamEvent::Error { message: err.to_string() },
            }
        })
    }
}

fn remap_copilot_stream_event(event: LlmStreamEvent) -> LlmStreamEvent {
    match event {
        LlmStreamEvent::Error { message } => LlmStreamEvent::Error {
            message: message.replace("provider 'copilot'", "provider 'copilot_local'"),
        },
        other => other,
    }
}

fn remap_copilot_error(error: LlmError) -> LlmError {
    match error {
        LlmError::MissingConfig { reason, .. } => LlmError::MissingConfig {
            provider: "copilot_local".into(),
            reason,
        },
        LlmError::Auth { .. } => LlmError::Auth {
            provider: "copilot_local".into(),
        },
        LlmError::RateLimit {
            retry_after_secs, ..
        } => LlmError::RateLimit {
            provider: "copilot_local".into(),
            retry_after_secs,
        },
        LlmError::Upstream { status, body, .. } => LlmError::Upstream {
            provider: "copilot_local".into(),
            status,
            body,
        },
        LlmError::Network { msg, .. } => LlmError::Network {
            provider: "copilot_local".into(),
            msg,
        },
        LlmError::Parse { reason, .. } => LlmError::Parse {
            provider: "copilot_local".into(),
            reason,
        },
        LlmError::Other { message, .. } => LlmError::Other {
            provider: "copilot_local".into(),
            message,
        },
    }
}

fn read_gh_auth_token() -> Result<String, LlmError> {
    let binary = crate::cli_utils::resolve_binary("gh");
    let output = Command::new(binary)
        .args(["auth", "token"])
        .env("PATH", crate::cli_utils::build_path_env())
        .output()
        .map_err(|e| LlmError::Other {
            provider: "copilot_local".into(),
            message: format!("failed to run gh auth token: {e}"),
        })?;

    if !output.status.success() {
        return Err(LlmError::Auth {
            provider: "copilot_local".into(),
        });
    }

    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        return Err(LlmError::Auth {
            provider: "copilot_local".into(),
        });
    }
    Ok(token)
}

pub fn make(
    api_key: &str,
    model: &str,
    base_url: &str,
) -> Result<Box<dyn LlmProviderDyn>, LlmError> {
    Ok(Box::new(CopilotLocalProvider::new(
        api_key.to_string(),
        model.to_string(),
        base_url.to_string(),
    )))
}

pub async fn list_models(_api_key: &str) -> Result<Vec<crate::ListModel>, LlmError> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_without_token_uses_local_provider_key() {
        let provider = make("", "", "").unwrap();

        assert_eq!(provider.provider_key(), "copilot_local");
    }

    #[test]
    fn constructor_preserves_token_override_model_and_base_url() {
        let provider = CopilotLocalProvider::new(
            "github_pat_test".into(),
            "gpt-4.1".into(),
            "http://localhost:9999/chat/completions".into(),
        );

        assert_eq!(provider.token_override.as_deref(), Some("github_pat_test"));
        assert_eq!(provider.model, "gpt-4.1");
        assert_eq!(
            provider.base_url.as_deref(),
            Some("http://localhost:9999/chat/completions")
        );
    }

    #[test]
    fn empty_model_uses_default() {
        let provider = CopilotLocalProvider::new("token".into(), "".into(), "".into());

        assert_eq!(provider.model, DEFAULT_MODEL);
    }

    #[test]
    fn remaps_inner_copilot_errors_to_local_provider() {
        let error = remap_copilot_error(LlmError::Auth {
            provider: "copilot".into(),
        });

        assert!(matches!(error, LlmError::Auth { provider } if provider == "copilot_local"));
    }

    #[test]
    fn remaps_stream_error_message_to_local_provider() {
        let event = remap_copilot_stream_event(LlmStreamEvent::Error {
            message: "authentication failed for provider 'copilot'".into(),
        });

        assert!(matches!(
            event,
            LlmStreamEvent::Error { message } if message.contains("provider 'copilot_local'")
        ));
    }
}
