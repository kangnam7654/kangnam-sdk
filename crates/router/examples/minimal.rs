//! Round-trip a single chat turn through the `dummy` provider
//! (no credentials, no network required).
use kangnam_router::{ChatMessage, create_provider};
use serde_json::json;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = create_provider("dummy", "", "", "")?;
    let messages = vec![ChatMessage::user("hello from minimal example")];
    let resp = provider
        .chat_dyn("you are a helpful assistant", &messages, &json!({}))
        .await?;
    println!("model={}", resp.model);
    println!("cost_usd={}", resp.estimated_cost_usd);
    println!("response={}", resp.rendered_text);
    Ok(())
}
