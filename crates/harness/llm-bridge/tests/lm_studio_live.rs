//! Layer 3 — Live LM Studio integration. Marked `#[ignore]` so it does
//! NOT run on default `cargo test`; opt-in by setting the
//! `LMSTUDIO_BASE_URL` env var (e.g. `http://localhost:1234/v1`) and
//! running:
//!
//! ```text
//! LMSTUDIO_BASE_URL=http://localhost:1234/v1 \
//! LMSTUDIO_MODEL=qwen2.5-7b-instruct \
//! cargo test -p kangnam-harness-llm-bridge --features test-util \
//!   --test lm_studio_live -- --ignored
//! ```
//!
//! The test asks the model "What's 7 times 6?" and registers a
//! `multiply` tool. A correct LM Studio + tool-capable model will
//! emit a tool_call → bridge dispatches → final answer mentions 42.
//!
//! If `LMSTUDIO_BASE_URL` is unset, the test skips early so it is
//! safe to run against live infrastructure that may not always be up.

use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use kangnam_harness_llm_bridge::LlmAgent;
use kangnam_harness_runtime::{
    AgentTool, DefaultCapabilities, FsCallbacks, ImageCallbacks, InteractionBridge, ToolCtx,
    ToolError, ToolResult, WebCallbacks,
};
use kangnam_router::create_provider;

struct NopFs;
#[async_trait]
impl FsCallbacks for NopFs {
    async fn read(&self, _: &Path) -> Result<Vec<u8>, ToolError> {
        Ok(vec![])
    }
    async fn write(&self, _: &Path, _: &[u8]) -> Result<(), ToolError> {
        Ok(())
    }
    async fn str_replace(&self, _: &Path, _: &str, _: &str) -> Result<(), ToolError> {
        Ok(())
    }
}
struct NopWeb;
#[async_trait]
impl WebCallbacks for NopWeb {
    async fn fetch(&self, _: &str) -> Result<Vec<u8>, ToolError> {
        Ok(vec![])
    }
}
struct NopImage;
#[async_trait]
impl ImageCallbacks for NopImage {
    async fn generate(&self, _: &str, p: &Path) -> Result<std::path::PathBuf, ToolError> {
        Ok(p.to_path_buf())
    }
}
struct NopBridge;
#[async_trait]
impl InteractionBridge for NopBridge {
    async fn register_question_form(
        &self,
        _: &Value,
    ) -> Result<(String, oneshot::Receiver<Value>), ToolError> {
        let (_tx, rx) = oneshot::channel();
        Ok(("await-x".into(), rx))
    }
    async fn register_preview(
        &self,
        _: &Value,
    ) -> Result<(String, oneshot::Receiver<Value>), ToolError> {
        let (_tx, rx) = oneshot::channel();
        Ok(("await-y".into(), rx))
    }
}

fn make_ctx() -> ToolCtx {
    ToolCtx::new(
        "live-test",
        DefaultCapabilities {
            fs: Arc::new(NopFs),
            web: Arc::new(NopWeb),
            image: Some(Arc::new(NopImage)),
            bridge: Arc::new(NopBridge),
        },
    )
}

struct Multiply {
    log: Arc<Mutex<Vec<(f64, f64)>>>,
}
#[async_trait]
impl AgentTool for Multiply {
    fn name(&self) -> &str {
        "multiply"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "a": {"type": "number", "description": "first operand"},
                "b": {"type": "number", "description": "second operand"}
            },
            "required": ["a", "b"]
        })
    }
    async fn execute(&self, params: Value, _: &ToolCtx) -> ToolResult {
        let a = params["a"].as_f64().unwrap_or(0.0);
        let b = params["b"].as_f64().unwrap_or(0.0);
        self.log.lock().unwrap().push((a, b));
        ToolResult::Success {
            content: json!({"product": a * b}),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires running LM Studio with LMSTUDIO_BASE_URL env var"]
async fn live_lm_studio_invokes_multiply_tool() {
    let base_url = match std::env::var("LMSTUDIO_BASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!("skipping: LMSTUDIO_BASE_URL not set");
            return;
        }
    };
    let model = std::env::var("LMSTUDIO_MODEL").unwrap_or_else(|_| "default".to_string());

    let provider = create_provider("openai_compat", "", &model, &base_url)
        .expect("provider construction should succeed");

    let log = Arc::new(Mutex::new(Vec::new()));
    let agent = LlmAgent::new(provider, make_ctx())
        .with_tool(
            Multiply {
                log: Arc::clone(&log),
            },
            "Multiply two numbers and return their product.",
        )
        .with_system_prompt(
            "You are a helpful math assistant. \
             ALWAYS use the multiply tool for multiplication; never compute it yourself.",
        )
        .with_max_iterations(4);

    let run = agent
        .run("What's 7 times 6? Use the multiply tool.")
        .await
        .expect("LM Studio multi-turn loop should complete");

    eprintln!("--- live run ---");
    eprintln!("iterations: {}", run.iterations);
    eprintln!("final_text: {}", run.final_text);
    eprintln!("invocations: {:#?}", run.tool_invocations);

    // The model must have invoked multiply at least once with 7 and 6
    // (in either order, since "7 times 6" is commutative).
    let calls = log.lock().unwrap();
    let saw_seven_six = calls
        .iter()
        .any(|(a, b)| (a == &7.0 && b == &6.0) || (a == &6.0 && b == &7.0));
    assert!(
        saw_seven_six,
        "expected multiply(7, 6) or multiply(6, 7); got {:?}",
        *calls
    );

    // The final answer should mention 42 somewhere — we don't strict-
    // match the exact phrasing because models vary.
    assert!(
        run.final_text.contains("42"),
        "final answer should include the product 42; got: {}",
        run.final_text
    );
}
