//! Demonstrates separating Thinking events from Delta events.
//!
//! Run with the `dummy` provider (no credentials needed):
//!   cargo run --example thinking
//!
//! For real Thinking events, replace the provider:
//!   - claude: requires ANTHROPIC_API_KEY, set thinking_budget_tokens
//!   - codex: requires OPENAI_API_KEY, set reasoning_effort or thinking_budget_tokens
//!   - gemini: requires GEMINI_API_KEY, set reasoning_effort to "low"|"medium"|"high"
//!   - openai_compat: any o-series model returning reasoning_content

use futures::StreamExt;
use kangnam_router::{ChatMessage, LlmRequestOptions, LlmStreamEvent, create_provider};
use serde_json::json;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = create_provider("dummy", "", "", "")?;
    let opts = LlmRequestOptions {
        thinking_budget_tokens: Some(8192),    // claude only
        reasoning_effort: Some("high".into()), // codex / gemini / openai_compat
        ..Default::default()
    };
    let messages = vec![ChatMessage::user("Why is the sky blue?")];

    let result_json = json!({});
    let mut stream = provider.chat_stream_with_options_dyn("", &messages, &opts, &result_json);

    while let Some(event) = stream.next().await {
        match event {
            LlmStreamEvent::Thinking { text } => {
                eprint!("\x1b[90m[thinking] {text}\x1b[0m");
            }
            LlmStreamEvent::Delta { text } => {
                print!("{text}");
            }
            LlmStreamEvent::End { total } => {
                println!();
                if let Some(t) = &total.thinking_text {
                    eprintln!("\n[total thinking: {} chars]", t.len());
                }
            }
            LlmStreamEvent::Error { message } => {
                eprintln!("[error] {message}");
            }
            _ => {}
        }
    }
    Ok(())
}
