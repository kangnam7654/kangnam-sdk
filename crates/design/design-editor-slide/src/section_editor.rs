//! Regenerates a single `SlideDoc` section of a `SiteDoc` via the AI.
//!
//! Mirrors [`super::zone_editor::ZoneEditor`] in shape — a Tera template
//! renders the prompt, the AI streams text, and the final response is
//! parsed into a typed value. The difference is the payload: this editor
//! operates on an entire section (`SlideDoc`) rather than a single
//! element's HTML fragment, because web-mode section edits can rewrite
//! layout, background, and element set — not just copy.
//!
//! Handler-level concerns (loading the existing SiteDoc, splicing the
//! new section back in, creating a new Version row) live in
//! `handlers/canvas.rs`.
use std::sync::Arc;

use futures::stream::{BoxStream, StreamExt};
use tera::{Context, Tera};
use thiserror::Error;

use kangnam_design_llm::{AiChunk, AiClient, AiError};
use kangnam_design_doc_slide::SlideDoc;

const TEMPLATE_NAME: &str = "section-edit-prompt";
const TEMPLATE_SOURCE: &str =
    include_str!("../templates/section-edit-prompt.tera");

const ADD_TEMPLATE_NAME: &str = "section-add-prompt";
const ADD_TEMPLATE_SOURCE: &str =
    include_str!("../templates/section-add-prompt.tera");

/// Streaming events for a single section-edit call.
#[derive(Debug, Clone, PartialEq)]
pub enum SectionEditEvent {
    /// Raw AI text delta — forwarded unchanged so the chat can show
    /// incremental progress.
    Delta(String),
    /// Terminal event. Carries the parsed replacement `SlideDoc` (with
    /// the same `id` as the section being edited) and its canonical
    /// JSON serialization so the handler can splice without
    /// re-serializing.
    Complete { section: SlideDoc, section_json: String },
}

/// Streaming events for a section-add call. Shape parallels
/// [`SectionEditEvent`] but does NOT carry an id match guarantee: the
/// handler is responsible for overwriting `section.id` with a
/// server-generated value before splicing into the SiteDoc.
#[derive(Debug, Clone, PartialEq)]
pub enum SectionAddEvent {
    Delta(String),
    Complete { section: SlideDoc, section_json: String },
}

#[derive(Debug, Error)]
pub enum SectionEditError {
    #[error(transparent)]
    Ai(#[from] AiError),
    #[error("could not render section-edit prompt: {0}")]
    PromptRender(String),
    #[error("AI returned text that is not a valid section SlideDoc: {0}")]
    InvalidSection(String),
    /// Contract violation: the AI returned a SlideDoc whose `id` does
    /// not match the section being edited. We reject rather than
    /// silently correct so the model keeps learning the constraint.
    #[error("AI returned section id {got:?}, expected {expected:?}")]
    IdMismatch { expected: String, got: String },
}

pub struct SectionEditor {
    ai: Arc<dyn AiClient>,
    tera: Tera,
}

/// Caller-supplied fields for a single section-edit call.
pub struct SectionEditInput<'a> {
    pub section_id: &'a str,
    /// Optional human label (from `SlideDoc.title`) — included in the
    /// prompt so the model can orient itself without parsing the JSON.
    pub section_title: Option<&'a str>,
    /// Serialized JSON of the CURRENT section. Passed verbatim into the
    /// template so the model can see both the structure (ids, element
    /// frames) and the text content in one place.
    pub section_json: &'a str,
    pub instruction: &'a str,
}

/// Caller-supplied fields for a section-add call. The handler generates
/// the target section id server-side and rewrites the returned
/// `SlideDoc.id` after parsing — no id-match enforcement here.
pub struct SectionAddInput<'a> {
    /// Semantic kind label (hero / features / cta / …). Surfaced in the
    /// prompt so the model knows what shape of section to produce.
    pub section_kind: &'a str,
    /// When `Some`, the new section is described to the AI as being
    /// inserted AFTER this existing section id (for context only — the
    /// handler does the actual splicing).
    pub insert_after_id: Option<&'a str>,
    /// Short human-readable summary of the existing sections, e.g.
    /// `"- hero-1 (Hero)\n- cta-1 (CTA)"`. Rendered into the prompt so
    /// the model can match tone and avoid duplicating what's already
    /// there.
    pub existing_sections_summary: &'a str,
    pub instruction: &'a str,
}

impl SectionEditor {
    pub fn new(ai: Arc<dyn AiClient>) -> Result<Self, SectionEditError> {
        let mut tera = Tera::default();
        tera.add_raw_template(TEMPLATE_NAME, TEMPLATE_SOURCE)
            .map_err(|e| SectionEditError::PromptRender(e.to_string()))?;
        tera.add_raw_template(ADD_TEMPLATE_NAME, ADD_TEMPLATE_SOURCE)
            .map_err(|e| SectionEditError::PromptRender(e.to_string()))?;
        Ok(Self { ai, tera })
    }

