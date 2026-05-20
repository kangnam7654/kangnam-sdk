//! Round-trip a single chat turn through the `dummy` provider
//! (no credentials, no network required).
use kangnam_router::{ChatMessage, ProviderConfig, RouteRequest, Router};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let router = Router::new().with_provider("default", ProviderConfig::new("dummy", "", "", ""));
    let request = RouteRequest::chat(vec![ChatMessage::user("hello from minimal example")])
        .with_system_prompt("you are a helpful assistant");
    let resp = router.chat(request).await?;

    println!("model={}", resp.model);
    println!("cost_usd={}", resp.estimated_cost_usd);
    println!("response={}", resp.rendered_text);
    Ok(())
}
