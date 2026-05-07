//! Shared streaming-editor loop extracted from `design-editor-html` and
//! `design-editor-slide`.
//!
//! Both editors follow the same protocol:
//!   1. Build a prompt (caller's responsibility — uses Tera, stays in the
//!      concrete crate).
//!   2. Call [`AiClient::complete`] and forward every `Delta` chunk to the
//!      caller's event stream.
//!   3. On `Done`, assemble the full text, strip markdown fences, and pass
//!      the result to a caller-supplied *parse* closure.
//!   4. Yield either `Complete(output)` or propagate the parse error.
//!
//! [`run_stream`] implements steps 2–4 generically. Steps 1 and the
//! definition of concrete event enums remain in each editor crate.

use futures::stream::{BoxStream, StreamExt};

use crate::client::{AiChunk, AiError};

/// Generic streaming event emitted by [`run_stream`].
///
/// - `Delta` — raw AI text fragment; forward to the UI for incremental
///   progress display.
/// - `Complete` — terminal event carrying the parsed output.
///
/// Concrete editor crates typically map this to their own public event
/// enum via `.map()` so callers keep stable, named-field types.
#[derive(Debug, Clone, PartialEq)]
pub enum EditEvent<O> {
    Delta(String),
    Complete(O),
}

/// Drive an AI completion stream to completion, forwarding deltas and
/// invoking `parse` on the assembled full text.
///
/// # Type parameters
/// - `O` — the parsed output type (e.g. `String`, `(SlideDoc, String)`).
/// - `E` — the error type used by the calling editor.
///
/// # Arguments
/// - `upstream` — the raw chunk stream from [`AiClient::complete`].
/// - `parse` — closure that receives the fence-stripped, trimmed full text
///   and returns `Result<O, E>`. Called exactly once, after `Done`.
/// - `wrap_ai_err` — maps [`AiError`] into the calling editor's error type.
///
/// The returned stream yields zero or more `Ok(EditEvent::Delta(_))` items
/// followed by exactly one terminal item: either `Ok(EditEvent::Complete(_))`
/// or an `Err(E)`.
pub fn run_stream<O, E>(
    upstream: BoxStream<'static, Result<AiChunk, AiError>>,
    parse: impl FnOnce(&str) -> Result<O, E> + Send + 'static,
    wrap_ai_err: impl Fn(AiError) -> E + Send + 'static,
) -> BoxStream<'static, Result<EditEvent<O>, E>>
where
    O: Send + 'static,
    E: Send + 'static,
{
    let stream = async_stream::stream! {
        let mut upstream = upstream;
        let mut full_text = String::new();
        while let Some(item) = upstream.next().await {
            match item {
                Ok(AiChunk::Delta(t)) => {
                    full_text.push_str(&t);
                    yield Ok(EditEvent::Delta(t));
                }
                Ok(AiChunk::Done { full_text: ai_full }) => {
                    let raw = if ai_full.is_empty() { full_text } else { ai_full };
                    let cleaned = strip_code_fence(raw.trim()).trim();
                    match parse(cleaned) {
                        Ok(output) => yield Ok(EditEvent::Complete(output)),
                        Err(e) => yield Err(e),
                    }
                    return;
                }
                Err(e) => {
                    yield Err(wrap_ai_err(e));
                    return;
                }
            }
        }
    };
    stream.boxed()
}

/// Strip the most common markdown code-fence wrappers that models emit
/// despite prompt-level instructions not to.
///
/// Handles:
/// - `` ```html ``, `` ```json ``, bare ` ``` ` (opening tag)
/// - trailing `` ``` ``
///
/// Returns a sub-slice of the input — no allocation.
pub fn strip_code_fence(s: &str) -> &str {
    let s = s.trim();
    let stripped = s
        .strip_prefix("```html")
        .or_else(|| s.strip_prefix("```json"))
        .or_else(|| s.strip_prefix("```"));
    let s = stripped.unwrap_or(s);
    let s = s.trim_start_matches('\n').trim();
    s.strip_suffix("```").map(str::trim).unwrap_or(s)
}

/// Truncate `s` to at most `max` bytes for use in error messages, appending
/// a byte-count suffix when the string was cut.
pub fn truncate_for_msg(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}… (+{} more bytes)", &s[..max], s.len() - max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_html_fence() {
        let s = "```html\n<div>hi</div>\n```";
        assert_eq!(strip_code_fence(s), "<div>hi</div>");
    }

    #[test]
    fn strip_json_fence() {
        let s = "```json\n{}\n```";
        assert_eq!(strip_code_fence(s), "{}");
    }

    #[test]
    fn strip_bare_fence() {
        let s = "```\nhello\n```";
        assert_eq!(strip_code_fence(s), "hello");
    }

    #[test]
    fn no_fence_passes_through() {
        let s = "<div>raw</div>";
        assert_eq!(strip_code_fence(s), s);
    }

    #[test]
    fn truncate_short() {
        assert_eq!(truncate_for_msg("abc", 10), "abc");
    }

    #[test]
    fn truncate_long() {
        let long = "a".repeat(300);
        let msg = truncate_for_msg(&long, 200);
        assert!(msg.starts_with(&"a".repeat(200)));
        assert!(msg.contains("+100 more bytes"));
    }
}