    pub fn build_prompt(
        &self,
        input: &SectionEditInput<'_>,
    ) -> Result<String, SectionEditError> {
        let mut ctx = Context::new();
        ctx.insert("section_id", input.section_id);
        ctx.insert("section_title", input.section_title.unwrap_or(""));
        ctx.insert("section_json", input.section_json);
        ctx.insert("instruction", input.instruction);
        self.tera
            .render(TEMPLATE_NAME, &ctx)
            .map_err(|e| SectionEditError::PromptRender(e.to_string()))
    }

    pub fn build_add_prompt(
        &self,
        input: &SectionAddInput<'_>,
    ) -> Result<String, SectionEditError> {
        let mut ctx = Context::new();
        ctx.insert("section_kind", input.section_kind);
        ctx.insert("insert_after_id", &input.insert_after_id.unwrap_or(""));
        ctx.insert(
            "existing_sections_summary",
            input.existing_sections_summary,
        );
        ctx.insert("instruction", input.instruction);
        self.tera
            .render(ADD_TEMPLATE_NAME, &ctx)
            .map_err(|e| SectionEditError::PromptRender(e.to_string()))
    }

    /// Stream a brand-new section generation. Unlike [`Self::edit`], this
    /// does NOT enforce an id match against any existing section — the
    /// handler takes whatever id the AI returns, overwrites it with a
    /// server-generated `{kind}-{shortid}`, and splices the result into
    /// the SiteDoc at the desired index.
    pub fn add(
        &self,
        input: SectionAddInput<'_>,
    ) -> BoxStream<'static, Result<SectionAddEvent, SectionEditError>> {
        let prompt = match self.build_add_prompt(&input) {
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
                        yield Ok(SectionAddEvent::Delta(t));
                    }
                    Ok(AiChunk::Done { full_text: ai_full }) => {
                        let raw = if ai_full.is_empty() {
                            full_text.clone()
                        } else {
                            ai_full
                        };
                        match parse_section_lenient(&raw) {
                            Ok((section, section_json)) => {
                                yield Ok(SectionAddEvent::Complete {
                                    section,
                                    section_json,
                                });
                            }
                            Err(e) => {
                                yield Err(e);
                            }
                        }
                        return;
                    }
                    Err(e) => {
                        yield Err(SectionEditError::Ai(e));
                        return;
                    }
                }
            }
        };
        stream.boxed()
    }

    pub fn edit(
        &self,
        input: SectionEditInput<'_>,
    ) -> BoxStream<'static, Result<SectionEditEvent, SectionEditError>> {
        let prompt = match self.build_prompt(&input) {
            Ok(p) => p,
            Err(e) => return futures::stream::iter(vec![Err(e)]).boxed(),
        };
        // Snapshot the expected id so the async closure owns it.
        let expected_id = input.section_id.to_string();
        let upstream = self.ai.complete(prompt, Vec::new());

        let stream = async_stream::stream! {
            let mut upstream = upstream;
            let mut full_text = String::new();
            while let Some(item) = upstream.next().await {
                match item {
                    Ok(AiChunk::Delta(t)) => {
                        full_text.push_str(&t);
                        yield Ok(SectionEditEvent::Delta(t));
                    }
                    Ok(AiChunk::Done { full_text: ai_full }) => {
                        let raw = if ai_full.is_empty() {
                            full_text.clone()
                        } else {
                            ai_full
                        };
                        match parse_section(&raw, &expected_id) {
                            Ok((section, section_json)) => {
                                yield Ok(SectionEditEvent::Complete {
                                    section,
                                    section_json,
                                });
                            }
                            Err(e) => {
                                yield Err(e);
                            }
                        }
                        return;
                    }
                    Err(e) => {
                        yield Err(SectionEditError::Ai(e));
                        return;
                    }
                }
            }
        };
        stream.boxed()
    }
}

/// Parse the raw AI response into a validated `SlideDoc` + its canonical
/// JSON. Strips markdown fences, enforces that the returned `id` matches
/// the section being edited, and re-serializes so downstream consumers
/// get a clean, whitespace-normalized payload.
fn parse_section(
    raw: &str,
    expected_id: &str,
) -> Result<(SlideDoc, String), SectionEditError> {
    let cleaned = strip_code_fence(raw.trim()).trim();
    if cleaned.is_empty() {
        return Err(SectionEditError::InvalidSection(
            "empty response".to_string(),
        ));
    }
    let section: SlideDoc = serde_json::from_str(cleaned).map_err(|e| {
        SectionEditError::InvalidSection(format!(
            "{e} :: {}",
            truncate(cleaned, 200)
        ))
    })?;
    if section.id != expected_id {
        return Err(SectionEditError::IdMismatch {
            expected: expected_id.to_string(),
            got: section.id,
        });
    }
    let json = serde_json::to_string(&section)
        .map_err(|e| SectionEditError::InvalidSection(e.to_string()))?;
    Ok((section, json))
}

