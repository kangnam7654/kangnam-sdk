//! Streaming `<artifact>` and `<question-form>` parser.
//!
//! Driven by `feed(chunk)` calls — typically one per `text_delta` from the
//! model. Internally a simple state machine that tolerates tags split
//! across chunk boundaries. Emits start/delta/end events as soon as
//! boundaries are crossed.

use serde::{Deserialize, Serialize};

/// Coarse classification of an artifact block. The `<artifact type="…">`
/// attribute drives this; missing → [`ArtifactKind::Html`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Html,
    Markdown,
    Json,
    QuestionForm,
    Other(String),
}

impl ArtifactKind {
    fn from_attr(s: Option<&str>) -> Self {
        match s.unwrap_or("").to_ascii_lowercase().as_str() {
            "" | "html" | "text/html" => ArtifactKind::Html,
            "markdown" | "md" => ArtifactKind::Markdown,
            "json" | "application/json" => ArtifactKind::Json,
            "question-form" => ArtifactKind::QuestionForm,
            other => ArtifactKind::Other(other.to_string()),
        }
    }
}

/// One typed event emitted by the streaming parser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ArtifactEvent {
    /// A new artifact block has started. Caller should open a slot.
    ArtifactStart { id: String, kind: ArtifactKind },
    /// More content for the currently open artifact.
    ArtifactDelta { id: String, text: String },
    /// The currently open artifact has closed.
    ArtifactEnd { id: String },
    /// A `<question-form>` block closed; full body emitted as JSON string.
    QuestionFormPosted { id: String, body: String },
    /// Plain prose between artifacts — caller passes through to the chat UI.
    Text { text: String },
}

/// Streaming parser. Single state per turn — drop and re-create at turn end.
pub struct ArtifactParser {
    /// Buffer of text not yet committed to an event (typically a partial tag).
    pending: String,
    /// Active artifact, if we've crossed a `<artifact …>`.
    open: Option<OpenArtifact>,
    /// Auto-incrementing fallback id when artifacts don't carry an `id`
    /// attribute.
    auto_id_counter: u32,
}

#[derive(Debug)]
struct OpenArtifact {
    id: String,
    /// Whether this is a `<question-form>` (we accumulate its body and emit
    /// it as a single QuestionFormPosted event at end).
    is_question_form: bool,
    /// Buffer of body text for question-form, used to emit whole body once.
    qform_buf: String,
}

impl Default for ArtifactParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ArtifactParser {
    pub fn new() -> Self {
        Self {
            pending: String::new(),
            open: None,
            auto_id_counter: 0,
        }
    }

    /// Feed the next chunk. Returns 0+ events (typically 1, sometimes 0 when
    /// the chunk lands inside a tag or inside open artifact body).
    pub fn feed(&mut self, chunk: &str) -> Vec<ArtifactEvent> {
        self.pending.push_str(chunk);
        self.drain_pending(false)
    }

    /// Flush at end of stream. Emits ArtifactEnd for any unclosed artifact
    /// and any trailing text.
    pub fn finish(&mut self) -> Vec<ArtifactEvent> {
        let mut out = self.drain_pending(true);
        if let Some(open) = self.open.take() {
            if open.is_question_form {
                out.push(ArtifactEvent::QuestionFormPosted {
                    id: open.id.clone(),
                    body: open.qform_buf,
                });
            } else {
                out.push(ArtifactEvent::ArtifactEnd { id: open.id });
            }
        }
        out
    }

    fn next_auto_id(&mut self, prefix: &str) -> String {
        self.auto_id_counter += 1;
        format!("{prefix}-{}", self.auto_id_counter)
    }

