use serde_json::json;
use ziwei_engine::{Branch, ZiweiEngine, generate_ziwei_chart};

fn minimal_input() -> serde_json::Value {
    json!({
        "birth_date": "1990-05-15",
        "birth_time": "14:30",
        "calendar_type": "solar"
    })
}

#[test]
fn ziwei_returns_object_shape() {
    let engine = ZiweiEngine;
    let (result, version) = engine.generate("ziwei", &minimal_input());

    assert!(result.is_object());
    assert!(!version.is_empty());
    assert_eq!(result["palaces"].as_array().map(Vec::len), Some(12));
    assert_eq!(
        result["major_star_placements"].as_array().map(Vec::len),
        Some(14)
    );
}

#[test]
fn ziwei_chart_alias_returns_object_shape() {
    let engine = ZiweiEngine;
    let (result, version) = engine.generate("ziwei_chart", &minimal_input());

    assert!(result.is_object());
    assert!(!version.is_empty());
    assert_eq!(result["chart_type"], "ziwei");
}

#[test]
fn unknown_reading_type_returns_error_object() {
    let engine = ZiweiEngine;
    let (result, version) = engine.generate("not_ziwei", &minimal_input());

    assert!(result.is_object());
    assert!(!version.is_empty());
    assert!(result["error"].as_str().is_some());
}

#[test]
fn public_api_returns_typed_chart() {
    let response = generate_ziwei_chart("1990-05-15", "14:30").unwrap();

    assert_eq!(response.chart.palaces.len(), 12);
    assert_eq!(response.chart.major_star_placements.len(), 14);
    assert_eq!(response.chart.birth.hour_branch, Branch::Wei);
    assert_eq!(response.result_json["chart_type"], "ziwei");
}

#[test]
fn json_contract_exposes_policy_and_localized_objects() {
    let result = generate_ziwei_chart("1990-05-15", "14:30")
        .unwrap()
        .result_json;

    assert_eq!(
        result["calculation_basis"]["leap_month_policy"],
        "same_lunar_month"
    );
    assert_eq!(
        result["calculation_basis"]["zi_hour_day_boundary"],
        "civil_date_no_late_zi_day_shift"
    );
    assert!(result["life_palace"]["code"].as_str().is_some());
    assert!(result["life_palace"]["ko"].as_str().is_some());
    assert!(result["life_palace"]["hanja"].as_str().is_some());
    assert!(
        result["palaces"]
            .as_array()
            .unwrap()
            .iter()
            .any(|palace| palace["name"]["ko"] == "부부궁")
    );
}