/// Parse a raw AI response as a SlideDoc without enforcing any id
/// constraint. Used by the `add` path where the server generates the
/// final id post-parse.
fn parse_section_lenient(raw: &str) -> Result<(SlideDoc, String), SectionEditError> {
    let cleaned = strip_code_fence(raw.trim()).trim();
    if cleaned.is_empty() {
        return Err(SectionEditError::InvalidSection(
            "empty response".to_string(),
        ));
    }
    let section: SlideDoc = serde_json::from_str(cleaned).map_err(|e| {
        SectionEditError::InvalidSection(format!(
            "{e} :: {}",
            truncate(cleaned, 200)
        ))
    })?;
    let json = serde_json::to_string(&section)
        .map_err(|e| SectionEditError::InvalidSection(e.to_string()))?;
    Ok((section, json))
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

    const NEW_HERO_JSON: &str = r##"{
        "id":"hero-1","width_px":1280,"height_px":720,
        "title":"Hero",
        "background":{"kind":"color","color":"#fff7ed"},
        "elements":[
            {"kind":"text","id":"hero-title","frame":{"x":0,"y":0,"w":1280,"h":200},
             "content":"<h1 class=\"text-6xl font-bold\">Welcome</h1>",
             "style":{"font_family":"Pretendard","font_size_px":48,"font_weight":700,
                      "color":"#111111","line_height":1.2,"letter_spacing_px":0.0,"align":"center"}}
        ]
    }"##;

    #[tokio::test]
    async fn build_prompt_includes_section_id_instruction_and_json() {
        let ai = Arc::new(FakeAiClient::new(Vec::<String>::new()));
        let svc = SectionEditor::new(ai).unwrap();
        let prompt = svc
            .build_prompt(&SectionEditInput {
                section_id: "hero-1",
                section_title: Some("Hero"),
                section_json: "{\"id\":\"hero-1\"}",
                instruction: "warm palette",
            })
            .unwrap();
        assert!(prompt.contains("`hero-1`"));
        assert!(prompt.contains("warm palette"));
        assert!(prompt.contains("Hero"));
        // Tailwind hint must be present — section edits share the web
        // context orientation with zone edits.
        assert!(prompt.contains("Tailwind"));
    }

    #[tokio::test]
    async fn edit_streams_delta_then_completes_with_parsed_section() {
        let ai = Arc::new(FakeAiClient::new(vec![NEW_HERO_JSON.to_string()]));
        let svc = SectionEditor::new(ai).unwrap();
        let mut stream = svc.edit(SectionEditInput {
            section_id: "hero-1",
            section_title: Some("Hero"),
            section_json: "{\"id\":\"hero-1\"}",
            instruction: "warm palette",
        });
        let mut deltas = Vec::<String>::new();
        let mut complete: Option<SectionEditEvent> = None;
        while let Some(item) = stream.next().await {
            match item.unwrap() {
                SectionEditEvent::Delta(d) => deltas.push(d),
                ev @ SectionEditEvent::Complete { .. } => {
                    complete = Some(ev);
                    break;
                }
            }
        }
        assert_eq!(deltas.len(), 1);
        let SectionEditEvent::Complete { section, section_json } =
            complete.expect("complete required")
        else {
            unreachable!()
        };
        assert_eq!(section.id, "hero-1");
        assert!(section_json.contains("fff7ed"));
    }

    #[tokio::test]
    async fn edit_rejects_id_mismatch() {
        // AI returns a SlideDoc with the WRONG id — we must reject so we
        // don't accidentally splice a hero-shaped doc over a cta section.
        let bad = NEW_HERO_JSON.replace("hero-1", "cta-1");
        let ai = Arc::new(FakeAiClient::new(vec![bad]));
        let svc = SectionEditor::new(ai).unwrap();
        let mut stream = svc.edit(SectionEditInput {
            section_id: "hero-1",
            section_title: Some("Hero"),
            section_json: "{\"id\":\"hero-1\"}",
            instruction: "x",
        });
        let mut err: Option<SectionEditError> = None;
        while let Some(item) = stream.next().await {
            if let Err(e) = item {
                err = Some(e);
                break;
            }
        }
        assert!(matches!(err, Some(SectionEditError::IdMismatch { .. })));
    }

    #[tokio::test]
    async fn edit_strips_markdown_fences() {
        let fenced = format!("```json\n{NEW_HERO_JSON}\n```");
        let ai = Arc::new(FakeAiClient::new(vec![fenced]));
        let svc = SectionEditor::new(ai).unwrap();
        let mut stream = svc.edit(SectionEditInput {
            section_id: "hero-1",
            section_title: Some("Hero"),
            section_json: "{\"id\":\"hero-1\"}",
            instruction: "x",
        });
        let mut ok = false;
        while let Some(item) = stream.next().await {
            if let Ok(SectionEditEvent::Complete { .. }) = item {
                ok = true;
                break;
            }
        }
        assert!(ok, "must complete after fence stripping");
    }

    #[tokio::test]
    async fn edit_rejects_non_json_response() {
        let ai = Arc::new(FakeAiClient::new(vec!["not json at all".to_string()]));
        let svc = SectionEditor::new(ai).unwrap();
        let mut stream = svc.edit(SectionEditInput {
            section_id: "hero-1",
            section_title: None,
            section_json: "{}",
            instruction: "x",
        });
        let mut err: Option<SectionEditError> = None;
        while let Some(item) = stream.next().await {
            if let Err(e) = item {
                err = Some(e);
                break;
            }
        }
        assert!(matches!(err, Some(SectionEditError::InvalidSection(_))));
    }

    #[tokio::test]
    async fn build_add_prompt_includes_kind_and_insert_position() {
        let ai = Arc::new(FakeAiClient::new(Vec::<String>::new()));
        let svc = SectionEditor::new(ai).unwrap();
        let prompt = svc
            .build_add_prompt(&SectionAddInput {
                section_kind: "features",
                insert_after_id: Some("hero-1"),
                existing_sections_summary: "- hero-1 (Hero)\n- cta-1 (CTA)",
                instruction: "three product benefits",
            })
            .unwrap();
        assert!(prompt.contains("features"));
        assert!(prompt.contains("`hero-1`"));
        assert!(prompt.contains("three product benefits"));
        assert!(prompt.contains("hero-1 (Hero)"));
        assert!(prompt.contains("Tailwind"));
    }

    #[tokio::test]
    async fn build_add_prompt_when_insert_after_is_none_says_append() {
        let ai = Arc::new(FakeAiClient::new(Vec::<String>::new()));
        let svc = SectionEditor::new(ai).unwrap();
        let prompt = svc
            .build_add_prompt(&SectionAddInput {
                section_kind: "cta",
                insert_after_id: None,
                existing_sections_summary: "",
                instruction: "final pitch",
            })
            .unwrap();
        // Korean template copy: "마지막" appears in the append branch.
        assert!(
            prompt.contains("마지막"),
            "append branch must surface \"last\" wording: {prompt}"
        );
    }

    #[tokio::test]
    async fn add_streams_delta_then_complete_with_any_id() {
        // Lenient parse — any SlideDoc-shaped response is accepted. The
        // handler will later overwrite the id.
        let ai = Arc::new(FakeAiClient::new(vec![NEW_HERO_JSON.to_string()]));
        let svc = SectionEditor::new(ai).unwrap();
        let mut stream = svc.add(SectionAddInput {
            section_kind: "hero",
            insert_after_id: None,
            existing_sections_summary: "",
            instruction: "x",
        });
        let mut complete: Option<SectionAddEvent> = None;
        while let Some(item) = stream.next().await {
            if let Ok(ev @ SectionAddEvent::Complete { .. }) = item {
                complete = Some(ev);
                break;
            }
        }
        let SectionAddEvent::Complete { section, .. } = complete.unwrap() else {
            unreachable!()
        };
        // The input's hero-1 id round-trips; the handler (not this
        // service) is responsible for overwriting it.
        assert_eq!(section.id, "hero-1");
    }

    #[tokio::test]
    async fn add_rejects_non_json() {
        let ai = Arc::new(FakeAiClient::new(vec!["notjson".to_string()]));
        let svc = SectionEditor::new(ai).unwrap();
        let mut stream = svc.add(SectionAddInput {
            section_kind: "cta",
            insert_after_id: None,
            existing_sections_summary: "",
            instruction: "x",
        });
        let mut err: Option<SectionEditError> = None;
        while let Some(item) = stream.next().await {
            if let Err(e) = item {
                err = Some(e);
                break;
            }
        }
        assert!(matches!(err, Some(SectionEditError::InvalidSection(_))));
    }

    #[tokio::test]
    async fn edit_surfaces_ai_errors() {
        let ai = Arc::new(FakeAiClient::failing("upstream boom"));
        let svc = SectionEditor::new(ai).unwrap();
        let mut stream = svc.edit(SectionEditInput {
            section_id: "hero-1",
            section_title: None,
            section_json: "{}",
            instruction: "x",
        });
        let mut err: Option<SectionEditError> = None;
        while let Some(item) = stream.next().await {
            if let Err(e) = item {
                err = Some(e);
                break;
            }
        }
        assert!(matches!(err, Some(SectionEditError::Ai(_))));
    }
}
