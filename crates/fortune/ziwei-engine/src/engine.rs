use crate::api::{parse_birth_date, parse_birth_time};
use crate::calendar::normalize_birth_date;
use crate::chart::{ChartInput, calculate_chart};
use crate::types::{Branch, MajorStar, PalaceName, Stem, ZiweiChart};
use serde_json::{Value, json};

pub struct ZiweiEngine;

pub const ZIWEI_ENGINE_VERSION: &str = "ziwei-v0.1.0";
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

    let normalized =
        normalize_birth_date(year, month, day, Some(calendar_type), is_lunar_leap_month)
            .ok_or_else(|| "invalid birth date or calendar_type".to_string())?;

    calculate_chart(ChartInput {
        birth: normalized,
        birth_time: format!("{hour:02}:{minute:02}"),
        hour,
        minute,
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
        "calculation_basis": {
            "palace_rule": "yin_start_forward_month_reverse_hour",
            "body_palace_rule": "yin_start_forward_month_forward_hour",
            "bureau_rule": "life_palace_na_yin",
            "major_star_rule": "ziwei_bureau_day_then_14_major_stars",
            "leap_month_policy": "same_lunar_month",
            "zi_hour_day_boundary": "civil_date_no_late_zi_day_shift",
        },
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
}
