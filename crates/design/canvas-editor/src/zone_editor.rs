use std::collections::BTreeSet;
use std::sync::Arc;

use futures::stream::{BoxStream, StreamExt};
use serde::{Deserialize, Serialize};
use tera::{Context, Tera};
use thiserror::Error;

use canvas_llm::{AiChunk, AiClient, AiError};

const SYSTEM_PROMPT_TEMPLATE: &str = include_str!("../templates/zone-edit-prompt.tera");
const TEMPLATE_NAME: &str = "zone-edit-prompt";
const BATCH_PROMPT_TEMPLATE: &str = include_str!("../templates/zone-edit-batch-prompt.tera");
const BATCH_TEMPLATE_NAME: &str = "zone-edit-batch-prompt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZoneEditEvent {
    Delta(String),
    Complete { html: String },
}

/// Batch-edit streaming events. `Delta` carries raw AI text (JSON fragments —
/// not individually useful to a human UI but kept so the chat can show a
/// generic "생성 중…(Nxxx자)" indicator). `Complete` fires once, after the full
/// response has been parsed and validated against the request's zone_id set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchEditEvent {
    Delta(String),
    /// Parsed `(zone_id, new_html)` pairs in the same order as the request.
    Complete { zones: Vec<(String, String)> },
}

#[derive(Debug, Error)]
pub enum ZoneEditError {
    #[error(transparent)]
    Ai(#[from] AiError),
    #[error("could not render zone-edit prompt: {0}")]
    PromptRender(String),
    #[error("AI returned no zone HTML")]
    EmptyOutput,
    /// Batch response JSON was malformed or did not cover exactly the
    /// requested zone_id set (missing / duplicate / extra ids).
    #[error("invalid batch response: {0}")]
    InvalidBatchResponse(String),
}

pub struct ZoneEditor {
    ai: Arc<dyn AiClient>,
    tera: Tera,
}

pub struct EditPromptInput<'a> {
    pub zone_id: &'a str,
    pub zone_label: &'a str,
    pub current_html: &'a str,
    pub instruction: &'a str,
    pub width_px: u32,
    pub height_px: u32,
    /// Free-form sentence describing the canvas the zone lives inside
    /// (e.g. "이 박스는 1280 × 720 px 프레젠테이션 슬라이드 안에 있다." for
    /// slides vs. "이 박스는 Tailwind 기반 반응형 랜딩페이지 섹션 안에
    /// 있다. `md:` prefix 유지." for canvas-web). Caller — the RPC
    /// handler — branches on `project.kind`; the editor just inserts
    /// this verbatim into the prompt so the model can orient itself
    /// without us maintaining two near-identical templates.
    pub context_hint: &'a str,
}

/// Input for multi-zone batch editing. The RPC layer is responsible for
/// enforcing the 2..=6 zone count constraint from the design doc; this
/// struct is permissive so it can also be unit-tested at the boundaries.
pub struct BatchEditInput<'a> {
    pub instruction: &'a str,
    pub zones: Vec<BatchZone<'a>>,
    pub width_px: u32,
    pub height_px: u32,
    /// Same canvas-orientation sentence as `EditPromptInput.context_hint`.
    /// Appears once, above the per-zone list, so the model applies a
    /// single coherent frame to every zone in the batch.
    pub context_hint: &'a str,
}

pub struct BatchZone<'a> {
    pub zone_id: &'a str,
    pub zone_label: &'a str,
    pub current_html: &'a str,
}

impl ZoneEditor {
    pub fn new(ai: Arc<dyn AiClient>) -> Result<Self, ZoneEditError> {
        let mut tera = Tera::default();
        tera.add_raw_template(TEMPLATE_NAME, SYSTEM_PROMPT_TEMPLATE)
            .map_err(|e| ZoneEditError::PromptRender(e.to_string()))?;
        tera.add_raw_template(BATCH_TEMPLATE_NAME, BATCH_PROMPT_TEMPLATE)
            .map_err(|e| ZoneEditError::PromptRender(e.to_string()))?;
        Ok(Self { ai, tera })
    }

