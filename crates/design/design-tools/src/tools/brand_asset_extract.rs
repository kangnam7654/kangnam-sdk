//! `brand_asset_extract` — multi-step brand asset capture (huashu-design
//! 5-step protocol synthesised onto our runtime).
//!
//! Steps the tool takes per call:
//! 1. Fetch the brand's URL via `WebCallbacks::fetch`.
//! 2. Grep the body for `#hex` and CSS-named colors; collect uniques.
//! 3. Heuristically pick a primary + accent (most-common, second-most).
//! 4. Write a brief `brand-spec.md` into the workspace summarising
//!    locate / palette / vocalize sections so the agent can reference
//!    `brand-spec.md` on later turns.
//!
//! The output is intentionally a starting point — a human or a later
//! agent turn refines voice / typography / negative-space cues. The
//! tool's value is that it does the boring fetch + grep so the agent
//! doesn't hallucinate a hex code.

use async_trait::async_trait;
use kangnam_harness_runtime::{AgentTool, ToolCtx, ToolResult};
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct BrandAssetExtractTool;

#[async_trait]
impl AgentTool for BrandAssetExtractTool {
    fn name(&self) -> &str { "brand_asset_extract" }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "required": ["brand", "url"],
            "properties": {
                "brand": { "type": "string", "description": "Brand name." },
                "url": { "type": "string", "description": "Source page (homepage / brand book)." },
                "out": { "type": "string", "description": "Output path inside working_dir. Defaults to `brand-spec.md`." }
            }
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolCtx) -> ToolResult {
        let brand = match params.get("brand").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return ToolResult::Failed { error: "missing `brand`".into() },
        };
        let url = match params.get("url").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return ToolResult::Failed { error: "missing `url`".into() },
        };
        let out_rel = params
            .get("out")
            .and_then(|v| v.as_str())
            .unwrap_or("brand-spec.md");

        let body_bytes = match ctx.capabilities.web.fetch(&url).await {
            Ok(b) => b,
            Err(e) => return ToolResult::Failed { error: format!("fetch failed: {e}") },
        };
        let body = String::from_utf8_lossy(&body_bytes);
        let palette = extract_palette(&body);

        let primary = palette
            .first()
            .cloned()
            .unwrap_or_else(|| "#000000".into());
        let accent = palette.get(1).cloned().unwrap_or_else(|| "#888888".into());

        let spec = format!(
            "# brand-spec — {brand}\n\
             \n\
             > Auto-extracted by `brand_asset_extract` from {url}.\n\
             > Treat as a starting point; refine voice / typography / negative-space cues by hand.\n\
             \n\
             ## Palette (top hex matches)\n\
             - primary: `{primary}`\n\
             - accent:  `{accent}`\n\
             - others:  {others}\n\
             \n\
             ## Locate\n\
             - source: {url}\n\
             - extracted: {count} unique hex colors\n\
             \n\
             ## Vocalize\n\
             - When using {brand} colors, prefer the primary as a UI accent and the accent as a hover / emphasis state. Never drop a purple→pink hero gradient.\n",
            brand = brand,
            url = url,
            primary = primary,
            accent = accent,
            others = palette
                .iter()
                .skip(2)
                .take(6)
                .map(|c| format!("`{c}`"))
                .collect::<Vec<_>>()
                .join(" "),
            count = palette.len(),
        );

        let abs = match ctx.resolve_path(out_rel) {
            Some(p) => p,
            None => return ToolResult::Failed {
                error: "brand_asset_extract requires a working directory".into(),
            },
        };
        if let Err(e) = ctx.capabilities.fs.write(&abs, spec.as_bytes()).await {
            return ToolResult::Failed { error: format!("write {} failed: {e}", abs.display()) };
        }

        ToolResult::Success {
            content: json!({
                "brand": brand,
                "spec_path": abs.display().to_string(),
                "primary": primary,
                "accent": accent,
                "palette": palette,
            }),
        }
    }
}

fn extract_palette(body: &str) -> Vec<String> {
    let re = Regex::new(r"#([0-9a-fA-F]{6}|[0-9a-fA-F]{3})\b").unwrap();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for cap in re.captures_iter(body) {
        let raw = cap.get(0).unwrap().as_str().to_lowercase();
        // Drop trivial neutrals — they pollute the palette ranking.
        if matches!(raw.as_str(), "#000" | "#fff" | "#000000" | "#ffffff") {
            continue;
        }
        *counts.entry(raw).or_insert(0) += 1;
    }
    let mut ordered: Vec<(String, usize)> = counts.into_iter().collect();
    ordered.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ordered.into_iter().map(|(c, _)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_ctx_with_web_workspace;
    use serde_json::json;

    #[tokio::test]
    async fn extracts_palette_and_writes_spec() {
        let body = b"<style>.a{color:#ff0033}.b{color:#ff0033}.c{color:#1234aa}</style>".to_vec();
        let (ctx, ws) = test_ctx_with_web_workspace(body);
        let tool = BrandAssetExtractTool;
        let res = tool.execute(
            json!({"brand":"Acme","url":"https://example.com"}),
            &ctx,
        ).await;
        match res {
            ToolResult::Success { content } => {
                assert_eq!(content["primary"], "#ff0033");
                assert_eq!(content["accent"], "#1234aa");
            }
            other => panic!("expected Success, got {other:?}"),
        }
        let spec = std::fs::read_to_string(ws.path().join("brand-spec.md")).unwrap();
        assert!(spec.contains("primary: `#ff0033`"));
    }

    #[test]
    fn extract_palette_strips_neutrals() {
        let p = extract_palette("#fff #fff #ff0033 #000000 #ff0033 #aa11bb");
        assert_eq!(p[0], "#ff0033");
        assert_eq!(p[1], "#aa11bb");
        assert!(!p.contains(&"#fff".to_string()));
    }
}
