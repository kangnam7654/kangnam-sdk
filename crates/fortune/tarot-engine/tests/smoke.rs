//! Public-API smoke lock for `TarotEngine::generate`.
//!
//! One test per spread the backend dispatches. Each asserts that
//! `generate` returns an object-shaped `Value` and a non-empty
//! version string.

use serde_json::json;
use tarot_engine::TarotEngine;

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