    pub fn build_prompt(&self, input: EditPromptInput<'_>) -> Result<String, ZoneEditError> {
        let mut ctx = Context::new();
        ctx.insert("zone_id", input.zone_id);
        ctx.insert("zone_label", input.zone_label);
        ctx.insert("current_html", input.current_html);
        ctx.insert("instruction", input.instruction);
        ctx.insert("width_px", &input.width_px);
        ctx.insert("height_px", &input.height_px);
        ctx.insert("context_hint", input.context_hint);
        self.tera
            .render(TEMPLATE_NAME, &ctx)
            .map_err(|e| ZoneEditError::PromptRender(e.to_string()))
    }

    pub fn build_batch_prompt(&self, input: &BatchEditInput<'_>) -> Result<String, ZoneEditError> {
        // Tera context wants Serialize-able values, but we hold `&str`s on
        // BatchZone to avoid allocations at the caller. Project to a tiny
        // Serialize view here (same trick canvas_generator uses for
        // attachments).
        #[derive(Serialize)]
        struct ZoneView<'a> {
            zone_id: &'a str,
            zone_label: &'a str,
            current_html: &'a str,
        }
        let views: Vec<ZoneView<'_>> = input
            .zones
            .iter()
            .map(|z| ZoneView {
                zone_id: z.zone_id,
                zone_label: z.zone_label,
                current_html: z.current_html,
            })
            .collect();
        let mut ctx = Context::new();
        ctx.insert("instruction", input.instruction);
        ctx.insert("zones", &views);
        ctx.insert("width_px", &input.width_px);
        ctx.insert("height_px", &input.height_px);
        ctx.insert("context_hint", input.context_hint);
        self.tera
            .render(BATCH_TEMPLATE_NAME, &ctx)
            .map_err(|e| ZoneEditError::PromptRender(e.to_string()))
    }

    pub fn edit(
        &self,
        input: EditPromptInput<'_>,
    ) -> BoxStream<'static, Result<ZoneEditEvent, ZoneEditError>> {
        let prompt = match self.build_prompt(input) {
            Ok(p) => p,
            Err(e) => return futures::stream::iter(vec![Err(e)]).boxed(),
        };
        let upstream = self.ai.complete(prompt, Vec::new());

        let stream = async_stream::stream! {
            let mut upstream = upstream;
            let mut full_text = String::new();
            while let Some(item) = upstream.next().await {
                match item {
                    Ok(AiChunk::Delta(t)) => {
                        full_text.push_str(&t);
                        yield Ok(ZoneEditEvent::Delta(t));
                    }
                    Ok(AiChunk::Done { full_text: ai_full }) => {
                        let raw = if ai_full.is_empty() { full_text.clone() } else { ai_full };
                        let cleaned = strip_code_fence(raw.trim()).trim();
                        if cleaned.is_empty() {
                            yield Err(ZoneEditError::EmptyOutput);
                            return;
                        }
                        yield Ok(ZoneEditEvent::Complete { html: cleaned.to_string() });
                        return;
                    }
                    Err(e) => {
                        yield Err(ZoneEditError::Ai(e));
                        return;
                    }
                }
            }
        };
        stream.boxed()
    }

    /// Batch-edit multiple zones in a single AI call. Deltas stream through
    /// unchanged; once the upstream signals Done, the accumulated text is
    /// parsed as `{"zones":[{"id","html"},...]}` and validated to cover
    /// exactly the requested zone_id set. On any structural issue we emit
    /// `InvalidBatchResponse` — the caller (RPC handler) is expected to
    /// surface this as a user-visible error and commit no overrides
    /// (all-or-nothing, per design doc §DB).
    pub fn edit_batch(
        &self,
        input: BatchEditInput<'_>,
    ) -> BoxStream<'static, Result<BatchEditEvent, ZoneEditError>> {
        let prompt = match self.build_batch_prompt(&input) {
            Ok(p) => p,
            Err(e) => return futures::stream::iter(vec![Err(e)]).boxed(),
        };
        // Snapshot the request id set (owned) so the closure can outlive
        // the caller's borrow of BatchEditInput.
        let requested_ids: Vec<String> =
            input.zones.iter().map(|z| z.zone_id.to_string()).collect();
        let upstream = self.ai.complete(prompt, Vec::new());

        let stream = async_stream::stream! {
            let mut upstream = upstream;
            let mut full_text = String::new();
            while let Some(item) = upstream.next().await {
                match item {
                    Ok(AiChunk::Delta(t)) => {
                        full_text.push_str(&t);
                        yield Ok(BatchEditEvent::Delta(t));
                    }
                    Ok(AiChunk::Done { full_text: ai_full }) => {
                        let raw = if ai_full.is_empty() { full_text.clone() } else { ai_full };
                        match parse_batch_response(&raw, &requested_ids) {
                            Ok(zones) => {
                                yield Ok(BatchEditEvent::Complete { zones });
                            }
                            Err(msg) => {
                                yield Err(ZoneEditError::InvalidBatchResponse(msg));
                            }
                        }
                        return;
                    }
                    Err(e) => {
                        yield Err(ZoneEditError::Ai(e));
                        return;
                    }
                }
            }
        };
        stream.boxed()
    }
}

