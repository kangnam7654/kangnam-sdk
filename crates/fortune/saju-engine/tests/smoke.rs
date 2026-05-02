//! Public-API smoke lock for `SajuEngine::generate`.
//!
//! One test per `reading_type` the backend dispatches. Each asserts that
//! `generate` returns an object-shaped `Value` and a non-empty version
//! string. Missing-input branches are allowed to return an error object;
//! the contract being locked is *shape*, not *content*.

use saju_engine::SajuEngine;
use serde_json::json;

fn minimal_input() -> serde_json::Value {
    json!({
        "birth_date": "1990-05-15",
        "birth_time": "14:30",
        "calendar_type": "solar"
    })
}

fn compatibility_input() -> serde_json::Value {
    json!({
        "birth_date": "1990-05-15",
        "birth_time": "14:30",
        "calendar_type": "solar",
        "options": {
            "target_birth_date": "1992-08-20",
            "target_birth_time": "09:00",
            "target_calendar_type": "solar"
        }
    })
}

fn assert_shape(reading_type: &str, input: &serde_json::Value) {
    let engine = SajuEngine;
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
fn daily_returns_object() {
    assert_shape("daily", &minimal_input());
}

#[test]
fn daily_detail_returns_object() {
    assert_shape("daily_detail", &minimal_input());
}

#[test]
fn saju_returns_object() {
    assert_shape("saju", &minimal_input());
}

#[test]
fn saju_full_returns_object() {
    assert_shape("saju_full", &minimal_input());
}

#[test]
fn weekly_returns_object() {
    assert_shape("weekly", &minimal_input());
}

#[test]
fn monthly_returns_object() {
    assert_shape("monthly", &minimal_input());
}

#[test]
fn compatibility_returns_object() {
    assert_shape("compatibility", &compatibility_input());
}

#[test]
fn compatibility_detail_returns_object() {
    assert_shape("compatibility_detail", &compatibility_input());
}

#[test]
fn monthly_fortune_returns_object() {
    assert_shape("monthly_fortune", &minimal_input());
}

#[test]
fn daeun_returns_object() {
    assert_shape("daeun", &minimal_input());
}

#[test]
fn unknown_reading_type_still_returns_shape() {
    // Fallback branch must produce an object + version, not panic.
    assert_shape("not_a_real_type", &minimal_input());
}
