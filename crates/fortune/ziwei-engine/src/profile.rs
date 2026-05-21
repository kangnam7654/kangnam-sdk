use crate::types::{ZiweiCalculationProfile, ZiweiSourcePolicy};

pub const ZIWEI_PROFILE_ID: &str = "iztro_compatible_kr_service";
pub const ZIWEI_PROFILE_VERSION: &str = "v1";
pub const ZIWEI_PROFILE_DISPLAY_NAME: &str = "iztro-compatible Korean service profile";
pub const ZIWEI_COMPATIBILITY_TARGET: &str = "iztro-compatible";
pub const ZIWEI_STAR_STATE_SOURCE_POLICY: &str =
    "major_star_branch_state_v0_heuristic_pending_authoritative_table";

pub fn calculation_profile() -> ZiweiCalculationProfile {
    ZiweiCalculationProfile {
        id: ZIWEI_PROFILE_ID.to_string(),
        version: ZIWEI_PROFILE_VERSION.to_string(),
        display_name: ZIWEI_PROFILE_DISPLAY_NAME.to_string(),
        compatibility_target: ZIWEI_COMPATIBILITY_TARGET.to_string(),
        school_policy: "sanhe_first_with_iztro_compatibility".to_string(),
        calendar_policy: "korean_lunar_calendar_rs_klc_kst".to_string(),
        primary_reference: "iztro_open_source_calculation_contract".to_string(),
        secondary_reference: "traditional_ziwei_tables_cross_check".to_string(),
        interpretation_policy: "dalgyeol_korean_service_copy_after_calculation".to_string(),
        unsupported_policy:
            "emit_explicit_pending_policy_for_tables_without_authoritative_fixture_lock".to_string(),
        source_policies: vec![
            ZiweiSourcePolicy {
                tier: "A".to_string(),
                role: "engine_calculation".to_string(),
                allowed_use:
                    "open_source_or_primary_table_fixtures_required_before_authoritative_output"
                        .to_string(),
            },
            ZiweiSourcePolicy {
                tier: "B".to_string(),
                role: "cross_check".to_string(),
                allowed_use: "compare_multiple_public_tables_before promoting_to_A".to_string(),
            },
            ZiweiSourcePolicy {
                tier: "C".to_string(),
                role: "interpretation_reference_only".to_string(),
                allowed_use: "copy_tone_or_explanation_never_core_calculation".to_string(),
            },
        ],
    }
}