/// Parse `{"zones":[{"id","html"},...]}` from the AI's raw output. Rules:
/// - Strip markdown code fences (models sometimes wrap JSON despite the
///   prompt saying not to).
/// - Require valid JSON with a `zones` array of objects each carrying
///   non-empty `id` and `html` strings.
/// - The set of returned ids must equal `requested` exactly — no missing,
///   no duplicates, no extras. This is what enforces all-or-nothing.
/// - Returned order preserves the request order so UIs can render progress
///   deterministically.
fn parse_batch_response(raw: &str, requested: &[String]) -> Result<Vec<(String, String)>, String> {
    #[derive(Deserialize)]
    struct Outer {
        zones: Vec<Inner>,
    }
    #[derive(Deserialize)]
    struct Inner {
        id: String,
        html: String,
    }

    let cleaned = strip_code_fence(raw.trim()).trim();
    if cleaned.is_empty() {
        return Err("empty response".to_string());
    }
    let parsed: Outer = serde_json::from_str(cleaned)
        .map_err(|e| format!("json parse: {e} :: {}", truncate_for_msg(cleaned)))?;

    let requested_set: BTreeSet<&str> = requested.iter().map(|s| s.as_str()).collect();
    let response_set: BTreeSet<&str> = parsed.zones.iter().map(|z| z.id.as_str()).collect();

    if response_set.len() != parsed.zones.len() {
        return Err(format!(
            "duplicate zone id in response (got {} entries, {} unique)",
            parsed.zones.len(),
            response_set.len()
        ));
    }
    if response_set != requested_set {
        let missing: Vec<&&str> = requested_set.difference(&response_set).collect();
        let extra: Vec<&&str> = response_set.difference(&requested_set).collect();
        return Err(format!(
            "zone id set mismatch: missing={missing:?} extra={extra:?}"
        ));
    }

    // Preserve request order in the returned vector so downstream rendering
    // (status list, override insertion) is deterministic regardless of the
    // order the model chose in its JSON.
    let by_id: std::collections::HashMap<String, String> = parsed
        .zones
        .into_iter()
        .map(|z| (z.id, z.html))
        .collect();
    let mut out = Vec::with_capacity(requested.len());
    for id in requested {
        let html = by_id
            .get(id)
            .cloned()
            .expect("set equality guarantees presence");
        if html.trim().is_empty() {
            return Err(format!("empty html for zone '{id}'"));
        }
        out.push((id.clone(), html));
    }
    Ok(out)
}

fn truncate_for_msg(s: &str) -> String {
    const MAX: usize = 200;
    if s.len() <= MAX {
        s.to_string()
    } else {
        format!("{}… (+{} more bytes)", &s[..MAX], s.len() - MAX)
    }
}

