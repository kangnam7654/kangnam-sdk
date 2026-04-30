use std::sync::Arc;

use futures::stream::{BoxStream, StreamExt};
use serde::Deserialize;
use tera::{Context, Tera};
use thiserror::Error;

use design_llm::{AiAttachment, AiChunk, AiClient, AiError};
use design_doc_slide::{Deck, SlideDoc, CANVAS_HEIGHT, CANVAS_WIDTH};

const TEMPLATE_NAME: &str = "deck-system-prompt";
const DECK_SYSTEM_PROMPT_TEMPLATE: &str =
    include_str!("../templates/deck-system-prompt.tera");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerationEvent {
    Delta(String),
    /// Terminal event of a successful generation. All three fields are
    /// derived from the same parsed `Deck`:
    /// - `html`     — full deck HTML (`<section>` per slide, scroll-snap
    ///   viewport). Served to the iframe preview and persisted.
    /// - `slide_doc_json` — **first slide only**, kept for v1 consumers
    ///   (PPTX v1 path, legacy frontend) that still expect a single-slide
    ///   payload. Callers on the v2 path should ignore this.
    /// - `deck_doc_json`  — full `Deck` JSON (`{"id","slides":[...]}`),
    ///   the v2 source of truth. Persisted into `versions.deck_doc_json`.
    Complete {
        html: String,
        slide_doc_json: String,
        deck_doc_json: String,
    },
}

#[derive(Debug, Error)]
pub enum GenerationError {
    #[error(transparent)]
    Ai(#[from] AiError),
    #[error("could not render system prompt: {0}")]
    PromptRender(String),
    #[error("AI returned text that is not a valid Deck: {0}")]
    InvalidSlideDoc(String),
}

pub struct CanvasGenerator {
    ai: Arc<dyn AiClient>,
    tera: Tera,
}

impl CanvasGenerator {
    pub fn new(ai: Arc<dyn AiClient>) -> Result<Self, GenerationError> {
        let mut tera = Tera::default();
        tera.add_raw_template(TEMPLATE_NAME, DECK_SYSTEM_PROMPT_TEMPLATE)
            .map_err(|e| GenerationError::PromptRender(e.to_string()))?;
        Ok(Self { ai, tera })
    }

    pub fn build_prompt(
        &self,
        user_prompt: &str,
        attachments: &[AiAttachment],
    ) -> Result<String, GenerationError> {
        let mut ctx = Context::new();
        ctx.insert("width_px", &CANVAS_WIDTH);
        ctx.insert("height_px", &CANVAS_HEIGHT);
        ctx.insert("user_prompt", user_prompt);

        // The template iterates `attachments` to render labels + `@path`
        // tokens (the latter activates Gemini CLI's multimodal attach;
        // harmless text for Claude/LM Studio, which receive images through
        // other channels). We project to a plain string view so Tera does not
        // need `serde::Serialize` on `AiAttachment` itself.
        #[derive(serde::Serialize)]
        struct AttachmentView<'a> {
            label: &'a str,
            path: String,
        }
        let views: Vec<AttachmentView<'_>> = attachments
            .iter()
            .map(|a| AttachmentView {
                label: &a.label,
                path: a.path.display().to_string(),
            })
            .collect();
        ctx.insert("attachments", &views);

        self.tera
            .render(TEMPLATE_NAME, &ctx)
            .map_err(|e| GenerationError::PromptRender(e.to_string()))
    }

    pub fn generate(
        &self,
        user_prompt: String,
    ) -> BoxStream<'static, Result<GenerationEvent, GenerationError>> {
        self.generate_with_attachments(user_prompt, Vec::new())
    }

    pub fn generate_with_attachments(
        &self,
        user_prompt: String,
        attachments: Vec<AiAttachment>,
    ) -> BoxStream<'static, Result<GenerationEvent, GenerationError>> {
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
                        yield Ok(GenerationEvent::Delta(t));
                    }
                    Ok(AiChunk::Done { full_text: ai_full }) => {
                        let raw = if ai_full.is_empty() { full_text.clone() } else { ai_full };
                        // Diagnostic: log the raw AI response (first 500
                        // chars) + parsed slide count so we can tell
                        // whether a "only one slide" complaint is the
                        // AI ignoring the deck schema, returning a
                        // single-slide deck, or something else.
                        tracing::info!(
                            raw_len = raw.len(),
                            raw_head = %truncate(&raw, 500),
                            "canvas.generate: AI response received",
                        );
                        match parse_deck(&raw) {
                            Ok((deck, deck_json)) => {
                                tracing::info!(
                                    slide_count = deck.slides.len(),
                                    "canvas.generate: deck parsed",
                                );
                                let html = design_doc_site::deck_to_html(&deck);
                                // Legacy `slide_doc_json` carries the first
                                // slide so v1 consumers (PPTX v1 path, any
                                // leftover single-slide reader) still see
                                // something structured. v2 consumers read
                                // `deck_doc_json` and ignore this.
                                let slide_doc_json = serde_json::to_string(&deck.slides[0])
                                    .unwrap_or_default();
                                yield Ok(GenerationEvent::Complete {
                                    html,
                                    slide_doc_json,
                                    deck_doc_json: deck_json,
                                });
                            }
                            Err(e) => {
                                tracing::warn!(
                                    error = %e,
                                    "canvas.generate: deck parse failed",
                                );
                                yield Err(GenerationError::InvalidSlideDoc(e));
                            }
                        }
                        return;
                    }
                    Err(e) => {
                        yield Err(GenerationError::Ai(e));
                        return;
                    }
                }
            }
        };
        stream.boxed()
    }
}

