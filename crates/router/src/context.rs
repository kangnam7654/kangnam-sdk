use crate::{ChatContent, ChatMessage, ImageSource, ListModel, LlmError};

const APPROX_CHARS_PER_TOKEN: usize = 4;
const MESSAGE_OVERHEAD_TOKENS: usize = 4;
const IMAGE_PLACEHOLDER_TOKENS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextWindowBudget {
    pub max_context_tokens: usize,
    pub reserve_output_tokens: usize,
    pub min_recent_messages: usize,
    pub max_summary_tokens: usize,
}

impl ContextWindowBudget {
    pub fn new(max_context_tokens: usize) -> Self {
        Self {
            max_context_tokens,
            reserve_output_tokens: 1024,
            min_recent_messages: 4,
            max_summary_tokens: 768,
        }
    }

    fn input_budget(&self) -> usize {
        self.max_context_tokens
            .saturating_sub(self.reserve_output_tokens)
            .max(1)
    }
}

pub async fn resolve_model_context_window_tokens(
    provider: &str,
    api_key: &str,
    model: &str,
    base_url: &str,
) -> Result<Option<usize>, LlmError> {
    let models = crate::list_models_with_base_url(provider, api_key, base_url).await?;
    Ok(find_model_context_window_tokens(&models, model))
}

pub fn find_model_context_window_tokens(models: &[ListModel], model: &str) -> Option<usize> {
    let needle = normalize_model_name(model);
    models
        .iter()
        .find(|candidate| {
            let name = normalize_model_name(&candidate.name);
            let display = normalize_model_name(&candidate.display_name);
            !needle.is_empty() && (name == needle || display == needle)
        })
        .or_else(|| {
            models
                .iter()
                .find(|candidate| candidate.input_token_limit.is_some())
        })
        .and_then(|model| model.input_token_limit)
        .and_then(|limit| usize::try_from(limit).ok())
}

#[derive(Debug, Clone)]
pub struct ContextCompactionResult {
    pub messages: Vec<ChatMessage>,
    pub original_tokens: usize,
    pub compacted_tokens: usize,
    pub compacted: bool,
}

pub fn estimate_request_tokens(system_prompt: &str, messages: &[ChatMessage]) -> usize {
    estimate_text_tokens(system_prompt) + estimate_messages_tokens(messages)
}

pub fn estimate_messages_tokens(messages: &[ChatMessage]) -> usize {
    messages
        .iter()
        .map(|message| {
            MESSAGE_OVERHEAD_TOKENS
                + estimate_text_tokens(&message.role)
                + message
                    .content
                    .iter()
                    .map(estimate_content_tokens)
                    .sum::<usize>()
        })
        .sum()
}

pub fn compact_messages_for_window(
    system_prompt: &str,
    messages: &[ChatMessage],
    budget: &ContextWindowBudget,
) -> ContextCompactionResult {
    let original_tokens = estimate_request_tokens(system_prompt, messages);
    let input_budget = budget.input_budget();
    if original_tokens <= input_budget || messages.len() <= 1 {
        return ContextCompactionResult {
            messages: messages.to_vec(),
            original_tokens,
            compacted_tokens: original_tokens,
            compacted: false,
        };
    }

    let tail_count = budget
        .min_recent_messages
        .min(messages.len().saturating_sub(1))
        .max(1);
    let mut tail_start = messages.len().saturating_sub(tail_count);

    if tail_starts_with_tool_result(messages, tail_start) {
        tail_start = tail_start.saturating_sub(1);
    }

    if tail_start == 0 {
        return ContextCompactionResult {
            messages: messages.to_vec(),
            original_tokens,
            compacted_tokens: original_tokens,
            compacted: false,
        };
    }

    let tail = &messages[tail_start..];
    let tail_tokens = estimate_request_tokens(system_prompt, tail);
    let summary_budget = budget
        .max_summary_tokens
        .min(input_budget.saturating_sub(tail_tokens).max(64));
    let summary = build_compaction_summary(&messages[..tail_start], summary_budget);

    let mut compacted_messages = Vec::with_capacity(tail.len() + 1);
    compacted_messages.push(ChatMessage::user(summary));
    compacted_messages.extend_from_slice(tail);

    let compacted_tokens = estimate_request_tokens(system_prompt, &compacted_messages);
    ContextCompactionResult {
        messages: compacted_messages,
        original_tokens,
        compacted_tokens,
        compacted: true,
    }
}

fn estimate_content_tokens(content: &ChatContent) -> usize {
    match content {
        ChatContent::Text(text) => estimate_text_tokens(text),
        ChatContent::Image { source, mime_type } => {
            IMAGE_PLACEHOLDER_TOKENS
                + estimate_text_tokens(mime_type)
                + match source {
                    ImageSource::Base64(data) => data.len() / 1024,
                    ImageSource::Url(url) => estimate_text_tokens(url),
                }
        }
        ChatContent::ToolResult {
            tool_use_id,
            content,
            ..
        } => estimate_text_tokens(tool_use_id) + estimate_text_tokens(content),
        ChatContent::ToolUse {
            id,
            name,
            arguments,
        } => {
            estimate_text_tokens(id)
                + estimate_text_tokens(name)
                + estimate_text_tokens(&arguments.to_string())
        }
    }
}

