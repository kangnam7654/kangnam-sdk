use crate::api::{parse_birth_date, parse_birth_time};
use crate::calendar::normalize_birth_date;
use crate::chart::{ChartInput, calculate_chart};
use crate::types::{
    Branch, DecadeCycle, DomainFact, FourTransformation, MajorStar, NamedStarPlacement, PalaceName,
    StarRef, StarState, Stem, Transformation, Triad, TriadSummary, ZiweiCalculationProfile,
    ZiweiChart, ZiweiSourcePolicy,
};
use serde_json::{Value, json};

pub struct ZiweiEngine;

pub const ZIWEI_ENGINE_VERSION: &str = "ziwei-v0.3.2";
pub const ZIWEI_READING_TYPES: [&str; 2] = ["ziwei", "ziwei_chart"];

pub fn is_valid_reading_type(reading_type: &str) -> bool {
    ZIWEI_READING_TYPES.contains(&reading_type)
}

impl ZiweiEngine {
    pub fn generate(&self, reading_type: &str, input: &Value) -> (Value, String) {
        let version = ZIWEI_ENGINE_VERSION.to_string();
        if !is_valid_reading_type(reading_type) {
            return (
                json!({"error": format!("unknown ziwei reading type: {reading_type}")}),
                version,
            );
        }

        match chart_from_engine_input(input) {
            Ok(chart) => {
                let value = chart_to_json_with_version(&chart, &version);
                (value, version)
            }
            Err(error) => (json!({ "error": error }), version),
        }
    }
}

pub(crate) fn chart_from_engine_input(input: &Value) -> Result<ZiweiChart, String> {
    let birth_date = input
        .get("birth_date")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "birth_date is required".to_string())?;
    let birth_time = input
        .get("birth_time")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "birth_time is required for ziwei chart".to_string())?;
    let (year, month, day) = parse_birth_date(birth_date).map_err(|error| error.to_string())?;
    let (hour, minute) = parse_birth_time(birth_time).map_err(|error| error.to_string())?;
    let calendar_type = input
        .get("calendar_type")
        .and_then(|value| value.as_str())
        .unwrap_or("solar");
    let is_lunar_leap_month = input
        .get("is_lunar_leap_month")
        .or_else(|| input.get("lunar_leap_month"))
        .or_else(|| {
            input
                .get("options")
                .and_then(|options| options.get("is_lunar_leap_month"))
        })
        .or_else(|| {
            input
                .get("options")
                .and_then(|options| options.get("lunar_leap_month"))
        })
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let target_year = input
        .get("target_year")
        .or_else(|| {
            input
                .get("options")
                .and_then(|options| options.get("target_year"))
        })
        .and_then(|value| value.as_i64())
        .and_then(|value| i32::try_from(value).ok());
    let gender = input
        .get("gender")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());

    let normalized =
        normalize_birth_date(year, month, day, Some(calendar_type), is_lunar_leap_month)
            .ok_or_else(|| "invalid birth date or calendar_type".to_string())?;

    calculate_chart(ChartInput {
        birth: normalized,
        birth_time: format!("{hour:02}:{minute:02}"),
        hour,
        minute,
        target_year,
        gender,
    })
    .map_err(|error| error.to_string())
}

pub(crate) fn chart_to_json_with_version(chart: &ZiweiChart, engine_version: &str) -> Value {
    let mut value = chart_to_json(chart);
    if let Some(obj) = value.as_object_mut() {
        obj.insert("engine_version".to_string(), json!(engine_version));
    }
    value
}