/// Wrapper that matches the deck-system-prompt's pinned output schema
/// (`{"deck":{"slides":[SlideDoc, ...]}}`). Kept private so callers go
/// through `parse_deck`, which also validates non-empty slides and
/// attaches a server-side deck id.
#[derive(Deserialize)]
struct DeckEnvelope {
    deck: DeckBody,
}

#[derive(Deserialize)]
struct DeckBody {
    slides: Vec<SlideDoc>,
}

/// Parse the raw AI output (optionally markdown-fenced) into a `Deck`
/// plus its serialized JSON. Rejects:
/// - malformed JSON,
/// - missing `deck.slides` wrapper (bare v1 SlideDoc),
/// - empty `slides: []` (degenerate deck — handler should bail rather
///   than persist an unusable version).
fn parse_deck(raw: &str) -> Result<(Deck, String), String> {
    let cleaned = strip_code_fence(raw.trim());
    let envelope: DeckEnvelope = serde_json::from_str(cleaned)
        .map_err(|e| format!("{e} :: {}", truncate(cleaned, 200)))?;
    if envelope.deck.slides.is_empty() {
        return Err(format!(
            "deck.slides[] is empty :: {}",
            truncate(cleaned, 200)
        ));
    }
    let deck = Deck {
        // Server-generated id so the AI can't collide deck ids across
        // versions; SlideDoc ids remain as authored.
        id: format!("deck-{}", uuid::Uuid::new_v4()),
        slides: envelope.deck.slides,
    };
    let json = serde_json::to_string(&deck).map_err(|e| e.to_string())?;
    Ok((deck, json))
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
    use design_llm::FakeAiClient;

    fn slide_doc_json() -> String {
        r##"{
            "id":"s1","width_px":1280,"height_px":720,
            "background":{"kind":"color","color":"#ffffff"},
            "elements":[
                {"kind":"text","id":"title","frame":{"x":80,"y":300,"w":1120,"h":120},
                 "content":"안녕, Canvas",
                 "style":{"font_family":"Pretendard","font_size_px":72,"font_weight":700,
                          "color":"#111111","line_height":1.2,"letter_spacing_px":-1.0,"align":"center"}}
            ]
        }"##
        .to_string()
    }

    #[tokio::test]
    async fn build_prompt_includes_canvas_size_and_user_text() {
        let ai = Arc::new(FakeAiClient::new(Vec::<String>::new()));
        let cgen = CanvasGenerator::new(ai).unwrap();
        let prompt = cgen.build_prompt("AI 안전성 표지", &[]).unwrap();
        assert!(prompt.contains("1280"));
        assert!(prompt.contains("720"));
        assert!(prompt.contains("AI 안전성 표지"));
        assert!(prompt.contains("Pretendard"));
        // No attachments → no references block.
        assert!(!prompt.contains("[참고 자료]"));
    }

    #[tokio::test]
    async fn build_prompt_inserts_reference_block_before_user_request() {
        let ai = Arc::new(FakeAiClient::new(Vec::<String>::new()));
        let cgen = CanvasGenerator::new(ai).unwrap();
        let atts = vec![
            AiAttachment::image(
                "/tmp/deck-page-001.png",
                "image/png",
                "deck.pdf (1/2 페이지)",
            ),
            AiAttachment::image(
                "/tmp/deck-page-002.png",
                "image/png",
                "deck.pdf (2/2 페이지)",
            ),
        ];
        let prompt = cgen.build_prompt("표지", &atts).unwrap();

        // Block must appear BEFORE the user request so the model actually
        // reads references before deciding what to output.
        let refs_idx = prompt
            .find("[참고 자료]")
            .expect("references block present");
        let user_idx = prompt
            .find("사용자 요청:")
            .expect("user request marker present");
        assert!(
            refs_idx < user_idx,
            "references block should come before user request"
        );

        // Labels and @path tokens are both rendered so Gemini CLI attaches
        // images and the model has source context for each page.
        assert!(prompt.contains("deck.pdf (1/2 페이지)"));
        assert!(prompt.contains("deck.pdf (2/2 페이지)"));
        assert!(prompt.contains("@/tmp/deck-page-001.png"));
        assert!(prompt.contains("@/tmp/deck-page-002.png"));
    }

    /// Wrap a bare SlideDoc JSON string in the deck envelope the generator
    /// now requires (`{"deck":{"slides":[SlideDoc]}}`). Keeps old tests
    /// that were written pre-v2 meaningful without duplicating the
    /// fixture.
    fn wrap_in_deck(slide_json: &str) -> String {
        format!(r##"{{"deck":{{"slides":[{slide_json}]}}}}"##)
    }

    #[tokio::test]
    async fn generate_streams_deltas_then_completes_with_html() {
        let json = wrap_in_deck(&slide_doc_json());
        let ai = Arc::new(FakeAiClient::new(vec![json]));
        let cgen = CanvasGenerator::new(ai.clone()).unwrap();

        let events: Vec<_> = cgen
            .generate("표지".into())
            .collect::<Vec<_>>()
            .await;
        let unwrapped: Vec<GenerationEvent> =
            events.into_iter().map(|r| r.expect("ok")).collect();

        assert!(unwrapped.iter().any(|e| matches!(e, GenerationEvent::Delta(_))));
        let last = unwrapped.last().expect("at least one");
        match last {
            GenerationEvent::Complete { html, slide_doc_json, deck_doc_json } => {
                assert!(html.contains("data-edit-zone=\"title\""));
                assert!(html.contains("안녕, Canvas"));
                assert!(slide_doc_json.contains("\"id\":\"s1\""));
                assert!(deck_doc_json.contains("\"slides\""));
            }
            _ => panic!("expected Complete last, got {last:?}"),
        }

        let recorded = ai.prompts();
        assert_eq!(recorded.len(), 1);
        assert!(recorded[0].contains("표지"));
    }

    #[tokio::test]
    async fn generate_strips_markdown_code_fences() {
        let wrapped = format!("```json\n{}\n```", wrap_in_deck(&slide_doc_json()));
        let ai = Arc::new(FakeAiClient::new(vec![wrapped]));
        let cgen = CanvasGenerator::new(ai).unwrap();

        let events: Vec<_> = cgen.generate("p".into()).collect::<Vec<_>>().await;
        let last = events.last().unwrap().as_ref().expect("ok");
        assert!(matches!(last, GenerationEvent::Complete { .. }));
    }

    #[tokio::test]
    async fn generate_yields_invalid_slide_doc_on_garbage() {
        let ai = Arc::new(FakeAiClient::new(vec!["nope, just text"]));
        let cgen = CanvasGenerator::new(ai).unwrap();
        let events: Vec<_> = cgen.generate("p".into()).collect::<Vec<_>>().await;
        let last = events.last().unwrap();
        assert!(matches!(last, Err(GenerationError::InvalidSlideDoc(_))));
    }

    #[tokio::test]
    async fn generate_propagates_ai_error() {
        let ai = Arc::new(FakeAiClient::failing("upstream gone"));
        let cgen = CanvasGenerator::new(ai).unwrap();
        let events: Vec<_> = cgen.generate("p".into()).collect::<Vec<_>>().await;
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], Err(GenerationError::Ai(_))));
    }

    // ----- P4: deck-wrapped generation -----

    /// Build a `{"deck":{"slides":[{...},{...},{...}]}}` fixture with N
    /// distinct slides (ids slide-1..slide-N). The template's multi-slide
    /// contract.
    fn deck_json_with_slides(n: usize) -> String {
        let slides: Vec<String> = (1..=n)
            .map(|i| {
                format!(
                    r##"{{"id":"slide-{i}","width_px":1280,"height_px":720,"background":{{"kind":"color","color":"#ffffff"}},"elements":[{{"kind":"text","id":"title","frame":{{"x":80,"y":300,"w":1120,"h":120}},"content":"장 {i}","style":{{"font_family":"Pretendard","font_size_px":72,"font_weight":700,"color":"#111111","line_height":1.2,"letter_spacing_px":-1.0,"align":"center"}}}}]}}"##
                )
            })
            .collect();
        format!(r##"{{"deck":{{"slides":[{}]}}}}"##, slides.join(","))
    }

    #[tokio::test]
    async fn generate_parses_deck_wrapped_single_slide() {
        // v2 contract: AI returns `{"deck":{"slides":[...]}}` even for a
        // single-slide request. The Complete event must carry the full
        // deck JSON plus (for legacy compat) the first slide JSON.
        let deck_raw = deck_json_with_slides(1);
        let ai = Arc::new(FakeAiClient::new(vec![deck_raw]));
        let cgen = CanvasGenerator::new(ai).unwrap();
        let events: Vec<_> = cgen.generate("표지".into()).collect::<Vec<_>>().await;
        let last = events.last().unwrap().as_ref().expect("ok");
        match last {
            GenerationEvent::Complete { html, slide_doc_json, deck_doc_json } => {
                let deck: design_doc_slide::Deck =
                    serde_json::from_str(deck_doc_json).expect("valid deck json");
                assert_eq!(deck.slides.len(), 1);
                assert_eq!(deck.slides[0].id, "slide-1");
                assert!(
                    slide_doc_json.contains("\"id\":\"slide-1\""),
                    "legacy field carries first slide: {slide_doc_json}"
                );
                // HTML is the deck-level render (section per slide).
                assert_eq!(html.matches("<section").count(), 1, "one <section> for one slide");
                assert!(html.contains("data-slide-id=\"slide-1\""));
            }
            other => panic!("expected Complete, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn generate_parses_deck_with_three_slides() {
        // Portfolio-style request → multi-slide deck. This is the whole
        // point of v2 — v1 could only ever emit one section, so a 3-slide
        // deck proves we've broken the 1-slide cap.
        let deck_raw = deck_json_with_slides(3);
        let ai = Arc::new(FakeAiClient::new(vec![deck_raw]));
        let cgen = CanvasGenerator::new(ai).unwrap();
        let events: Vec<_> = cgen
            .generate("포트폴리오 3장".into())
            .collect::<Vec<_>>()
            .await;
        let last = events.last().unwrap().as_ref().expect("ok");
        match last {
            GenerationEvent::Complete { html, deck_doc_json, .. } => {
                let deck: design_doc_slide::Deck =
                    serde_json::from_str(deck_doc_json).expect("valid deck json");
                assert_eq!(deck.slides.len(), 3);
                assert_eq!(deck.slides[0].id, "slide-1");
                assert_eq!(deck.slides[2].id, "slide-3");
                assert_eq!(
                    html.matches("<section").count(),
                    3,
                    "three <section> elements for three slides"
                );
            }
            other => panic!("expected Complete, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn generate_rejects_empty_slides_array() {
        // An empty deck is a degenerate AI output — surface it as
        // InvalidSlideDoc so the handler can bail instead of persisting
        // an unusable version.
        let empty = r##"{"deck":{"slides":[]}}"##;
        let ai = Arc::new(FakeAiClient::new(vec![empty]));
        let cgen = CanvasGenerator::new(ai).unwrap();
        let events: Vec<_> = cgen.generate("p".into()).collect::<Vec<_>>().await;
        let last = events.last().unwrap();
        assert!(
            matches!(last, Err(GenerationError::InvalidSlideDoc(_))),
            "expected InvalidSlideDoc on empty slides, got {last:?}"
        );
    }

    #[tokio::test]
    async fn generate_rejects_missing_deck_wrapper() {
        // Bare SlideDoc (the v1 format) is no longer valid output — the
        // v2 template explicitly frames output as `{"deck":{...}}`. If
        // the AI regresses to bare SlideDoc, we want a clear error
        // instead of silently accepting it (which would mean the deck
        // contract is gone).
        let bare_slide = r##"{"id":"s","width_px":1280,"height_px":720,"background":{"kind":"color","color":"#fff"},"elements":[]}"##;
        let ai = Arc::new(FakeAiClient::new(vec![bare_slide]));
        let cgen = CanvasGenerator::new(ai).unwrap();
        let events: Vec<_> = cgen.generate("p".into()).collect::<Vec<_>>().await;
        let last = events.last().unwrap();
        assert!(
            matches!(last, Err(GenerationError::InvalidSlideDoc(_))),
            "expected InvalidSlideDoc on bare SlideDoc, got {last:?}"
        );
    }
}
