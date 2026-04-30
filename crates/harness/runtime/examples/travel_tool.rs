//! Travel Planner integration sketch — shows how a non-design domain
//! plugs into `kangnam-harness-runtime` with its own capability bundle.
//!
//! Run: `cargo run --example travel_tool -p kangnam-harness-runtime`
//!
//! This example is **illustrative only** — the providers are stubs that
//! return canned data. A production Travel Planner would back them with
//! real APIs (Booking.com / Naver Map / OpenAI tool use / etc).

use std::sync::Arc;

use async_trait::async_trait;
use kangnam_harness_runtime::{
    AgentTool, AwaitKind, InteractionBridge, ToolCtx, ToolError, ToolResult,
};
use serde_json::{json, Value};
use tokio::sync::oneshot;

// ── 1. Domain capability traits ──────────────────────────────────────────

#[async_trait]
trait WebSearch: Send + Sync {
    async fn search(&self, query: &str) -> Result<Vec<String>, ToolError>;
}

#[async_trait]
trait MapProvider: Send + Sync {
    async fn deep_link(&self, place: &str) -> Result<String, ToolError>;
}

#[async_trait]
trait AccommodationProvider: Send + Sync {
    async fn search(
        &self,
        city: &str,
        budget_band: &str,
    ) -> Result<Vec<AccommodationOption>, ToolError>;
}

#[derive(Debug, Clone, serde::Serialize)]
struct AccommodationOption {
    name: String,
    nightly_krw: u32,
    location: String,
}

// ── 2. The capability bundle Travel Planner passes to ToolCtx ───────────

#[derive(Clone)]
struct TravelCapabilities {
    web: Arc<dyn WebSearch>,
    map: Arc<dyn MapProvider>,
    accommodation: Arc<dyn AccommodationProvider>,
    interaction: Arc<dyn InteractionBridge>,
}

// ── 3. A tool that uses the bundle ──────────────────────────────────────

struct AccommodationSearchTool;

#[async_trait]
impl AgentTool<TravelCapabilities> for AccommodationSearchTool {
    fn name(&self) -> &str {
        "travel.accommodation_search"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "required": ["city"],
            "properties": {
                "city": { "type": "string" },
                "budget_band": {
                    "type": "string",
                    "enum": ["budget", "mid", "upper_mid"]
                }
            }
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolCtx<TravelCapabilities>) -> ToolResult {
        let city = match params.get("city").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return ToolResult::Failed { error: "missing `city`".into() },
        };
        let budget_band = params
            .get("budget_band")
            .and_then(|v| v.as_str())
            .unwrap_or("mid")
            .to_string();

        let options = match ctx
            .capabilities
            .accommodation
            .search(&city, &budget_band)
            .await
        {
            Ok(opts) => opts,
            Err(e) => return ToolResult::Failed { error: format!("accommodation search: {e}") },
        };

        ToolResult::Success {
            content: json!({
                "city": city,
                "budget_band": budget_band,
                "options": options,
            }),
        }
    }
}

/// Tool that suspends the agent turn waiting for the user to pick one
/// of the accommodation options. Demonstrates `AwaitKind::Selection` —
/// added to `kangnam-harness-runtime` specifically for cases like this.
struct PickAccommodationTool;

#[async_trait]
impl AgentTool<TravelCapabilities> for PickAccommodationTool {
    fn name(&self) -> &str {
        "travel.accommodation_pick"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "required": ["options"],
            "properties": {
                "options": { "type": "array" }
            }
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolCtx<TravelCapabilities>) -> ToolResult {
        match ctx
            .capabilities
            .interaction
            .register_await(AwaitKind::Selection, &params)
            .await
        {
            Ok((await_id, receiver)) => ToolResult::AwaitUser {
                await_id,
                kind: AwaitKind::Selection,
                payload: params,
                receiver,
            },
            Err(e) => ToolResult::Failed { error: format!("interaction: {e}") },
        }
    }
}

// ── 4. Stub provider implementations for the example ────────────────────

struct StubWeb;
#[async_trait]
impl WebSearch for StubWeb {
    async fn search(&self, query: &str) -> Result<Vec<String>, ToolError> {
        Ok(vec![format!("https://example.com/?q={query}")])
    }
}

struct StubMap;
#[async_trait]
impl MapProvider for StubMap {
    async fn deep_link(&self, place: &str) -> Result<String, ToolError> {
        Ok(format!("https://map.example.com/?place={place}"))
    }
}

struct StubAccommodation;
#[async_trait]
impl AccommodationProvider for StubAccommodation {
    async fn search(
        &self,
        city: &str,
        budget_band: &str,
    ) -> Result<Vec<AccommodationOption>, ToolError> {
        Ok(vec![
            AccommodationOption {
                name: format!("{city} Sky Hotel"),
                nightly_krw: match budget_band {
                    "budget" => 80_000,
                    "upper_mid" => 320_000,
                    _ => 180_000,
                },
                location: format!("{city} downtown"),
            },
            AccommodationOption {
                name: format!("{city} Riverside Inn"),
                nightly_krw: match budget_band {
                    "budget" => 70_000,
                    "upper_mid" => 280_000,
                    _ => 150_000,
                },
                location: format!("{city} riverside"),
            },
        ])
    }
}

/// In-memory bridge — the production version would post to chat-rpc
/// and resolve via the host's PendingAwaits map.
struct InMemoryBridge;
#[async_trait]
impl InteractionBridge for InMemoryBridge {
    async fn register_await(
        &self,
        kind: AwaitKind,
        _payload: &Value,
    ) -> Result<(String, oneshot::Receiver<Value>), ToolError> {
        let (tx, rx) = oneshot::channel();
        // Auto-resolve immediately so the example finishes; a real
        // host would wait for a user response.
        let canned = json!({"kind": format!("{:?}", kind), "picked": 0});
        let _ = tx.send(canned);
        Ok((format!("await-{:?}-1", kind), rx))
    }

    async fn register_question_form(
        &self,
        payload: &Value,
    ) -> Result<(String, oneshot::Receiver<Value>), ToolError> {
        self.register_await(AwaitKind::QuestionForm, payload).await
    }

    async fn register_preview(
        &self,
        payload: &Value,
    ) -> Result<(String, oneshot::Receiver<Value>), ToolError> {
        self.register_await(AwaitKind::Preview, payload).await
    }
}

// ── 5. Driver ───────────────────────────────────────────────────────────

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let caps = TravelCapabilities {
        web: Arc::new(StubWeb),
        map: Arc::new(StubMap),
        accommodation: Arc::new(StubAccommodation),
        interaction: Arc::new(InMemoryBridge),
    };
    let ctx = ToolCtx::new("demo-session", caps);

    // 1. Search
    let search_tool = AccommodationSearchTool;
    let result = search_tool
        .execute(json!({"city": "Seoul", "budget_band": "mid"}), &ctx)
        .await;
    println!("search → {:?}", result);
    let options = match result {
        ToolResult::Success { content } => content,
        other => panic!("search failed: {other:?}"),
    };

    // 2. Pick — suspends turn, but our InMemoryBridge auto-resolves
    let pick_tool = PickAccommodationTool;
    let pick = pick_tool.execute(options.clone(), &ctx).await;
    println!("pick → {:?}", pick);
    if let ToolResult::AwaitUser { receiver, .. } = pick {
        let user_response = receiver.await.expect("bridge should send");
        println!("user response → {}", user_response);
    }

    // 3. Map deep link
    let link = ctx
        .capabilities
        .map
        .deep_link("Seoul Sky Hotel")
        .await
        .expect("map link");
    println!("map → {}", link);
}
