//! Structured-output helpers for host applications.
//!
//! Providers expose reasoning/thinking separately where possible, but local and
//! OpenAI-compatible models can still wrap JSON in fences, prepend prose, or leak
//! `<think>...</think>` blocks into ordinary text. These helpers intentionally
//! operate on the visible/final text (`LlmResponse::rendered_text`) and never on
//! `thinking_text`.

use serde::de::DeserializeOwned;
use thiserror::Error;

use crate::{LlmResponse, LlmStreamEvent};

const MAX_JSON_CANDIDATES: usize = 8;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StructuredOutputError {
    #[error("no JSON object found in model output")]
    NoJsonObject,
    #[error("failed to parse JSON object: {0}")]
    Json(String),
}

/// Remove balanced `<think>...</think>` blocks from text. Unbalanced blocks are
/// left intact so callers can reject/debug rather than accidentally dropping the
/// only visible answer.
pub fn strip_think_blocks(raw: &str) -> String {
    const OPEN: &str = "<think>";
    const CLOSE: &str = "</think>";
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;

    while let Some(open_idx) = rest.find(OPEN) {
        out.push_str(&rest[..open_idx]);
        let after_open = &rest[open_idx + OPEN.len()..];
        let Some(close_idx) = after_open.find(CLOSE) else {
            out.push_str(&rest[open_idx..]);
            return out;
        };
        rest = &after_open[close_idx + CLOSE.len()..];
    }
    out.push_str(rest);
    out
}

/// If the entire output is a Markdown code fence, return only its body.
/// Non-fenced text is returned unchanged.
pub fn strip_outer_code_fence(raw: &str) -> &str {
    let trimmed = raw.trim();
    if !trimmed.starts_with("```") {
        return raw;
    }

    let Some(first_newline) = trimmed.find('\n') else {
        return raw;
    };
    let Some(last_fence) = trimmed.rfind("```") else {
        return raw;
    };
    if last_fence <= first_newline {
        return raw;
    }
    trimmed[first_newline + 1..last_fence].trim()
}

/// Return the last balanced JSON-object candidates in `raw`, newest first.
/// Braces inside JSON strings are ignored.
pub fn json_object_candidates(raw: &str) -> Vec<String> {
    let cleaned = strip_think_blocks(raw);
    let cleaned = strip_outer_code_fence(&cleaned);

    let mut candidates: Vec<String> = Vec::new();
    let mut depth = 0usize;
    let mut start: Option<usize> = None;
    let mut in_string = false;
    let mut escape = false;

    for (idx, ch) in cleaned.char_indices() {
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start = Some(idx);
                }
                depth += 1;
            }
            '}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    if let Some(s) = start.take() {
                        let end = idx + ch.len_utf8();
                        candidates.push(cleaned[s..end].to_string());
                        if candidates.len() > MAX_JSON_CANDIDATES {
                            candidates.remove(0);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    candidates.into_iter().rev().collect()
}

/// Parse a structured JSON object from model-visible text.
///
/// This tolerates leading prose, markdown fences, balanced `<think>` blocks, and
/// extra earlier JSON fragments by trying the newest balanced object first.
pub fn parse_json_tolerant<T: DeserializeOwned>(raw: &str) -> Result<T, StructuredOutputError> {
    let candidates = json_object_candidates(raw);
    if candidates.is_empty() {
        return Err(StructuredOutputError::NoJsonObject);
    }

    let mut last_error = None;
    for candidate in candidates {
        match serde_json::from_str::<T>(&candidate) {
            Ok(parsed) => return Ok(parsed),
            Err(error) => last_error = Some(error.to_string()),
        }
    }

    Err(StructuredOutputError::Json(
        last_error.unwrap_or_else(|| "unknown JSON parse error".to_string()),
    ))
}

/// Parse structured JSON from the final visible response text. This never reads
/// `thinking_text`.
pub fn parse_response_json<T: DeserializeOwned>(
    response: &LlmResponse,
) -> Result<T, StructuredOutputError> {
    parse_json_tolerant(&response.rendered_text)
}

/// Collect only user-visible text from already-buffered stream events.
/// Reasoning/thinking, usage, tool calls, and errors are ignored.
pub fn visible_text_from_events<'a, I>(events: I) -> String
where
    I: IntoIterator<Item = &'a LlmStreamEvent>,
{
    let mut out = String::new();
    for event in events {
        match event {
            LlmStreamEvent::Delta { text } => out.push_str(text),
            LlmStreamEvent::End { total } if out.is_empty() => {
                out.push_str(&total.rendered_text);
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct Reply {
        reply: String,
    }

    #[test]
    fn parses_json_after_think_and_prose() {
        let raw = "<think>private reasoning</think>\nHere you go:\n```json\n{\"reply\":\"안녕하세요\"}\n```";
        let parsed: Reply = parse_json_tolerant(raw).unwrap();
        assert_eq!(parsed.reply, "안녕하세요");
    }

    #[test]
    fn prefers_last_balanced_object() {
        let raw = r#"draft {"reply": 1} final {"reply":"ok"}"#;
        let parsed: Reply = parse_json_tolerant(raw).unwrap();
        assert_eq!(parsed.reply, "ok");
    }

    #[test]
    fn ignores_braces_inside_strings() {
        let raw = r#"{"reply":"use {braces} literally"}"#;
        let parsed: Reply = parse_json_tolerant(raw).unwrap();
        assert_eq!(parsed.reply, "use {braces} literally");
    }

    #[test]
    fn response_json_ignores_thinking_text() {
        let response = LlmResponse {
            rendered_text: r#"{"reply":"visible"}"#.into(),
            thinking_text: Some(r#"{"reply":"hidden"}"#.into()),
            ..Default::default()
        };
        let parsed: Reply = parse_response_json(&response).unwrap();
        assert_eq!(parsed.reply, "visible");
    }

    #[test]
    fn visible_text_excludes_thinking_events() {
        let events = vec![
            LlmStreamEvent::Thinking {
                text: "secret".into(),
            },
            LlmStreamEvent::Delta {
                text: "hello".into(),
            },
            LlmStreamEvent::Usage {
                input_tokens: Some(1),
                output_tokens: 1,
                estimated_cost_usd: 0.0,
            },
            LlmStreamEvent::Delta {
                text: " world".into(),
            },
        ];
        assert_eq!(visible_text_from_events(&events), "hello world");
    }
}