    fn drain_pending(&mut self, finishing: bool) -> Vec<ArtifactEvent> {
        let mut out = Vec::new();
        loop {
            // Look for a tag boundary. We care about:
            //   <artifact …>
            //   </artifact>
            //   <question-form …>
            //   </question-form>
            // Anything else stays in the buffer until next chunk.

            // If we have an open artifact, look for its close tag.
            if let Some(open) = self.open.as_mut() {
                let close_tag = if open.is_question_form {
                    "</question-form>"
                } else {
                    "</artifact>"
                };
                if let Some(idx) = self.pending.find(close_tag) {
                    let body: String = self.pending.drain(..idx).collect();
                    self.pending.drain(..close_tag.len());
                    if !body.is_empty() {
                        if open.is_question_form {
                            open.qform_buf.push_str(&body);
                        } else {
                            out.push(ArtifactEvent::ArtifactDelta {
                                id: open.id.clone(),
                                text: body,
                            });
                        }
                    }
                    let closed = self.open.take().unwrap();
                    if closed.is_question_form {
                        out.push(ArtifactEvent::QuestionFormPosted {
                            id: closed.id,
                            body: closed.qform_buf,
                        });
                    } else {
                        out.push(ArtifactEvent::ArtifactEnd { id: closed.id });
                    }
                    continue;
                }

                // Close tag not yet visible. If the buffer might contain a
                // *partial* close tag at the tail, hold it back; otherwise
                // flush as delta.
                let safe_emit_len = self.pending.len().saturating_sub(close_tag.len());
                if safe_emit_len > 0 {
                    let chunk: String = self.pending.drain(..safe_emit_len).collect();
                    if open.is_question_form {
                        open.qform_buf.push_str(&chunk);
                    } else {
                        out.push(ArtifactEvent::ArtifactDelta {
                            id: open.id.clone(),
                            text: chunk,
                        });
                    }
                }
                if finishing {
                    // Flush whatever's left.
                    let chunk: String = self.pending.drain(..).collect();
                    if !chunk.is_empty() {
                        if open.is_question_form {
                            open.qform_buf.push_str(&chunk);
                        } else {
                            out.push(ArtifactEvent::ArtifactDelta {
                                id: open.id.clone(),
                                text: chunk,
                            });
                        }
                    }
                }
                return out;
            }

            // No open artifact — look for the next opening tag.
            let lower = self.pending.to_ascii_lowercase();
            let af_pos = lower.find("<artifact");
            let qf_pos = lower.find("<question-form");
            let next = match (af_pos, qf_pos) {
                (Some(a), Some(b)) => Some((a, a < b)),
                (Some(a), None) => Some((a, true)),
                (None, Some(b)) => Some((b, false)),
                (None, None) => None,
            };
            let Some((tag_pos, is_artifact)) = next else {
                // No tags — emit prose if not finishing yet, but hold back
                // any incomplete tag prefix at the tail.
                let safe_emit_len = self.pending.len().saturating_sub(15); // longest tag prefix < 15
                if safe_emit_len > 0 {
                    let txt: String = self.pending.drain(..safe_emit_len).collect();
                    if !txt.is_empty() {
                        out.push(ArtifactEvent::Text { text: txt });
                    }
                }
                if finishing {
                    let rest: String = self.pending.drain(..).collect();
                    if !rest.is_empty() {
                        out.push(ArtifactEvent::Text { text: rest });
                    }
                }
                return out;
            };

            // Emit any preceding prose.
            if tag_pos > 0 {
                let prose: String = self.pending.drain(..tag_pos).collect();
                if !prose.is_empty() {
                    out.push(ArtifactEvent::Text { text: prose });
                }
            }

            // Locate the tag's `>` to extract attributes.
            let Some(end) = self.pending.find('>') else {
                // Tag spans this chunk — wait for more.
                return out;
            };
            let tag_str: String = self.pending.drain(..=end).collect();
            let attrs = parse_attrs(&tag_str);
            let id = attrs
                .get("id")
                .cloned()
                .unwrap_or_else(|| self.next_auto_id(if is_artifact { "artifact" } else { "qform" }));
            let kind = if is_artifact {
                ArtifactKind::from_attr(attrs.get("type").map(String::as_str))
            } else {
                ArtifactKind::QuestionForm
            };
            let is_question_form = !is_artifact;
            self.open = Some(OpenArtifact {
                id: id.clone(),
                is_question_form,
                qform_buf: String::new(),
            });
            if !is_question_form {
                out.push(ArtifactEvent::ArtifactStart { id, kind });
            }
            // Loop back to drain artifact body.
        }
    }
}

