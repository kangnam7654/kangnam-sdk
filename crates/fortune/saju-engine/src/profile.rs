use serde_json::{Value, json};

pub const SAJU_PROFILE_ID: &str = "lunar_6tail_compatible_kr_service";
pub const SAJU_PROFILE_VERSION: &str = "v1";
pub const SAJU_COMPATIBILITY_TARGET: &str = "6tail-lunar-compatible";

pub fn calculation_profile_json() -> Value {
    json!({
        "id": SAJU_PROFILE_ID,
        "version": SAJU_PROFILE_VERSION,
        "display_name": "6tail lunar-compatible Korean saju service profile",
        "compatibility_target": SAJU_COMPATIBILITY_TARGET,
        "calendar_policy": "korean_lunar_calendar_rs_klc_kst",
        "sect_policy": "civil_date_no_late_zi_day_shift",
        "primary_reference": "6tail_lunar_python_lunar_javascript_eightchar_contract",
        "secondary_reference": "traditional_korean_manseoryok_tables_cross_check",
        "interpretation_policy": "dalgyeol_korean_service_copy_after_calculation",
        "unsupported_policy": "emit_explicit_approximate_or_pending_policy_for_rules_without_fixture_lock",
        "source_policies": [
            {
                "tier": "A",
                "role": "engine_calculation",
                "allowed_use": "open_source_contract_or_primary_table_fixture_required_before_authoritative_output"
            },
            {
                "tier": "B",
                "role": "cross_check",
                "allowed_use": "compare_multiple_manseoryok_sources_before_promoting_to_A"
            },
            {
                "tier": "C",
                "role": "interpretation_reference_only",
                "allowed_use": "copy_tone_or_explanation_never_core_calculation"
            }
        ]
    })
}