pub(crate) fn chart_to_json(chart: &ZiweiChart) -> Value {
    json!({
        "chart_type": chart.chart_type,
        "schema_version": chart.schema_version,
        "calculation_profile": calculation_profile_json(&chart.calculation_profile),
        "birth": {
            "original_date": chart.birth.original_date,
            "solar_date": chart.birth.solar_date,
            "lunar_date": chart.birth.lunar_date,
            "birth_time": chart.birth.birth_time,
            "hour_branch": branch_json(chart.birth.hour_branch),
            "calendar_type": chart.birth.calendar_type,
            "is_lunar_leap_month": chart.birth.is_lunar_leap_month,
            "was_lunar_converted": chart.birth.was_lunar_converted,
        },
        "life_palace": branch_json(chart.life_palace),
        "body_palace": branch_json(chart.body_palace),
        "five_element_bureau": {
            "element": chart.five_element_bureau.element.code(),
            "element_ko": chart.five_element_bureau.element.korean(),
            "number": chart.five_element_bureau.number,
            "label": chart.five_element_bureau.label,
            "na_yin": chart.five_element_bureau.na_yin,
        },
        "ziwei_star": branch_json(chart.ziwei_star),
        "tianfu_star": branch_json(chart.tianfu_star),
        "palaces": chart.palaces.iter().map(|palace| {
            json!({
                "branch": branch_json(palace.branch),
                "stem": stem_json(palace.stem),
                "name": palace_json(palace.name),
                "major_stars": palace.major_stars.iter().map(|star| star_json(*star)).collect::<Vec<_>>(),
                "auxiliary_stars": palace.auxiliary_stars.iter().map(star_ref_json).collect::<Vec<_>>(),
                "malefic_stars": palace.malefic_stars.iter().map(star_ref_json).collect::<Vec<_>>(),
                "transformations": palace.transformations.iter().map(transformation_json).collect::<Vec<_>>(),
                "is_life_palace": palace.is_life_palace,
                "is_body_palace": palace.is_body_palace,
            })
        }).collect::<Vec<_>>(),
        "major_star_placements": chart.major_star_placements.iter().map(|placement| {
            json!({
                "star": star_json(placement.star),
                "branch": branch_json(placement.branch),
            })
        }).collect::<Vec<_>>(),
        "auxiliary_star_placements": chart.auxiliary_star_placements.iter().map(named_star_placement_json).collect::<Vec<_>>(),
        "malefic_star_placements": chart.malefic_star_placements.iter().map(named_star_placement_json).collect::<Vec<_>>(),
        "transformations": chart.transformations.iter().map(transformation_json).collect::<Vec<_>>(),
        "triads": chart.triads.iter().map(triad_json).collect::<Vec<_>>(),
        "star_states": chart.star_states.iter().map(star_state_json).collect::<Vec<_>>(),
        "triad_summaries": chart.triad_summaries.iter().map(triad_summary_json).collect::<Vec<_>>(),
        "decade_cycles": chart.decade_cycles.iter().map(decade_cycle_json).collect::<Vec<_>>(),
        "annual_flow": chart.annual_flow.as_ref().map(annual_flow_json),
        "chart_lords": chart.chart_lords.iter().map(chart_lord_json).collect::<Vec<_>>(),
        "domain_facts": chart.domain_facts.iter().map(domain_fact_json).collect::<Vec<_>>(),
        "calculation_basis": {
            "calculation_profile_id": chart.calculation_profile.id,
            "calculation_profile_version": chart.calculation_profile.version,
            "palace_rule": "yin_start_forward_month_reverse_hour",
            "body_palace_rule": "yin_start_forward_month_forward_hour",
            "bureau_rule": "life_palace_na_yin",
            "major_star_rule": "ziwei_bureau_day_then_14_major_stars",
            "auxiliary_star_rule": "zuofu_youbi_by_lunar_month_wenchang_wenqu_by_hour_tiankui_tianyue_by_year_stem",
            "malefic_star_rule": "qingyang_tuoluo_by_lucun_huoling_by_year_branch_hour_dikong_dijie_by_hour",
            "four_transformations_rule": "lunar_year_stem_transformation_table",
            "sanfang_sizheng_rule": "same_mod_4_branches_plus_opposite_branch",
            "star_state_rule": "major_star_branch_state_v0_heuristic_pending_authoritative_table",
            "decade_cycle_rule": "start_from_five_element_bureau_age_clockwise_default",
            "decade_direction_rule": "yang_male_yin_female_clockwise_else_counterclockwise",
            "chart_lord_rule": "ming_zhu_by_life_palace_shen_zhu_by_year_branch",
            "annual_flow_rule": "target_year_stem_branch_with_annual_life_palace",
            "domain_fact_rule": "palace_signal_counts_v0",
            "leap_month_policy": "same_lunar_month",
            "zi_hour_day_boundary": "civil_date_no_late_zi_day_shift",
        },
    })
}

