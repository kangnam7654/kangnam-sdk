//! Orchestrates AI-driven `SiteDoc` generation for canvas-web projects.
//!
//! This mirrors [`super::canvas_generator::CanvasGenerator`] (which
//! streams `Deck` generation for slides projects) but targets the
//! web-mode prompt template (`site-system-prompt.tera`) and parses the
//! response as a [`SiteDoc`] instead of a `Deck`.
//!
//! The generator is deliberately thin — it wires the Tera template, the
//! `AiClient` stream, and the SiteDoc parser together. Handler-level
//! concerns (persistence, override composition, stream-event translation)
//! live in `handlers/canvas.rs`.

use std::sync::Arc;

use futures::stream::{BoxStream, StreamExt};
use tera::{Context, Tera};
use thiserror::Error;

use kangnam_design_llm::{AiAttachment, AiChunk, AiClient, AiError};
use kangnam_design_doc_site::SiteDoc;

const TEMPLATE_NAME: &str = "site-system-prompt";
const TEMPLATE_SOURCE: &str =
    include_str!("../templates/site-system-prompt.tera");

/// Events yielded by [`SiteGenerator::generate`] during a single
/// generation turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiteGenerationEvent {
    /// Raw text delta from the AI — forwarded unchanged so the frontend
    /// can show incremental JSON as it streams.
    Delta(String),
    /// Terminal success event. Carries the rendered HTML (ready to feed
    /// into the preview iframe) and the `SiteDoc` JSON (persisted into
    /// `versions.site_doc_json`).
    Complete {
        html: String,
        site_doc_json: String,
    },
}

#[derive(Debug, Error)]
pub enum SiteGenerationError {
    #[error(transparent)]
    Ai(#[from] AiError),
    #[error("could not render site system prompt: {0}")]
    PromptRender(String),
    #[error("AI returned text that is not a valid SiteDoc: {0}")]
    InvalidSiteDoc(String),
}

pub struct SiteGenerator {
    ai: Arc<dyn AiClient>,
    tera: Tera,
}

impl SiteGenerator {
    pub fn new(ai: Arc<dyn AiClient>) -> Result<Self, SiteGenerationError> {
        let mut tera = Tera::default();
        tera.add_raw_template(TEMPLATE_NAME, TEMPLATE_SOURCE)
            .map_err(|e| SiteGenerationError::PromptRender(e.to_string()))?;
        Ok(Self { ai, tera })
    }

    pub fn build_prompt(
        &self,
        user_prompt: &str,
        attachments: &[AiAttachment],
    ) -> Result<String, SiteGenerationError> {
        let mut ctx = Context::new();
        ctx.insert("user_prompt", user_prompt);
        ctx.insert("references", &!attachments.is_empty());
        self.tera
            .render(TEMPLATE_NAME, &ctx)
            .map_err(|e| SiteGenerationError::PromptRender(e.to_string()))
    }

    pub fn generate(
        &self,
        user_prompt: String,
    ) -> BoxStream<'static, Result<SiteGenerationEvent, SiteGenerationError>> {
        self.generate_with_attachments(user_prompt, Vec::new())
    }

    pub fn generate_with_attachments(
        &self,
        user_prompt: String,
        attachments: Vec<AiAttachment>,
    ) -> BoxStream<'static, Result<SiteGenerationEvent, SiteGenerationError>> {
        let prompt = match self.build_prompt(&user_prompt, &attachments) {
            Ok(p) => p,
            Err(e) => return futures::stream::iter(vec![Err(e)]).boxed(),
        };

        let upstream = self.ai.complete(prompt, attachments);
        let stream = async_stream::stream! {
            let mut upstream = upstream;
            let mut full_text = String::new();
            while let Some(item) = upstream.next().await {
                match item {
                    Ok(AiChunk::Delta(t)) => {
                        full_text.push_str(&t);
                        yield Ok(SiteGenerationEvent::Delta(t));
                    }
                    Ok(AiChunk::Done { full_text: ai_full }) => {
                        let raw = if ai_full.is_empty() {
                            full_text.clone()
                        } else {
                            ai_full
                        };
                        match parse_site(&raw) {
                            Ok((site, site_json)) => {
                                let html = site.to_html();
                                yield Ok(SiteGenerationEvent::Complete {
                                    html,
                                    site_doc_json: site_json,
                                });
                            }
                            Err(e) => {
                                yield Err(SiteGenerationError::InvalidSiteDoc(e));
                            }
                        }
                        return;
                    }
                    Err(e) => {
                        yield Err(SiteGenerationError::Ai(e));
                        return;
                    }
                }
            }
        };
        stream.boxed()
    }
}

