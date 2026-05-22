use serde_json::{Value, json};
use std::fmt;

use crate::category_meanings::is_valid_category;
use crate::draw::DRAW_POOL_SIZE;
use crate::engine::{TarotEngine, is_valid_reading_type};
use crate::types::SpreadType;

#[derive(Debug, Clone, Default)]
pub struct TarotEngineRequest<'a> {
    pub reading_type: &'a str,
    pub birth_date: Option<&'a str>,
    pub birth_time: Option<&'a str>,
    pub calendar_type: Option<&'a str>,
    pub category: Option<&'a str>,
    pub draw_index: Option<u64>,
    pub selected_position: Option<usize>,
    pub selected_positions: Option<&'a [usize]>,
}

#[derive(Debug, Clone)]
pub struct TarotEngineResponse {
    pub result_json: Value,
    pub engine_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TarotEngineError {
    UnknownReadingType(String),
    UnknownCategory(String),
    InvalidSelectedPosition(usize),
    InvalidSelectedPositions { expected: usize, actual: usize },
}

impl fmt::Display for TarotEngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownReadingType(value) => write!(f, "unknown tarot reading type: {value}"),
            Self::UnknownCategory(value) => write!(f, "unknown tarot category: {value}"),
            Self::InvalidSelectedPosition(value) => {
                write!(f, "selected position out of tarot draw pool: {value}")
            }
            Self::InvalidSelectedPositions { expected, actual } => write!(
                f,
                "invalid selected positions: expected {expected}, got {actual}"
            ),
        }
    }
}

impl std::error::Error for TarotEngineError {}

pub fn generate_public_daily_tarot(
    category: Option<&str>,
    draw_index: u64,
) -> Result<TarotEngineResponse, TarotEngineError> {
    generate_tarot_reading(TarotEngineRequest {
        reading_type: "tarot_daily",
        category,
        draw_index: Some(draw_index),
        calendar_type: Some("solar"),
        ..Default::default()
    })
}

pub fn generate_tarot_reading(
    request: TarotEngineRequest<'_>,
) -> Result<TarotEngineResponse, TarotEngineError> {
    if !is_valid_reading_type(request.reading_type) {
        return Err(TarotEngineError::UnknownReadingType(
            request.reading_type.to_string(),
        ));
    }

    let category = request
        .category
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(category) = category {
        if !is_valid_category(category) {
            return Err(TarotEngineError::UnknownCategory(category.to_string()));
        }
    }

    validate_selected_positions(&request)?;

    let input = build_engine_input(&request, category);
    let engine = TarotEngine;
    let (mut result_json, engine_version) = engine.generate(request.reading_type, &input);
    attach_engine_version(&mut result_json, &engine_version);

    Ok(TarotEngineResponse {
        result_json,
        engine_version,
    })
}

fn attach_engine_version(result_json: &mut Value, engine_version: &str) {
    if let Some(object) = result_json.as_object_mut() {
        object
            .entry("engine_version")
            .or_insert_with(|| json!(engine_version));
    }
}

fn build_engine_input(request: &TarotEngineRequest<'_>, category: Option<&str>) -> Value {
    let mut options = serde_json::Map::new();
    if let Some(category) = category {
        options.insert("category".into(), json!(category));
    }
    if let Some(draw_index) = request.draw_index {
        options.insert("draw_index".into(), json!(draw_index));
    }
    if let Some(selected_position) = request.selected_position {
        options.insert("selected_position".into(), json!(selected_position));
    }
    if let Some(selected_positions) = request.selected_positions {
        options.insert("selected_positions".into(), json!(selected_positions));
    }

    json!({
        "reading_type": request.reading_type,
        "birth_date": request.birth_date.unwrap_or(""),
        "birth_time": request.birth_time.unwrap_or(""),
        "calendar_type": request.calendar_type.unwrap_or("solar"),
        "options": Value::Object(options),
    })
}

fn validate_selected_positions(request: &TarotEngineRequest<'_>) -> Result<(), TarotEngineError> {
    if let Some(position) = request.selected_position {
        if position >= DRAW_POOL_SIZE as usize {
            return Err(TarotEngineError::InvalidSelectedPosition(position));
        }
    }

    let Some(positions) = request.selected_positions else {
        return Ok(());
    };
    let spread = SpreadType::from_reading_type(request.reading_type)
        .expect("reading_type already validated before selected positions");
    let expected = spread.card_count();
    if positions.len() != expected
        || positions
            .iter()
            .any(|position| *position >= DRAW_POOL_SIZE as usize)
        || {
            let mut sorted = positions.to_vec();
            sorted.sort_unstable();
            sorted.dedup();
            sorted.len() != positions.len()
        }
    {
        return Err(TarotEngineError::InvalidSelectedPositions {
            expected,
            actual: positions.len(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_level_public_daily_generates_calculation_only_result() {
        let response = generate_public_daily_tarot(Some("love"), 1).unwrap();

        assert_eq!(response.result_json["spread_type"], "tarot_daily");
        assert_eq!(
            response.result_json["engine_version"],
            response.engine_version
        );
        assert_eq!(response.result_json["reading_facts"]["category"], "love");
        assert!(
            response
                .result_json
                .get("interpretation_framework")
                .is_none()
        );
        assert!(response.result_json.get("overall_summary").is_none());
        assert!(
            response.result_json["cards"][0]
                .get("interpretation")
                .is_none()
        );
        assert!(
            response.result_json["cards"][0]
                .get("preview_text")
                .is_none()
        );
    }

    #[test]
    fn high_level_api_rejects_unknown_category() {
        let error = generate_public_daily_tarot(Some("bad"), 1).unwrap_err();
        assert_eq!(error, TarotEngineError::UnknownCategory("bad".into()));
    }

    #[test]
    fn high_level_api_rejects_unknown_reading_type() {
        let error = generate_tarot_reading(TarotEngineRequest {
            reading_type: "tarot_saju_fusion",
            ..Default::default()
        })
        .unwrap_err();

        assert_eq!(
            error,
            TarotEngineError::UnknownReadingType("tarot_saju_fusion".into())
        );
    }

    #[test]
    fn high_level_api_rejects_bad_selected_positions() {
        let error = generate_tarot_reading(TarotEngineRequest {
            reading_type: "tarot_three",
            selected_positions: Some(&[1, 1, 2]),
            ..Default::default()
        })
        .unwrap_err();

        assert_eq!(
            error,
            TarotEngineError::InvalidSelectedPositions {
                expected: 3,
                actual: 3
            }
        );
    }
}
