//! `<question-form>` JSON schema + validator. The model emits forms inline
//! using a small JSON dialect — we type-check shape so the renderer can
//! trust its inputs and the host can validate user responses round-trip.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum QuestionFormError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("missing required field `{0}`")]
    MissingField(&'static str),
    #[error("question[{idx}] has unsupported type `{kind}`")]
    UnknownType { idx: usize, kind: String },
    #[error("question[{idx}] checkbox.maxSelections must be a positive integer")]
    BadMaxSelections { idx: usize },
}

/// One question. Matches open-design's [`QuestionFormQuestion`][qf] shape.
///
/// [qf]: https://github.com/nexu-io/open-design/blob/main/src/artifacts/question-form.ts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub label: String,
    #[serde(rename = "type")]
    pub kind: QuestionType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    /// For `checkbox` only.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "maxSelections")]
    pub max_selections: Option<u32>,
    /// For `direction-cards`. Each card is a free-form JSON object so we
    /// don't have to model every visual detail in the type system.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cards: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QuestionType {
    Radio,
    Checkbox,
    Select,
    Text,
    Textarea,
    DirectionCards,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionForm {
    #[serde(default)]
    pub description: Option<String>,
    pub questions: Vec<Question>,
}

/// Parse and validate a question-form body (the JSON between
/// `<question-form>…</question-form>`).
pub fn parse_question_form(body: &str) -> Result<QuestionForm, QuestionFormError> {
    let form: QuestionForm = serde_json::from_str(body)?;
    for (idx, q) in form.questions.iter().enumerate() {
        // Checkbox sanity.
        if matches!(q.kind, QuestionType::Checkbox) {
            if let Some(0) = q.max_selections {
                return Err(QuestionFormError::BadMaxSelections { idx });
            }
        }
        if q.id.is_empty() {
            return Err(QuestionFormError::MissingField("id"));
        }
    }
    Ok(form)
}

/// User-side response to a posted form. The host posts this back over the
/// `cli.questionFormResponse` JSON-RPC method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormResponse {
    pub form_id: String,
    pub answers: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_discovery_form() {
        let body = r#"{
          "description": "Quick brief",
          "questions": [
            { "id": "output", "label": "What?", "type": "radio", "required": true,
              "options": ["A", "B"] },
            { "id": "tone", "label": "Tone", "type": "checkbox", "maxSelections": 2,
              "options": ["a", "b", "c"] },
            { "id": "audience", "label": "Who?", "type": "text",
              "placeholder": "investors" }
          ]
        }"#;
        let form = parse_question_form(body).unwrap();
        assert_eq!(form.questions.len(), 3);
        assert_eq!(form.questions[1].max_selections, Some(2));
        assert_eq!(form.questions[2].kind, QuestionType::Text);
    }

    #[test]
    fn rejects_zero_max_selections() {
        let body = r#"{"questions":[{"id":"x","label":"y","type":"checkbox","maxSelections":0}]}"#;
        assert!(matches!(
            parse_question_form(body).unwrap_err(),
            QuestionFormError::BadMaxSelections { idx: 0 }
        ));
    }

    #[test]
    fn rejects_unknown_type() {
        let body = r#"{"questions":[{"id":"x","label":"y","type":"emoji-picker"}]}"#;
        // serde fails first → Json error
        let err = parse_question_form(body).unwrap_err();
        assert!(matches!(err, QuestionFormError::Json(_)));
    }

    #[test]
    fn rejects_missing_id() {
        let body = r#"{"questions":[{"id":"","label":"y","type":"text"}]}"#;
        assert!(matches!(
            parse_question_form(body).unwrap_err(),
            QuestionFormError::MissingField("id")
        ));
    }

    #[test]
    fn direction_cards_carry_arbitrary_card_payloads() {
        let body = r##"{
          "questions": [
            { "id": "direction", "label": "Pick", "type": "direction-cards",
              "options": ["a","b"],
              "cards": [{"id":"a","palette":["#fff","#000"]},{"id":"b","palette":["#abc","#def"]}] }
          ]
        }"##;
        let form = parse_question_form(body).unwrap();
        assert_eq!(form.questions[0].cards.len(), 2);
    }
}