fn calculation_profile_json(profile: &ZiweiCalculationProfile) -> Value {
    json!({
        "id": profile.id,
        "version": profile.version,
        "display_name": profile.display_name,
        "compatibility_target": profile.compatibility_target,
        "school_policy": profile.school_policy,
        "calendar_policy": profile.calendar_policy,
        "primary_reference": profile.primary_reference,
        "secondary_reference": profile.secondary_reference,
        "interpretation_policy": profile.interpretation_policy,
        "unsupported_policy": profile.unsupported_policy,
        "source_policies": profile.source_policies.iter().map(source_policy_json).collect::<Vec<_>>(),
    })
}

fn source_policy_json(policy: &ZiweiSourcePolicy) -> Value {
    json!({
        "tier": policy.tier,
        "role": policy.role,
        "allowed_use": policy.allowed_use,
    })
}

fn branch_json(branch: Branch) -> Value {
    json!({
        "code": branch.code(),
        "ko": branch.korean(),
        "hanja": branch.hanja(),
    })
}

fn stem_json(stem: Stem) -> Value {
    json!({
        "code": stem.code(),
        "ko": stem.korean(),
        "hanja": stem.hanja(),
    })
}

fn palace_json(palace: PalaceName) -> Value {
    json!({
        "code": palace.code(),
        "ko": palace.korean(),
    })
}

fn star_json(star: MajorStar) -> Value {
    json!({
        "code": star.code(),
        "ko": star.korean(),
        "hanja": star.hanja(),
    })
}

fn transformation_kind_json(kind: FourTransformation) -> Value {
    json!({
        "code": kind.code(),
        "ko": kind.korean(),
        "hanja": kind.hanja(),
    })
}

fn star_ref_json(star: &StarRef) -> Value {
    json!({
        "code": star.code,
        "ko": star.ko,
        "hanja": star.hanja,
    })
}

fn transformation_json(transformation: &Transformation) -> Value {
    json!({
        "type": transformation.kind.code(),
        "kind": transformation_kind_json(transformation.kind),
        "ko": transformation.kind.korean(),
        "hanja": transformation.kind.hanja(),
        "star": star_ref_json(&transformation.star),
        "palace": transformation.branch.map(branch_json),
        "placement_status": transformation.placement_status,
    })
}

fn named_star_placement_json(placement: &NamedStarPlacement) -> Value {
    json!({
        "star": star_ref_json(&placement.star),
        "branch": branch_json(placement.branch),
    })
}

fn triad_json(triad: &Triad) -> Value {
    json!({
        "palace": branch_json(triad.palace),
        "related_palaces": triad.related_palaces.iter().map(|branch| branch_json(*branch)).collect::<Vec<_>>(),
    })
}

fn star_state_json(state: &StarState) -> Value {
    json!({
        "star": star_ref_json(&state.star),
        "branch": branch_json(state.branch),
        "level": state.level,
        "label": state.label,
        "source_policy": state.source_policy,
    })
}

fn triad_summary_json(summary: &TriadSummary) -> Value {
    json!({
        "palace": branch_json(summary.palace),
        "related_palaces": summary.related_palaces.iter().map(|branch| branch_json(*branch)).collect::<Vec<_>>(),
        "major_star_count": summary.major_star_count,
        "auxiliary_star_count": summary.auxiliary_star_count,
        "malefic_star_count": summary.malefic_star_count,
        "transformation_count": summary.transformation_count,
    })
}

fn decade_cycle_json(cycle: &DecadeCycle) -> Value {
    json!({
        "index": cycle.index,
        "start_age": cycle.start_age,
        "end_age": cycle.end_age,
        "palace": branch_json(cycle.palace),
        "stem": stem_json(cycle.stem),
        "branch": branch_json(cycle.branch),
        "transformations": cycle.transformations.iter().map(transformation_json).collect::<Vec<_>>(),
        "direction": cycle.direction,
    })
}

