use serde_json::{Value, json};

pub const TAROT_PROFILE_ID: &str = "rws_open_data_compatible_kr_service";
pub const TAROT_PROFILE_VERSION: &str = "v1";
pub const TAROT_COMPATIBILITY_TARGET: &str = "rider-waite-smith-open-data-compatible";

pub fn deck_source_profile_json() -> Value {
    json!({
        "id": TAROT_PROFILE_ID,
        "version": TAROT_PROFILE_VERSION,
        "display_name": "Rider-Waite-Smith open-data compatible Korean tarot service profile",
        "compatibility_target": TAROT_COMPATIBILITY_TARGET,
        "deck_reference": "Rider-Waite-Smith 78-card deck",
        "primary_reference": "ekelen_tarot_api_and_metabismuth_tarot_json_card_contract",
        "secondary_reference": "public_domain_rws_card_order_cross_check",
        "interpretation_policy": "dalgyeol_korean_service_copy_after_card_identity_and_orientation",
        "unsupported_policy": "non_RWS_decks_require_new_profile_id_and_fixture_set",
        "source_policies": [
            {
                "tier": "A",
                "role": "deck_identity",
                "allowed_use": "78_card_RWS_order_names_arcana_suits_numbers"
            },
            {
                "tier": "B",
                "role": "meaning_cross_check",
                "allowed_use": "public_API_or_JSON_meaning_data_for_consistency_checks"
            },
            {
                "tier": "C",
                "role": "interpretation_reference_only",
                "allowed_use": "tone_or_example_phrasing_never_card_identity"
            }
        ]
    })
}
