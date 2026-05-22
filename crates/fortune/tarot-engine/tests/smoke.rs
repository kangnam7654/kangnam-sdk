//! Public-API smoke lock for `TarotEngine::generate`.
//!
//! One test per spread the backend dispatches. Each asserts that
//! `generate` returns an object-shaped `Value` and a non-empty
//! version string.

use serde_json::{Value, json};
use tarot_engine::TarotEngine;

const PROSE_KEYS: &[&str] = &[
    "overall_summary",
    "interpretation",
    "interpretation_policy",
    "interpretation_framework",
    "lead",
    "preview_text",
    "meaning",
    "meaning_upright",
    "meaning_reversed",
    "saju_connection",
    "mixed_interpretation",
    "basic_interpretation",
    "saju_interpretation",
    "source_archetype",
    "keywords",
];

fn minimal_input() -> serde_json::Value {
    json!({
        "birth_date": "1990-05-15",
        "birth_time": "14:30",
        "calendar_type": "solar"
    })
}

fn assert_shape(reading_type: &str, input: &serde_json::Value) {
    let engine = TarotEngine;
    let (result, version) = engine.generate(reading_type, input);
    assert!(
        result.is_object(),
        "reading_type={reading_type}: expected object, got {result}"
    );
    assert!(
        !version.is_empty(),
        "reading_type={reading_type}: version must be non-empty"
    );
}

fn assert_no_prose_keys(reading_type: &str, value: &Value) {
    fn visit(reading_type: &str, path: &str, value: &Value) {
        match value {
            Value::Object(map) => {
                for key in PROSE_KEYS {
                    assert!(
                        !map.contains_key(*key),
                        "reading_type={reading_type}: prose key {key} at {path}"
                    );
                }
                for (key, child) in map {
                    visit(reading_type, &format!("{path}.{key}"), child);
                }
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    visit(reading_type, &format!("{path}[{index}]"), child);
                }
            }
            _ => {}
        }
    }

    visit(reading_type, "$", value);
}

#[test]
fn tarot_daily_returns_object() {
    assert_shape("tarot_daily", &minimal_input());
}

#[test]
fn tarot_one_returns_object() {
    assert_shape("tarot_one", &minimal_input());
}

#[test]
fn tarot_one_preview_returns_object() {
    assert_shape("tarot_one_preview", &minimal_input());
}

#[test]
fn tarot_three_returns_object() {
    assert_shape("tarot_three", &minimal_input());
}

#[test]
fn tarot_celtic_returns_object() {
    assert_shape("tarot_celtic", &minimal_input());
}

#[test]
fn unknown_reading_type_still_returns_shape() {
    // Error branch returns an object `{"error": "..."}` + version.
    assert_shape("not_a_tarot_spread", &minimal_input());
}

#[test]
fn all_tarot_reading_types_are_calculation_only() {
    for reading_type in [
        "tarot_daily",
        "tarot_one",
        "tarot_one_preview",
        "tarot_three",
        "tarot_celtic",
    ] {
        let (result, _version) = TarotEngine.generate(reading_type, &minimal_input());
        assert_no_prose_keys(reading_type, &result);
    }
}

#[test]
fn public_card_catalog_exposes_identity_only() {
    let card = tarot_engine::cards::get_card(0).expect("major arcana card");
    let serialized = serde_json::to_value(card).expect("serialize public card");
    assert_no_prose_keys("public_card_catalog", &serialized);

    let debug = format!("{card:?}");
    assert!(!debug.contains("keywords"));
    assert!(!debug.contains("upright_meaning"));
    assert!(!debug.contains("reversed_meaning"));
}
