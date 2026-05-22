//! Public-API smoke lock for `SajuEngine::generate`.
//!
//! One test per `reading_type` the backend dispatches. Each asserts that
//! `generate` returns an object-shaped `Value` and a non-empty version
//! string. Missing-input branches are allowed to return an error object;
//! the contract being locked is *shape*, not *content*.

use saju_engine::SajuEngine;
use serde_json::{Value, json};

const PROSE_KEYS: &[&str] = &[
    "legacy_prose",
    "personality",
    "fortune_outlook",
    "interpretation",
    "interpretation_policy",
    "lead",
    "analysis",
    "advice",
    "description",
    "meaning",
    "modern_take",
    "prompt",
    "prompts",
    "ai_prompts",
    "headline",
    "sections",
    "summary",
    "element_energy",
    "personality_summary",
    "persona_today",
    "saju_connection",
    "mixed_interpretation",
    "basic_interpretation",
    "saju_interpretation",
    "confidence_note",
    "preview_text",
    "overall_summary",
];

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

fn minimal_input_with_gender() -> serde_json::Value {
    json!({
        "birth_date": "1990-05-15",
        "birth_time": "14:30",
        "calendar_type": "solar",
        "gender": "male"
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
fn daily_returns_object() {
    assert_shape("daily", &minimal_input());
}

#[test]
fn daily_detail_returns_object() {
    assert_shape("daily_detail", &minimal_input());
}

#[test]
fn daily_detail_category_details_has_7_keys() {
    // lunawave /today 페이지가 의존하는 7 카테고리(love/career/health/wealth +
    // study/travel/relations) JSON 키를 lock — 빠지면 frontend의 카테고리 행이
    // 빈 점수(0점)로 노출되며 UX가 깨지므로 contract test로 고정.
    let (result, _v) = SajuEngine.generate("daily_detail", &minimal_input());
    let cats = result
        .get("category_details")
        .and_then(|v| v.as_object())
        .expect("category_details must be an object");
    for key in [
        "love",
        "career",
        "health",
        "wealth",
        "study",
        "travel",
        "relations",
    ] {
        let entry = cats
            .get(key)
            .unwrap_or_else(|| panic!("missing category: {key}"));
        let obj = entry.as_object().expect("category entry must be object");
        assert!(
            obj.get("advice").is_none(),
            "{key}.advice must stay backend-only"
        );
        let score = obj
            .get("score")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| panic!("{key}.score missing or non-integer"));
        assert!(
            (30..=98).contains(&score),
            "{key}.score out of range: {score}"
        );
    }
    // 결혼·자녀는 daily에 노출하지 않는다 — 키가 새로 들어가면 본명/일운 BM 분리가
    // 깨진 신호이므로 회귀 lock.
    assert!(
        !cats.contains_key("marriage"),
        "marriage must not appear in daily category_details (본명 전용 SKU)"
    );
    assert!(
        !cats.contains_key("children"),
        "children must not appear in daily category_details (본명 전용 SKU)"
    );
}

#[test]
fn saju_returns_object() {
    assert_shape("saju", &minimal_input());
}

#[test]
fn saju_returns_canonical_full_interpretation() {
    let (result, _v) = SajuEngine.generate("saju", &minimal_input());
    assert_eq!(result["schema_version"], "saju_core_v2");
    assert!(result.get("interpretation").is_none());
    assert!(result.get("headline").is_none());
    assert!(result.get("sections").is_none());
}

#[test]
fn saju_includes_calculation_fields() {
    let (result, version) = SajuEngine.generate("saju", &minimal_input_with_gender());

    assert_eq!(version, "saju-v1.7");
    assert_eq!(result["schema_version"], "saju_core_v2");
    assert_eq!(
        result["manseoryok"]["hidden_stems"]
            .as_array()
            .map(Vec::len),
        Some(4)
    );
    assert!(result["manseoryok"]["branch_interactions"].is_object());
    assert!(result["element_balance"].is_object());
    assert!(result["ten_gods_summary"].is_object());
    assert!(result["calculation_basis"].is_object());
    assert!(result["daeun_summary"].is_object());
    assert!(result["daeun_summary"]["daeun_start"].is_object());
    assert_eq!(
        result["manseoryok"]["fortune_cycles"]["monthly"]
            .as_array()
            .map(Vec::len),
        Some(12)
    );
    assert!(result["signals"].as_array().is_some_and(|v| !v.is_empty()));
    assert!(result["evidence"].as_array().is_some_and(|v| !v.is_empty()));
}

#[test]
fn all_saju_reading_types_are_calculation_only() {
    let cases = [
        ("daily", minimal_input()),
        ("daily_detail", minimal_input_with_gender()),
        ("saju", minimal_input_with_gender()),
        ("saju_wealth", minimal_input_with_gender()),
        ("saju_love", minimal_input_with_gender()),
        ("saju_marriage", minimal_input_with_gender()),
        ("saju_career", minimal_input_with_gender()),
        ("saju_health", minimal_input_with_gender()),
        ("saju_study", minimal_input_with_gender()),
        ("saju_children", minimal_input_with_gender()),
        ("saju_travel", minimal_input_with_gender()),
        ("saju_relations", minimal_input_with_gender()),
        ("weekly", minimal_input()),
        ("monthly", minimal_input()),
        ("compatibility", compatibility_input()),
        ("compatibility_detail", compatibility_input()),
        ("monthly_fortune", minimal_input()),
        ("daeun", minimal_input_with_gender()),
    ];

    for (reading_type, input) in cases {
        let (result, _version) = SajuEngine.generate(reading_type, &input);
        assert_no_prose_keys(reading_type, &result);
    }
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
