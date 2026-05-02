//! Demonstrates Anthropic prompt caching for 90% cost reduction.
//!
//! Run with the `dummy` provider (no credentials needed):
//!   cargo run --example prompt_caching
//!
//! For real caching:
//!   - Set provider to "claude" with ANTHROPIC_API_KEY
//!   - cache_breakpoints work only on the claude HTTP provider
//!   - Other providers silently ignore the option

use kangnam_router::{CacheBreakpoint, ChatMessage, LlmRequestOptions, create_provider};
use serde_json::json;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = create_provider("dummy", "", "claude-sonnet-4-5", "")?;

    // Long, stable system prompt — perfect candidate for caching
    let long_system = "You are an expert assistant. ".repeat(500);

    // RAG context — also stable across multiple turns
    let rag_context = format!(
        "Document context:\n{}",
        "Lorem ipsum dolor sit amet, ".repeat(2000)
    );

    let messages = vec![
        ChatMessage::user(rag_context),
        ChatMessage::user("What's in this document?"),
    ];

    let opts = LlmRequestOptions {
        cache_breakpoints: vec![
            CacheBreakpoint::System,          // cache long system prompt
            CacheBreakpoint::MessageIndex(0), // cache RAG context
        ],
        ..Default::default()
    };

    let resp = provider
        .chat_with_options_dyn(&long_system, &messages, &opts, &json!({}))
        .await?;

    println!("{}", resp.rendered_text);
    println!();
    println!("--- Cache stats ---");
    println!(
        "Cache creation tokens: {:?}",
        resp.cache_creation_input_tokens
    );
    println!("Cache read tokens: {:?}", resp.cache_read_input_tokens);
    println!("Cost (USD): ${:.6}", resp.estimated_cost_usd);
    Ok(())
}