fn normalize_model_name(model: &str) -> String {
    model
        .trim()
        .trim_start_matches("models/")
        .to_ascii_lowercase()
}

fn estimate_text_tokens(text: &str) -> usize {
    let chars = text.chars().count();
    if chars == 0 {
        0
    } else {
        chars.div_ceil(APPROX_CHARS_PER_TOKEN).max(1)
    }
}

fn tail_starts_with_tool_result(messages: &[ChatMessage], tail_start: usize) -> bool {
    tail_start > 0
        && messages
            .get(tail_start)
            .is_some_and(message_contains_tool_result)
        && messages
            .get(tail_start - 1)
            .is_some_and(message_contains_tool_use)
}

fn message_contains_tool_result(message: &ChatMessage) -> bool {
    message
        .content
        .iter()
        .any(|content| matches!(content, ChatContent::ToolResult { .. }))
}

fn message_contains_tool_use(message: &ChatMessage) -> bool {
    message
        .content
        .iter()
        .any(|content| matches!(content, ChatContent::ToolUse { .. }))
}

fn build_compaction_summary(messages: &[ChatMessage], max_tokens: usize) -> String {
    let mut summary = String::from(
        "Earlier conversation was compressed to fit the model context window. Use this as background only.\n",
    );
    for (idx, message) in messages.iter().enumerate() {
        summary.push_str("- ");
        summary.push_str(&(idx + 1).to_string());
        summary.push_str(" ");
        summary.push_str(&message.role);
        summary.push_str(": ");
        summary.push_str(&message_preview(message));
        summary.push('\n');
    }
    truncate_to_estimated_tokens(&summary, max_tokens)
}

fn message_preview(message: &ChatMessage) -> String {
    let parts = message
        .content
        .iter()
        .map(content_preview)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    truncate_chars(&parts.join(" | "), 320)
}

fn content_preview(content: &ChatContent) -> String {
    match content {
        ChatContent::Text(text) => normalize_ws(text),
        ChatContent::Image { mime_type, .. } => format!("<image:{mime_type}>"),
        ChatContent::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => format!(
            "<tool_result id={tool_use_id} error={is_error}> {}",
            normalize_ws(content)
        ),
        ChatContent::ToolUse {
            id,
            name,
            arguments,
        } => format!("<tool_call id={id} name={name}> {}", arguments),
    }
}

fn normalize_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_to_estimated_tokens(text: &str, max_tokens: usize) -> String {
    truncate_chars(text, max_tokens.saturating_mul(APPROX_CHARS_PER_TOKEN))
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let keep = max_chars.saturating_sub(15);
    let mut out = text.chars().take(keep).collect::<String>();
    out.push_str("\n[truncated]");
    out
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{ChatMessage, ListModel, ToolCall};

    #[test]
    fn compact_messages_keeps_recent_tail_and_summarizes_older_history() {
        let messages = vec![
            ChatMessage::user("old user ".repeat(200)),
            ChatMessage::assistant("old assistant ".repeat(200)),
            ChatMessage::user("recent question"),
        ];
        let budget = ContextWindowBudget {
            max_context_tokens: 180,
            reserve_output_tokens: 20,
            min_recent_messages: 1,
            max_summary_tokens: 40,
        };

        let result = compact_messages_for_window("", &messages, &budget);

        assert!(result.compacted);
        assert!(result.compacted_tokens < result.original_tokens);
        assert_eq!(result.messages.len(), 2);
        assert!(result.messages[0].text_content().contains("compressed"));
        assert_eq!(result.messages[1].text_content(), "recent question");
    }

    #[test]
    fn compact_messages_keeps_tool_use_with_matching_tool_result() {
        let calls = vec![ToolCall {
            id: "call-1".into(),
            name: "lookup".into(),
            arguments: json!({"q": "value"}),
        }];
        let messages = vec![
            ChatMessage::user("old context ".repeat(200)),
            ChatMessage::assistant_with_tool_calls("", calls),
            ChatMessage::tool_result("call-1", "tool answer", false),
        ];
        let budget = ContextWindowBudget {
            max_context_tokens: 140,
            reserve_output_tokens: 20,
            min_recent_messages: 1,
            max_summary_tokens: 32,
        };

        let result = compact_messages_for_window("", &messages, &budget);

        assert!(result.compacted);
        assert_eq!(result.messages.len(), 3);
        assert!(message_contains_tool_use(&result.messages[1]));
        assert!(message_contains_tool_result(&result.messages[2]));
    }

    #[test]
    fn finds_context_window_for_matching_model() {
        let models = vec![
            ListModel {
                name: "gpt-small".into(),
                display_name: "GPT Small".into(),
                description: None,
                input_token_limit: Some(8_000),
                output_token_limit: None,
                default_reasoning_level: None,
                supported_reasoning_levels: Vec::new(),
            },
            ListModel {
                name: "models/gpt-large".into(),
                display_name: "GPT Large".into(),
                description: None,
                input_token_limit: Some(128_000),
                output_token_limit: None,
                default_reasoning_level: None,
                supported_reasoning_levels: Vec::new(),
            },
        ];

        assert_eq!(
            find_model_context_window_tokens(&models, "gpt-large"),
            Some(128_000)
        );
    }
}