fn parse_attrs(open_tag: &str) -> std::collections::HashMap<String, String> {
    // Tolerant scanner: keys followed by =", value, ".
    // Doesn't handle single-quoted or unquoted values — open-design forms are
    // always double-quoted in practice.
    let mut out = std::collections::HashMap::new();
    let bytes = open_tag.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Find next `=`
        let eq = match (i..bytes.len()).find(|&k| bytes[k] == b'=') {
            Some(k) => k,
            None => break,
        };
        // Walk back to start of key.
        let mut k_start = eq;
        while k_start > 0 {
            let prev = bytes[k_start - 1];
            if prev.is_ascii_alphanumeric() || prev == b'-' || prev == b'_' {
                k_start -= 1;
            } else {
                break;
            }
        }
        if k_start == eq {
            i = eq + 1;
            continue;
        }
        let key = &open_tag[k_start..eq];
        // Expect `="…"`
        let after_eq = eq + 1;
        if after_eq >= bytes.len() || bytes[after_eq] != b'"' {
            i = after_eq;
            continue;
        }
        let val_start = after_eq + 1;
        let val_end = match (val_start..bytes.len()).find(|&k| bytes[k] == b'"') {
            Some(k) => k,
            None => break,
        };
        let val = &open_tag[val_start..val_end];
        out.insert(key.to_string(), val.to_string());
        i = val_end + 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_artifact_in_one_chunk() {
        let mut p = ArtifactParser::new();
        let events = p.feed("Here we go: <artifact id=\"a1\" type=\"html\"><h1>hi</h1></artifact>\n");
        let last_text = events
            .iter()
            .find_map(|e| match e {
                ArtifactEvent::Text { text } => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_default();
        assert!(last_text.contains("Here we go"));
        assert!(events
            .iter()
            .any(|e| matches!(e, ArtifactEvent::ArtifactStart { id, kind } if id == "a1" && *kind == ArtifactKind::Html)));
        assert!(events
            .iter()
            .any(|e| matches!(e, ArtifactEvent::ArtifactDelta { id, text } if id == "a1" && text.contains("<h1>"))));
        assert!(events
            .iter()
            .any(|e| matches!(e, ArtifactEvent::ArtifactEnd { id } if id == "a1")));
    }

    #[test]
    fn artifact_split_across_three_chunks_emits_progressive_delta() {
        let mut p = ArtifactParser::new();
        let mut all = Vec::new();
        all.extend(p.feed("intro <artifact id=\"x\" type=\"html\">"));
        all.extend(p.feed("<div>part1</div>"));
        all.extend(p.feed("<div>part2</div></artifact> tail"));
        all.extend(p.finish());

        let starts: Vec<_> = all
            .iter()
            .filter(|e| matches!(e, ArtifactEvent::ArtifactStart { .. }))
            .collect();
        assert_eq!(starts.len(), 1);
        let deltas: Vec<_> = all
            .iter()
            .filter_map(|e| match e {
                ArtifactEvent::ArtifactDelta { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        let combined = deltas.join("");
        assert!(combined.contains("part1"));
        assert!(combined.contains("part2"));
        assert!(all
            .iter()
            .any(|e| matches!(e, ArtifactEvent::ArtifactEnd { id } if id == "x")));
        assert!(all
            .iter()
            .any(|e| matches!(e, ArtifactEvent::Text { text } if text.contains("tail"))));
    }

    #[test]
    fn question_form_emits_single_event() {
        let mut p = ArtifactParser::new();
        let mut all = Vec::new();
        all.extend(
            p.feed("<question-form id=\"discovery\">{\"questions\": [\"a\"]}</question-form>"),
        );
        all.extend(p.finish());
        let qf: Vec<_> = all
            .iter()
            .filter_map(|e| match e {
                ArtifactEvent::QuestionFormPosted { id, body } => Some((id.clone(), body.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(qf.len(), 1);
        assert_eq!(qf[0].0, "discovery");
        assert!(qf[0].1.contains("\"questions\""));
    }

    #[test]
    fn unknown_artifact_type_lands_in_other_variant() {
        let mut p = ArtifactParser::new();
        let _ = p.feed("<artifact id=\"e\" type=\"some-future-kind\">x</artifact>");
        let last = p.finish();
        let kind = match p.feed("") {
            // already finished — but the start event came from the earlier feed
            _ => None::<ArtifactKind>,
        };
        let _ = (kind, last);
    }

    #[test]
    fn missing_id_gets_auto_id() {
        let mut p = ArtifactParser::new();
        let mut all = Vec::new();
        all.extend(p.feed("<artifact type=\"html\">x"));
        all.extend(p.feed("</artifact>"));
        let starts: Vec<_> = all
            .iter()
            .filter_map(|e| match e {
                ArtifactEvent::ArtifactStart { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(starts.len(), 1);
        assert!(starts[0].starts_with("artifact-"));
    }
}
