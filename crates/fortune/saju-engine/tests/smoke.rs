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
        let advice = obj
            .get("advice")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("{key}.advice missing or non-string"));
        assert!(!advice.is_empty(), "{key}.advice empty");
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
    let interp = result.get("interpretation").expect("interpretation field");
    assert!(interp.get("headline").is_some(), "headline required");
    assert!(
        interp.get("summary").is_none(),
        "canonical saju report must not use the legacy simple summary tier"
    );
    let sections = interp
        .get("sections")
        .and_then(|v| v.as_array())
        .expect("saju must include sections array");
    assert_eq!(sections.len(), 5, "canonical saju report has 5 sections");
}

#[test]
fn saju_includes_full_report_enrichment_fields() {
    let (result, version) = SajuEngine.generate("saju", &minimal_input_with_gender());

    assert_eq!(version, "saju-v1.7");
    assert_eq!(result["dalgyeol_enrichment_version"], "saju_extended_v1");
    assert_eq!(result["hidden_stems"].as_array().map(Vec::len), Some(4));
    assert!(result["branch_relations"].as_array().is_some());
    assert_eq!(result["naeum"].as_array().map(Vec::len), Some(4));
    assert!(result["yin_yang_balance"].is_object());
    assert!(result["ten_gods_summary"].is_object());
    assert!(result["wolryeong"].is_object());
    assert!(result["seasonal_energy"].is_object());
    assert!(result["strength_profile"].is_object());
    assert!(result["calculation_basis"].is_object());
    assert!(result["daeun_summary"].is_object());
    assert!(result["daeun_summary"]["daeun_start"].is_object());
    assert!(result["annual_fortune"].is_object());
    assert_eq!(
        result["monthly_fortunes"].as_array().map(Vec::len),
        Some(12)
    );
    assert!(
        result["life_timeline"]
            .as_array()
            .is_some_and(|v| v.len() >= 7)
    );
    assert!(
        result["domain_fortunes"]
            .as_array()
            .is_some_and(|v| v.len() >= 14)
    );
    assert!(
        result["ai_prompts"]
            .as_array()
            .is_some_and(|v| v.len() >= 4)
    );
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