fn strip_code_fence(s: &str) -> &str {
    // Models occasionally wrap their output in ```html, ```json, or a bare
    // ``` fence despite prompt-level instructions not to. We strip the most
    // common language tags up front so downstream parsers see raw content.
    let s = s.trim();
    let stripped = s
        .strip_prefix("```html")
        .or_else(|| s.strip_prefix("```json"))
        .or_else(|| s.strip_prefix("```"));
    let s = stripped.unwrap_or(s);
    let s = s.trim_start_matches('\n').trim();
    s.strip_suffix("```").map(str::trim).unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use canvas_llm::FakeAiClient;
    use futures::StreamExt;

    fn input<'a>(instr: &'a str, current: &'a str) -> EditPromptInput<'a> {
        EditPromptInput {
            zone_id: "title",
            zone_label: "표지 제목",
            current_html: current,
            instruction: instr,
            width_px: 1280,
            height_px: 720,
            context_hint: "이 박스는 1280 × 720 px 프레젠테이션 슬라이드 안에 있다.",
        }
    }

    #[tokio::test]
    async fn build_prompt_renders_web_context_hint_when_given() {
        // Web-mode zones live in a Tailwind landing-page section, not a
        // 1280×720 slide. The caller is responsible for passing the right
        // hint (handler branches on project.kind); the editor just has to
        // flow it through unchanged.
        let ai = Arc::new(FakeAiClient::new(Vec::<String>::new()));
        let editor = ZoneEditor::new(ai).unwrap();
        let prompt = editor
            .build_prompt(EditPromptInput {
                zone_id: "headline",
                zone_label: "Hero 제목",
                current_html: r#"<h1 class="text-6xl md:text-8xl">Hi</h1>"#,
                instruction: "더 진중하게",
                width_px: 1280,
                height_px: 600,
                context_hint:
                    "이 박스는 Tailwind 기반 반응형 랜딩페이지 섹션 안에 있다. `md:` prefix 유지.",
            })
            .unwrap();
        assert!(prompt.contains("랜딩페이지"), "prompt: {prompt}");
        assert!(prompt.contains("`md:`"), "prompt: {prompt}");
    }

    #[tokio::test]
    async fn build_prompt_renders_slides_context_hint_when_given() {
        let ai = Arc::new(FakeAiClient::new(Vec::<String>::new()));
        let editor = ZoneEditor::new(ai).unwrap();
        let prompt = editor.build_prompt(input("x", "<div>old</div>")).unwrap();
        assert!(
            prompt.contains("프레젠테이션 슬라이드"),
            "slides hint should flow through: {prompt}"
        );
    }

    #[tokio::test]
    async fn build_prompt_includes_zone_id_label_instruction_and_html() {
        let ai = Arc::new(FakeAiClient::new(Vec::<String>::new()));
        let editor = ZoneEditor::new(ai).unwrap();
        let prompt = editor
            .build_prompt(input(
                "더 간결하게",
                r#"<div data-edit-zone="title">긴 제목</div>"#,
            ))
            .unwrap();
        assert!(prompt.contains("title"));
        assert!(prompt.contains("표지 제목"));
        assert!(prompt.contains("더 간결하게"));
        assert!(prompt.contains("긴 제목"));
        assert!(prompt.contains("1280"));
    }

    #[tokio::test]
    async fn edit_streams_deltas_then_completes_with_html() {
        let new_html = r#"<div data-edit-zone="title" style="left:80px;top:300px">새 제목</div>"#;
        let ai = Arc::new(FakeAiClient::new(vec![new_html]));
        let editor = ZoneEditor::new(ai.clone()).unwrap();

        let events: Vec<_> = editor
            .edit(input("짧게", r#"<div data-edit-zone="title">긴 제목</div>"#))
            .collect::<Vec<_>>()
            .await;
        let unwrapped: Vec<ZoneEditEvent> =
            events.into_iter().map(|r| r.expect("ok")).collect();

        assert!(unwrapped.iter().any(|e| matches!(e, ZoneEditEvent::Delta(_))));
        let last = unwrapped.last().expect("at least one");
        match last {
            ZoneEditEvent::Complete { html } => {
                assert!(html.contains("새 제목"), "got {html}");
                assert!(html.contains(r#"data-edit-zone="title""#));
            }
            _ => panic!("expected Complete last"),
        }

        let recorded = ai.prompts();
        assert_eq!(recorded.len(), 1);
        assert!(recorded[0].contains("짧게"));
    }

    #[tokio::test]
    async fn edit_strips_markdown_code_fences() {
        let wrapped = format!(
            "```html\n{}\n```",
            r#"<div data-edit-zone="title">새</div>"#
        );
        let ai = Arc::new(FakeAiClient::new(vec![wrapped]));
        let editor = ZoneEditor::new(ai).unwrap();
        let events: Vec<_> = editor
            .edit(input("x", r#"<div data-edit-zone="title">old</div>"#))
            .collect::<Vec<_>>()
            .await;
        let last = events.last().unwrap().as_ref().expect("ok");
        match last {
            ZoneEditEvent::Complete { html } => {
                assert!(!html.contains("```"), "fences must be stripped: {html}");
                assert!(html.contains("새"));
            }
            _ => panic!("expected Complete"),
        }
    }

    #[tokio::test]
    async fn edit_yields_empty_output_on_blank_response() {
        let ai = Arc::new(FakeAiClient::new(vec!["   "]));
        let editor = ZoneEditor::new(ai).unwrap();
        let events: Vec<_> = editor
            .edit(input("x", r#"<div data-edit-zone="t">old</div>"#))
            .collect::<Vec<_>>()
            .await;
        let last = events.last().unwrap();
        assert!(matches!(last, Err(ZoneEditError::EmptyOutput)), "got {last:?}");
    }

    #[tokio::test]
    async fn edit_propagates_ai_error() {
        let ai = Arc::new(FakeAiClient::failing("upstream gone"));
        let editor = ZoneEditor::new(ai).unwrap();
        let events: Vec<_> = editor
            .edit(input("x", r#"<div data-edit-zone="t">x</div>"#))
            .collect::<Vec<_>>()
            .await;
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], Err(ZoneEditError::Ai(_))));
    }

    // -- Batch edit --
    //
    // These exercise the multi-zone path introduced for canvas.edit_zones.
    // The editor itself is permissive about zone count (2+); the 2..=6 cap
    // lives in the RPC handler where it belongs. All tests here use a
    // FakeAiClient scripted with a fixed final response so we can assert
    // parsing + validation independent of any real model behaviour.

    fn batch<'a>(
        instr: &'a str,
        zones: Vec<(&'a str, &'a str, &'a str)>,
    ) -> BatchEditInput<'a> {
        BatchEditInput {
            instruction: instr,
            zones: zones
                .into_iter()
                .map(|(id, label, html)| BatchZone {
                    zone_id: id,
                    zone_label: label,
                    current_html: html,
                })
                .collect(),
            width_px: 1280,
            height_px: 720,
            context_hint: "이 박스는 1280 × 720 px 프레젠테이션 슬라이드 안에 있다.",
        }
    }

    #[tokio::test]
    async fn build_batch_prompt_renders_web_context_hint_when_given() {
        let ai = Arc::new(FakeAiClient::new(Vec::<String>::new()));
        let editor = ZoneEditor::new(ai).unwrap();
        let mut b = batch(
            "톤 맞춰서",
            vec![
                ("a", "A", r#"<p data-edit-zone="a">A</p>"#),
                ("b", "B", r#"<p data-edit-zone="b">B</p>"#),
            ],
        );
        b.context_hint =
            "이 박스는 Tailwind 기반 반응형 랜딩페이지 섹션 안에 있다. `md:` prefix 유지.";
        let prompt = editor.build_batch_prompt(&b).unwrap();
        assert!(prompt.contains("랜딩페이지"), "prompt: {prompt}");
        assert!(prompt.contains("`md:`"), "prompt: {prompt}");
    }

    #[tokio::test]
    async fn build_batch_prompt_lists_every_zone_and_the_shared_instruction() {
        let ai = Arc::new(FakeAiClient::new(Vec::<String>::new()));
        let editor = ZoneEditor::new(ai).unwrap();
        let prompt = editor
            .build_batch_prompt(&batch(
                "톤 맞춰서 더 진중하게",
                vec![
                    ("title", "표지 제목", r#"<div data-edit-zone="title">원본</div>"#),
                    ("sub", "부제목", r#"<div data-edit-zone="sub">서브</div>"#),
                ],
            ))
            .unwrap();
        // Shared instruction rendered once.
        assert_eq!(prompt.matches("톤 맞춰서 더 진중하게").count(), 1);
        // Each zone_id, label, and html present.
        assert!(prompt.contains("title"));
        assert!(prompt.contains("sub"));
        assert!(prompt.contains("표지 제목"));
        assert!(prompt.contains("부제목"));
        assert!(prompt.contains("원본"));
        assert!(prompt.contains("서브"));
        // Schema directive present so the model produces parseable JSON.
        assert!(prompt.contains(r#"{"zones""#));
        // Canvas size still flows through.
        assert!(prompt.contains("1280"));
    }

    #[tokio::test]
    async fn edit_batch_parses_json_response_and_preserves_request_order() {
        let response = r#"{"zones":[{"id":"sub","html":"<div data-edit-zone=\"sub\">새 서브</div>"},{"id":"title","html":"<div data-edit-zone=\"title\">새 제목</div>"}]}"#;
        let ai = Arc::new(FakeAiClient::new(vec![response]));
        let editor = ZoneEditor::new(ai).unwrap();

        let events: Vec<_> = editor
            .edit_batch(batch(
                "진중하게",
                vec![
                    ("title", "표지 제목", r#"<div data-edit-zone="title">A</div>"#),
                    ("sub", "부제", r#"<div data-edit-zone="sub">B</div>"#),
                ],
            ))
            .collect::<Vec<_>>()
            .await;
        let last = events.last().expect("some").as_ref().expect("ok");
        match last {
            BatchEditEvent::Complete { zones } => {
                // Request order (title, sub) must survive even though the
                // response listed sub first — deterministic for UI.
                assert_eq!(zones.len(), 2);
                assert_eq!(zones[0].0, "title");
                assert_eq!(zones[1].0, "sub");
                assert!(zones[0].1.contains("새 제목"));
                assert!(zones[1].1.contains("새 서브"));
            }
            _ => panic!("expected Complete"),
        }
    }

    #[tokio::test]
    async fn edit_batch_strips_markdown_fences_around_json() {
        let response = "```json\n{\"zones\":[{\"id\":\"t\",\"html\":\"<p data-edit-zone=\\\"t\\\">X</p>\"}]}\n```";
        let ai = Arc::new(FakeAiClient::new(vec![response]));
        let editor = ZoneEditor::new(ai).unwrap();

        let events: Vec<_> = editor
            .edit_batch(batch("x", vec![("t", "T", r#"<p data-edit-zone="t">old</p>"#)]))
            .collect::<Vec<_>>()
            .await;
        let last = events.last().unwrap().as_ref().expect("ok");
        match last {
            BatchEditEvent::Complete { zones } => {
                assert_eq!(zones.len(), 1);
                assert!(zones[0].1.contains("X"));
            }
            _ => panic!("expected Complete"),
        }
    }

    #[tokio::test]
    async fn edit_batch_rejects_missing_zone_id() {
        // Requested two zones, AI returned only one → should error, no
        // partial commit. This is the main defence for all-or-nothing.
        let response = r#"{"zones":[{"id":"title","html":"<div data-edit-zone=\"title\">OK</div>"}]}"#;
        let ai = Arc::new(FakeAiClient::new(vec![response]));
        let editor = ZoneEditor::new(ai).unwrap();

        let events: Vec<_> = editor
            .edit_batch(batch(
                "x",
                vec![
                    ("title", "T", r#"<div data-edit-zone="title">A</div>"#),
                    ("sub", "S", r#"<div data-edit-zone="sub">B</div>"#),
                ],
            ))
            .collect::<Vec<_>>()
            .await;
        let last = events.last().unwrap();
        match last {
            Err(ZoneEditError::InvalidBatchResponse(msg)) => {
                assert!(msg.contains("sub"), "msg should mention missing id: {msg}");
            }
            other => panic!("expected InvalidBatchResponse, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn edit_batch_rejects_extra_zone_id() {
        // Model hallucinated an additional zone the user didn't select.
        let response = r#"{"zones":[
            {"id":"title","html":"<div data-edit-zone=\"title\">OK</div>"},
            {"id":"ghost","html":"<div data-edit-zone=\"ghost\">?</div>"}
        ]}"#;
        let ai = Arc::new(FakeAiClient::new(vec![response]));
        let editor = ZoneEditor::new(ai).unwrap();

        let events: Vec<_> = editor
            .edit_batch(batch("x", vec![("title", "T", r#"<div data-edit-zone="title">A</div>"#)]))
            .collect::<Vec<_>>()
            .await;
        let last = events.last().unwrap();
        assert!(matches!(last, Err(ZoneEditError::InvalidBatchResponse(_))));
    }

    #[tokio::test]
    async fn edit_batch_rejects_duplicate_zone_id() {
        let response = r#"{"zones":[
            {"id":"title","html":"<div data-edit-zone=\"title\">A</div>"},
            {"id":"title","html":"<div data-edit-zone=\"title\">B</div>"}
        ]}"#;
        let ai = Arc::new(FakeAiClient::new(vec![response]));
        let editor = ZoneEditor::new(ai).unwrap();

        let events: Vec<_> = editor
            .edit_batch(batch("x", vec![("title", "T", r#"<div data-edit-zone="title">old</div>"#)]))
            .collect::<Vec<_>>()
            .await;
        let last = events.last().unwrap();
        match last {
            Err(ZoneEditError::InvalidBatchResponse(msg)) => {
                assert!(msg.contains("duplicate"), "msg: {msg}");
            }
            other => panic!("expected InvalidBatchResponse, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn edit_batch_rejects_invalid_json() {
        let ai = Arc::new(FakeAiClient::new(vec!["not json at all"]));
        let editor = ZoneEditor::new(ai).unwrap();

        let events: Vec<_> = editor
            .edit_batch(batch("x", vec![("t", "T", r#"<p data-edit-zone="t">x</p>"#)]))
            .collect::<Vec<_>>()
            .await;
        let last = events.last().unwrap();
        assert!(matches!(last, Err(ZoneEditError::InvalidBatchResponse(_))));
    }

    #[tokio::test]
    async fn edit_batch_rejects_empty_html_for_a_zone() {
        let response = r#"{"zones":[
            {"id":"title","html":"<div data-edit-zone=\"title\">A</div>"},
            {"id":"sub","html":"   "}
        ]}"#;
        let ai = Arc::new(FakeAiClient::new(vec![response]));
        let editor = ZoneEditor::new(ai).unwrap();

        let events: Vec<_> = editor
            .edit_batch(batch(
                "x",
                vec![
                    ("title", "T", r#"<div data-edit-zone="title">A</div>"#),
                    ("sub", "S", r#"<div data-edit-zone="sub">B</div>"#),
                ],
            ))
            .collect::<Vec<_>>()
            .await;
        let last = events.last().unwrap();
        match last {
            Err(ZoneEditError::InvalidBatchResponse(msg)) => {
                assert!(msg.contains("empty html"), "msg: {msg}");
            }
            other => panic!("expected InvalidBatchResponse, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn edit_batch_streams_deltas_before_complete() {
        // Even with JSON responses we still surface deltas so the UI can
        // render a generic "생성 중…" spinner. Contents of the deltas are
        // implementation-dependent (raw JSON fragments from the AI).
        let response = r#"{"zones":[{"id":"t","html":"<p data-edit-zone=\"t\">X</p>"}]}"#;
        let ai = Arc::new(FakeAiClient::new(vec![response]));
        let editor = ZoneEditor::new(ai).unwrap();

        let events: Vec<_> = editor
            .edit_batch(batch("x", vec![("t", "T", r#"<p data-edit-zone="t">old</p>"#)]))
            .collect::<Vec<_>>()
            .await;
        let oks: Vec<&BatchEditEvent> = events
            .iter()
            .filter_map(|r| r.as_ref().ok())
            .collect();
        assert!(oks.iter().any(|e| matches!(e, BatchEditEvent::Delta(_))));
        assert!(matches!(oks.last().unwrap(), BatchEditEvent::Complete { .. }));
    }

    #[tokio::test]
    async fn edit_batch_propagates_ai_error() {
        let ai = Arc::new(FakeAiClient::failing("upstream gone"));
        let editor = ZoneEditor::new(ai).unwrap();
        let events: Vec<_> = editor
            .edit_batch(batch("x", vec![("t", "T", r#"<p data-edit-zone="t">x</p>"#)]))
            .collect::<Vec<_>>()
            .await;
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], Err(ZoneEditError::Ai(_))));
    }
}