fn chart_lord_json(lord: &crate::types::ChartLord) -> Value {
    json!({
        "kind": lord.kind,
        "star": star_ref_json(&lord.star),
        "basis": lord.basis,
    })
}

fn annual_flow_json(flow: &crate::types::AnnualFlow) -> Value {
    json!({
        "year": flow.year,
        "stem": stem_json(flow.stem),
        "branch": branch_json(flow.branch),
        "palace": branch_json(flow.palace),
        "transformations": flow.transformations.iter().map(transformation_json).collect::<Vec<_>>(),
        "source_policy": flow.source_policy,
    })
}

fn domain_fact_json(fact: &DomainFact) -> Value {
    json!({
        "domain": fact.domain,
        "palace": branch_json(fact.palace),
        "label": fact.label,
        "major_star_count": fact.major_star_count,
        "auxiliary_star_count": fact.auxiliary_star_count,
        "malefic_star_count": fact.malefic_star_count,
        "transformation_count": fact.transformation_count,
        "signal_level": fact.signal_level,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_returns_chart_json() {
        let input = json!({
            "birth_date": "1990-05-15",
            "birth_time": "14:30",
            "calendar_type": "solar"
        });

        let (result, version) = ZiweiEngine.generate("ziwei", &input);

        assert_eq!(version, ZIWEI_ENGINE_VERSION);
        assert_eq!(result["chart_type"], "ziwei");
        assert_eq!(result["palaces"].as_array().map(Vec::len), Some(12));
        assert_eq!(
            result["major_star_placements"].as_array().map(Vec::len),
            Some(14)
        );
        assert_eq!(result["transformations"].as_array().map(Vec::len), Some(4));
        assert_eq!(result["triads"].as_array().map(Vec::len), Some(12));
        assert_eq!(
            result["auxiliary_star_placements"].as_array().map(Vec::len),
            Some(10)
        );
        assert_eq!(
            result["malefic_star_placements"].as_array().map(Vec::len),
            Some(6)
        );
        assert_eq!(result["star_states"].as_array().map(Vec::len), Some(14));
        assert_eq!(result["triad_summaries"].as_array().map(Vec::len), Some(12));
        assert_eq!(result["decade_cycles"].as_array().map(Vec::len), Some(12));
        assert_eq!(result["chart_lords"].as_array().map(Vec::len), Some(2));
        assert!(result["annual_flow"].is_null());
        assert_eq!(result["domain_facts"].as_array().map(Vec::len), Some(12));
        assert_eq!(
            result["calculation_profile"]["id"],
            "iztro_compatible_kr_service"
        );
        assert_eq!(result["calculation_profile"]["version"], "v1");
        assert_eq!(
            result["calculation_profile"]["compatibility_target"],
            "iztro-compatible"
        );
        assert_eq!(
            result["calculation_profile"]["source_policies"]
                .as_array()
                .map(Vec::len),
            Some(3)
        );
        assert_eq!(
            result["calculation_basis"]["calculation_profile_id"],
            "iztro_compatible_kr_service"
        );
        assert_eq!(
            result["calculation_basis"]["four_transformations_rule"],
            "lunar_year_stem_transformation_table"
        );
        assert_eq!(
            result["transformations"][0]["placement_status"],
            "placed_major_star"
        );
    }

    #[test]
    fn missing_birth_time_returns_error_shape() {
        let input = json!({
            "birth_date": "1990-05-15",
            "calendar_type": "solar"
        });

        let (result, version) = ZiweiEngine.generate("ziwei", &input);

        assert_eq!(version, ZIWEI_ENGINE_VERSION);
        assert!(result["error"].as_str().is_some());
    }

    #[test]
    fn target_year_returns_annual_flow_json() {
        let input = json!({
            "birth_date": "1990-05-15",
            "birth_time": "14:30",
            "calendar_type": "solar",
            "target_year": 2026
        });

        let (result, _) = ZiweiEngine.generate("ziwei", &input);

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
}
