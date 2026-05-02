//! Demonstrates live cost display via Usage events during streaming.
//!
//! Run with the `dummy` provider (no credentials needed):
//!   cargo run --example streaming_usage
//!
//! Real Usage events come from:
//!   - claude: progressive (every 50 output tokens)
//!   - gemini: per-chunk (50-token throttle)
//!   - codex / openai_compat / copilot: single event before End

use futures::StreamExt;
use kangnam_router::{ChatMessage, LlmStreamEvent, create_provider};
use serde_json::json;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = create_provider("dummy", "", "", "")?;
    let messages = vec![ChatMessage::user("Write a long answer please.")];

    let result_json = json!({});
    let mut stream = provider.chat_stream_dyn("", &messages, &result_json);

    while let Some(event) = stream.next().await {
        match event {
            LlmStreamEvent::Delta { text } => {
                print!("{text}");
            }
            LlmStreamEvent::Usage {
                output_tokens,
                estimated_cost_usd,
                ..
            } => {
                eprint!(
                    "\r\x1b[33m[live: {output_tokens} tokens, ${estimated_cost_usd:.4}]\x1b[0m"
                );
            }
            LlmStreamEvent::End { total } => {
                println!();
                println!(
                    "\n[final: {} input + {} output tokens, ${:.4}]",
                    total.input_tokens.unwrap_or(0),
                    total.output_tokens.unwrap_or(0),
                    total.estimated_cost_usd
                );
            }
            LlmStreamEvent::Error { message } => {
                eprintln!("[error] {message}");
            }
            _ => {}
        }
    }
    Ok(())
}
