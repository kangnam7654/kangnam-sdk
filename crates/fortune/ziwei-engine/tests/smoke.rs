use serde_json::json;
use ziwei_engine::{
    Branch, ZIWEI_PROFILE_ID, ZIWEI_PROFILE_VERSION, ZiweiEngine, ZiweiEngineRequest,
    generate_ziwei_chart, generate_ziwei_reading,
};

fn minimal_input() -> serde_json::Value {
    json!({
        "birth_date": "1990-05-15",
        "birth_time": "14:30",
        "calendar_type": "solar"
    })
}

fn target_year_input() -> serde_json::Value {
    json!({
        "birth_date": "1990-05-15",
        "birth_time": "14:30",
        "calendar_type": "solar",
        "target_year": 2026
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
    assert_eq!(
        result["auxiliary_star_placements"].as_array().map(Vec::len),
        Some(10)
    );
    assert_eq!(
        result["malefic_star_placements"].as_array().map(Vec::len),
        Some(6)
    );
    assert_eq!(result["transformations"].as_array().map(Vec::len), Some(4));
    assert_eq!(result["triads"].as_array().map(Vec::len), Some(12));
    assert_eq!(result["star_states"].as_array().map(Vec::len), Some(14));
    assert_eq!(result["triad_summaries"].as_array().map(Vec::len), Some(12));
    assert_eq!(result["decade_cycles"].as_array().map(Vec::len), Some(12));
    assert_eq!(result["chart_lords"].as_array().map(Vec::len), Some(2));
    assert_eq!(result["domain_facts"].as_array().map(Vec::len), Some(12));
    assert_eq!(result["calculation_profile"]["id"], ZIWEI_PROFILE_ID);
    assert_eq!(
        result["calculation_profile"]["version"],
        ZIWEI_PROFILE_VERSION
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
    assert_eq!(response.chart.calculation_profile.id, ZIWEI_PROFILE_ID);
    assert_eq!(
        response.chart.calculation_profile.compatibility_target,
        "iztro-compatible"
    );
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
    assert_eq!(
        result["calculation_basis"]["four_transformations_rule"],
        "lunar_year_stem_transformation_table"
    );
    assert_eq!(
        result["calculation_basis"]["auxiliary_star_rule"],
        "zuofu_youbi_by_lunar_month_wenchang_wenqu_by_hour_tiankui_tianyue_by_year_stem"
    );
    assert_eq!(
        result["calculation_basis"]["malefic_star_rule"],
        "qingyang_tuoluo_by_lucun_huoling_by_year_branch_hour_dikong_dijie_by_hour"
    );
    assert_eq!(
        result["calculation_basis"]["decade_cycle_rule"],
        "start_from_five_element_bureau_age_clockwise_default"
    );
    assert_eq!(
        result["calculation_basis"]["annual_flow_rule"],
        "target_year_stem_branch_with_annual_life_palace"
    );
    assert_eq!(
        result["calculation_basis"]["sanfang_sizheng_rule"],
        "same_mod_4_branches_plus_opposite_branch"
    );
    assert_eq!(
        result["calculation_basis"]["calculation_profile_id"],
        ZIWEI_PROFILE_ID
    );
    assert_eq!(
        result["calculation_basis"]["calculation_profile_version"],
        ZIWEI_PROFILE_VERSION
    );
    assert_eq!(
        result["calculation_profile"]["unsupported_policy"],
        "emit_explicit_pending_policy_for_tables_without_authoritative_fixture_lock"
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
    assert!(
        result["transformations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["ko"] == "화록"
                && item["star"]["ko"].as_str().is_some()
                && item["placement_status"].as_str().is_some())
    );
    assert!(result["palaces"].as_array().unwrap().iter().any(|palace| {
        palace["auxiliary_stars"].as_array().unwrap().len()
            + palace["malefic_stars"].as_array().unwrap().len()
            > 0
    }));
    assert!(
        result["domain_facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|fact| fact["domain"] == "life" && fact["signal_level"].as_str().is_some())
    );
}

#[test]
fn target_year_input_exposes_annual_flow() {
    let engine = ZiweiEngine;
    let (result, _) = engine.generate("ziwei", &target_year_input());

    assert_eq!(result["annual_flow"]["year"], 2026);
    assert_eq!(result["annual_flow"]["stem"]["code"], "bing");
    assert_eq!(result["annual_flow"]["branch"]["code"], "wu");
    assert_eq!(
        result["annual_flow"]["transformations"]
            .as_array()
            .map(Vec::len),
        Some(4)
    );
}

#[test]
fn typed_api_accepts_gender_and_target_year() {
    let response = generate_ziwei_reading(ZiweiEngineRequest {
        reading_type: "ziwei",
        birth_date: Some("1990-05-15"),
        birth_time: Some("14:30"),
        calendar_type: Some("solar"),
        is_lunar_leap_month: false,
        gender: Some("female"),
        target_year: Some(2026),
    })
    .unwrap();

    assert_eq!(response.result_json["annual_flow"]["year"], 2026);
    assert_eq!(
        response.result_json["decade_cycles"][0]["direction"],
        "counterclockwise"
    );
}