/// Parse the raw AI response into a `SiteDoc` + its serialized JSON.
/// Strips markdown code fences, validates non-empty `sections[]`, and
/// stamps a server-generated `id` if the AI omitted one so callers can
/// trust uniqueness.
fn parse_site(raw: &str) -> Result<(SiteDoc, String), String> {
    let cleaned = strip_code_fence(raw.trim());
    let mut site: SiteDoc = serde_json::from_str(cleaned)
        .map_err(|e| format!("{e} :: {}", truncate(cleaned, 200)))?;
    if site.sections.is_empty() {
        return Err(format!(
            "sections[] is empty :: {}",
            truncate(cleaned, 200)
        ));
    }
    // Server-generated id so distinct generations can't collide on the
    // same string; section ids remain as authored.
    if site.id.is_empty() || site.id == "site-<slug>" {
        site.id = format!("site-{}", uuid::Uuid::new_v4());
    }
    let json = serde_json::to_string(&site).map_err(|e| e.to_string())?;
    Ok((site, json))
}

fn strip_code_fence(s: &str) -> &str {
    let s = s.trim();
    let stripped = s
        .strip_prefix("```json")
        .or_else(|| s.strip_prefix("```"));
    let s = stripped.unwrap_or(s);
    let s = s.trim_start_matches('\n').trim();
    s.strip_suffix("```").map(str::trim).unwrap_or(s)
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kangnam_design_llm::FakeAiClient;

    const MIN_SITE_JSON: &str = r##"{
        "id": "site-x",
        "title": "Landing",
        "sections": [
            {
                "id": "hero-1",
                "width_px": 1280,
                "height_px": 720,
                "title": "Hero",
                "background": { "kind": "color", "color": "#ffffff" },
                "elements": [
                    {
                        "kind": "text",
                        "id": "headline",
                        "frame": { "x": 0, "y": 0, "w": 1280, "h": 200 },
                        "content": "<h1 class=\"text-6xl font-bold\">Welcome</h1>",
                        "style": {
                            "font_family": "Pretendard",
                            "font_size_px": 48,
                            "font_weight": 700,
                            "color": "#111827",
                            "line_height": 1.2,
                            "letter_spacing_px": -0.5,
                            "align": "center"
                        }
                    }
                ]
            }
        ]
    }"##;

    #[tokio::test]
    async fn build_prompt_includes_user_text_and_omits_references_block_when_empty() {
        let ai = Arc::new(FakeAiClient::new(Vec::<String>::new()));
        let svc = SiteGenerator::new(ai).unwrap();
        let prompt = svc.build_prompt("음식 배달 앱 랜딩", &[]).unwrap();
        assert!(prompt.contains("음식 배달 앱 랜딩"));
        assert!(prompt.contains("Tailwind"));
        // `references` flag is false → conditional block absent.
        assert!(!prompt.contains("레퍼런스의 톤"));
    }

    #[tokio::test]
    async fn build_prompt_includes_references_block_when_attachments_present() {
        let ai = Arc::new(FakeAiClient::new(Vec::<String>::new()));
        let svc = SiteGenerator::new(ai).unwrap();
        let atts = vec![AiAttachment::image(
            "/tmp/ref.png",
            "image/png",
            "ref.png",
        )];
        let prompt = svc.build_prompt("랜딩", &atts).unwrap();
        assert!(prompt.contains("레퍼런스의 톤"));
    }

    #[tokio::test]
    async fn generate_streams_deltas_then_completes_with_html_and_json() {
        let ai = Arc::new(FakeAiClient::new(vec![MIN_SITE_JSON.to_string()]));
        let svc = SiteGenerator::new(ai).unwrap();

        let mut stream = svc.generate("랜딩 만들어줘".into());
        let mut deltas = Vec::<String>::new();
        let mut complete = None;
        while let Some(item) = stream.next().await {
            match item.unwrap() {
                SiteGenerationEvent::Delta(s) => deltas.push(s),
                ev @ SiteGenerationEvent::Complete { .. } => {
                    complete = Some(ev);
                    break;
                }
            }
        }

        assert_eq!(deltas.len(), 1);
        let complete = complete.expect("must complete");
        let SiteGenerationEvent::Complete { html, site_doc_json } = complete
        else {
            unreachable!()
        };
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains(r#"data-section-id="hero-1""#));
        assert!(html.contains("Welcome"));
        // JSON should be re-serialized and include the hero section.
        assert!(site_doc_json.contains("hero-1"));
    }

    #[tokio::test]
    async fn generate_rejects_non_json_response() {
        let ai = Arc::new(FakeAiClient::new(vec!["not json".to_string()]));
        let svc = SiteGenerator::new(ai).unwrap();

        let mut stream = svc.generate("x".into());
        let mut err: Option<SiteGenerationError> = None;
        while let Some(item) = stream.next().await {
            if let Err(e) = item {
                err = Some(e);
                break;
            }
        }
        let err = err.expect("must error");
        assert!(matches!(err, SiteGenerationError::InvalidSiteDoc(_)));
    }

    #[tokio::test]
    async fn generate_rejects_empty_sections_array() {
        let empty_site = r##"{"id":"site-x","sections":[]}"##;
        let ai = Arc::new(FakeAiClient::new(vec![empty_site.to_string()]));
        let svc = SiteGenerator::new(ai).unwrap();

        let mut stream = svc.generate("x".into());
        let mut err: Option<SiteGenerationError> = None;
        while let Some(item) = stream.next().await {
            if let Err(e) = item {
                err = Some(e);
                break;
            }
        }
        assert!(matches!(err, Some(SiteGenerationError::InvalidSiteDoc(_))));
    }

    #[tokio::test]
    async fn generate_strips_markdown_code_fences() {
        let fenced = format!("```json\n{MIN_SITE_JSON}\n```");
        let ai = Arc::new(FakeAiClient::new(vec![fenced]));
        let svc = SiteGenerator::new(ai).unwrap();

        let mut stream = svc.generate("x".into());
        let mut ok = false;
        while let Some(item) = stream.next().await {
            if let Ok(SiteGenerationEvent::Complete { .. }) = item {
                ok = true;
                break;
            }
        }
        assert!(ok, "expected Complete event after fence stripping");
    }

    #[tokio::test]
    async fn generate_assigns_server_id_when_ai_leaves_placeholder() {
        // `parse_site` should replace the placeholder "site-<slug>" with a
        // real server-generated id so two generations can't collide.
        let placeholder_site = MIN_SITE_JSON.replace("site-x", "site-<slug>");
        let ai = Arc::new(FakeAiClient::new(vec![placeholder_site]));
        let svc = SiteGenerator::new(ai).unwrap();

        let mut stream = svc.generate("x".into());
        let mut site_json: Option<String> = None;
        while let Some(item) = stream.next().await {
            if let Ok(SiteGenerationEvent::Complete { site_doc_json, .. }) = item {
                site_json = Some(site_doc_json);
                break;
            }
        }
        let site_json = site_json.unwrap();
        assert!(!site_json.contains("site-<slug>"));
        assert!(site_json.contains("\"site-"));
    }

    #[tokio::test]
    async fn generate_surfaces_ai_errors() {
        let ai = Arc::new(FakeAiClient::failing("boom"));
        let svc = SiteGenerator::new(ai).unwrap();
        let mut stream = svc.generate("x".into());
        let mut err: Option<SiteGenerationError> = None;
        while let Some(item) = stream.next().await {
            if let Err(e) = item {
                err = Some(e);
                break;
            }
        }
        assert!(matches!(err, Some(SiteGenerationError::Ai(_))));
    }
}
