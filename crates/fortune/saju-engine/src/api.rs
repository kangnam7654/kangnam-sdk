use chrono::NaiveDate;
use serde_json::{Map, Value, json};
use std::fmt;

use crate::engine::{SajuEngine, is_valid_reading_type};

pub const SAJU_DAILY_CATEGORIES: [&str; 7] = [
    "love",
    "career",
    "health",
    "wealth",
    "study",
    "travel",
    "relations",
];

#[derive(Debug, Clone, Copy)]
pub struct BirthInput<'a> {
    pub birth_date: &'a str,
    pub birth_time: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedBirthTime {
    pub hour: u32,
    pub minute: u32,
    pub has_birth_time: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedBirth {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub has_birth_time: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SajuEngineRequest<'a> {
    pub reading_type: &'a str,
    pub birth_date: Option<&'a str>,
    pub birth_time: Option<&'a str>,
    pub calendar_type: Option<&'a str>,
    pub gender: Option<&'a str>,
    pub target_birth_date: Option<&'a str>,
    pub target_birth_time: Option<&'a str>,
    pub target_gender: Option<&'a str>,
    pub year: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct SajuEngineResponse {
    pub result_json: Value,
    pub engine_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SajuEngineError {
    UnknownReadingType(String),
    MissingBirthDate,
    InvalidBirthDate(String),
    InvalidBirthTime(String),
    MissingTargetBirthDate,
    InvalidTargetBirthDate(String),
    InvalidTargetBirthTime(String),
    EngineReturnedError(String),
}

impl fmt::Display for SajuEngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownReadingType(value) => write!(f, "unknown saju reading type: {value}"),
            Self::MissingBirthDate => write!(f, "birth_date is required"),
            Self::InvalidBirthDate(value) => write!(f, "invalid birth_date: {value}"),
            Self::InvalidBirthTime(value) => write!(f, "invalid birth_time: {value}"),
            Self::MissingTargetBirthDate => {
                write!(f, "target_birth_date is required for compatibility readings")
            }
            Self::InvalidTargetBirthDate(value) => {
                write!(f, "invalid target_birth_date: {value}")
            }
            Self::InvalidTargetBirthTime(value) => {
                write!(f, "invalid target_birth_time: {value}")
            }
            Self::EngineReturnedError(value) => write!(f, "saju engine returned error: {value}"),
        }
    }
}

impl std::error::Error for SajuEngineError {}

pub fn parse_birth_time(value: Option<&str>) -> Result<ParsedBirthTime, SajuEngineError> {
    parse_birth_time_with(value, SajuEngineError::InvalidBirthTime)
}

pub fn parse_birth_input(input: BirthInput<'_>) -> Result<ParsedBirth, SajuEngineError> {
    parse_birth_input_with(
        input,
        SajuEngineError::InvalidBirthDate,
        SajuEngineError::InvalidBirthTime,
    )
}

pub fn generate_daily_saju(
    birth_date: &str,
    birth_time: Option<&str>,
) -> Result<SajuEngineResponse, SajuEngineError> {
    generate_saju_reading(SajuEngineRequest {
        reading_type: "daily",
        birth_date: Some(birth_date),
        birth_time,
        calendar_type: Some("solar"),
        ..Default::default()
    })
}

pub fn generate_saju_profile(
    birth_date: &str,
    birth_time: Option<&str>,
    gender: Option<&str>,
) -> Result<SajuEngineResponse, SajuEngineError> {
    generate_saju_reading(SajuEngineRequest {
        reading_type: "saju",
        birth_date: Some(birth_date),
        birth_time,
        calendar_type: Some("solar"),
        gender,
        ..Default::default()
    })
}

pub fn generate_saju_compatibility(
    birth_date: &str,
    birth_time: Option<&str>,
    target_birth_date: &str,
    target_birth_time: Option<&str>,
) -> Result<SajuEngineResponse, SajuEngineError> {
    generate_saju_reading(SajuEngineRequest {
        reading_type: "compatibility",
        birth_date: Some(birth_date),
        birth_time,
        calendar_type: Some("solar"),
        target_birth_date: Some(target_birth_date),
        target_birth_time,
        ..Default::default()
    })
}

pub fn generate_saju_reading(
    request: SajuEngineRequest<'_>,
) -> Result<SajuEngineResponse, SajuEngineError> {
    if !is_valid_reading_type(request.reading_type) {
        return Err(SajuEngineError::UnknownReadingType(
            request.reading_type.to_string(),
        ));
    }

    let birth_date = request
        .birth_date
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(SajuEngineError::MissingBirthDate)?;
    let birth = parse_birth_input(BirthInput {
        birth_date,
        birth_time: request.birth_time,
    })?;

    let target_birth = if is_compatibility_reading(request.reading_type) {
        let target_birth_date = request
            .target_birth_date
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or(SajuEngineError::MissingTargetBirthDate)?;
        Some(parse_birth_input_with(
            BirthInput {
                birth_date: target_birth_date,
                birth_time: request.target_birth_time,
            },
            SajuEngineError::InvalidTargetBirthDate,
            SajuEngineError::InvalidTargetBirthTime,
        )?)
    } else {
        None
    };

    let input = build_engine_input(&request, &birth, target_birth.as_ref());
    let engine = SajuEngine;
    let (mut result_json, engine_version) = engine.generate(request.reading_type, &input);
    if let Some(error) = result_json.get("error").and_then(|value| value.as_str()) {
        return Err(SajuEngineError::EngineReturnedError(error.to_string()));
    }
    attach_engine_version(&mut result_json, &engine_version);

    Ok(SajuEngineResponse {
        result_json,
        engine_version,
    })
}

fn build_engine_input(
    request: &SajuEngineRequest<'_>,
    birth: &ParsedBirth,
    target_birth: Option<&ParsedBirth>,
) -> Value {
    let mut root = Map::new();
    root.insert("reading_type".into(), json!(request.reading_type));
    root.insert("birth_date".into(), json!(format_birth_date(birth)));
    if birth.has_birth_time {
        root.insert("birth_time".into(), json!(format_birth_time(birth)));
    }
    root.insert(
        "calendar_type".into(),
        json!(request.calendar_type.unwrap_or("solar")),
    );
    if let Some(gender) = non_empty(request.gender) {
        root.insert("gender".into(), json!(gender));
    }

    let mut options = Map::new();
    if let Some(year) = request.year {
        options.insert("year".into(), json!(year));
    }
    if let Some(target) = target_birth {
        options.insert("target_birth_date".into(), json!(format_birth_date(target)));
        if target.has_birth_time {
            options.insert("target_birth_time".into(), json!(format_birth_time(target)));
        }
        if let Some(gender) = non_empty(request.target_gender) {
            options.insert("target_gender".into(), json!(gender));
        }
    }
    root.insert("options".into(), Value::Object(options));

    Value::Object(root)
}

fn attach_engine_version(result_json: &mut Value, engine_version: &str) {
    if let Some(object) = result_json.as_object_mut() {
        object
            .entry("engine_version")
            .or_insert_with(|| json!(engine_version));
    }
}

fn parse_birth_input_with(
    input: BirthInput<'_>,
    date_error: fn(String) -> SajuEngineError,
    time_error: fn(String) -> SajuEngineError,
) -> Result<ParsedBirth, SajuEngineError> {
    let date = parse_birth_date(input.birth_date, date_error)?;
    let time = parse_birth_time_with(input.birth_time, time_error)?;

    Ok(ParsedBirth {
        year: date.year,
        month: date.month,
        day: date.day,
        hour: time.hour,
        minute: time.minute,
        has_birth_time: time.has_birth_time,
    })
}

struct ParsedDate {
    year: i32,
    month: u32,
    day: u32,
}

fn parse_birth_date(
    value: &str,
    make_error: fn(String) -> SajuEngineError,
) -> Result<ParsedDate, SajuEngineError> {
    let value = value.trim();
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != 3 {
        return Err(make_error(value.to_string()));
    }

    let year = parts[0]
        .parse::<i32>()
        .map_err(|_| make_error(value.to_string()))?;
    let month = parts[1]
        .parse::<u32>()
        .map_err(|_| make_error(value.to_string()))?;
    let day = parts[2]
        .parse::<u32>()
        .map_err(|_| make_error(value.to_string()))?;
    NaiveDate::from_ymd_opt(year, month, day).ok_or_else(|| make_error(value.to_string()))?;

    Ok(ParsedDate { year, month, day })
}

fn parse_birth_time_with(
    value: Option<&str>,
    make_error: fn(String) -> SajuEngineError,
) -> Result<ParsedBirthTime, SajuEngineError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(ParsedBirthTime {
            hour: 12,
            minute: 0,
            has_birth_time: false,
        });
    };

    let mut parts = value.split(':');
    let hour = parts
        .next()
        .ok_or_else(|| make_error(value.to_string()))?
        .parse::<u32>()
        .map_err(|_| make_error(value.to_string()))?;
    if hour > 23 {
        return Err(make_error(value.to_string()));
    }

    let minute = if let Some(minute) = parts.next() {
        let minute = minute
            .parse::<u32>()
            .map_err(|_| make_error(value.to_string()))?;
        if minute > 59 {
            return Err(make_error(value.to_string()));
        }
        minute
    } else {
        0
    };
    if parts.next().is_some() {
        return Err(make_error(value.to_string()));
    }

    Ok(ParsedBirthTime {
        hour,
        minute,
        has_birth_time: true,
    })
}

fn format_birth_date(birth: &ParsedBirth) -> String {
    format!("{:04}-{:02}-{:02}", birth.year, birth.month, birth.day)
}

fn format_birth_time(birth: &ParsedBirth) -> String {
    format!("{:02}:{:02}", birth.hour, birth.minute)
}

fn is_compatibility_reading(reading_type: &str) -> bool {
    matches!(reading_type, "compatibility" | "compatibility_detail")
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SAJU_ENGINE_VERSION, SAJU_READING_TYPES};

    #[test]
    fn high_level_api_generates_saju_result() {
        let response = generate_saju_profile("1990-05-15", Some("14:30"), Some("M")).unwrap();

        assert_eq!(response.engine_version, SAJU_ENGINE_VERSION);
        assert_eq!(response.result_json["engine_version"], SAJU_ENGINE_VERSION);
        assert!(response.result_json["four_pillars"].is_object());
    }

    #[test]
    fn high_level_api_rejects_unknown_reading_type() {
        let error = generate_saju_reading(SajuEngineRequest {
            reading_type: "tarot_daily",
            birth_date: Some("1990-05-15"),
            ..Default::default()
        })
        .unwrap_err();

        assert_eq!(
            error,
            SajuEngineError::UnknownReadingType("tarot_daily".into())
        );
    }

    #[test]
    fn high_level_api_rejects_invalid_birth_date() {
        let error = generate_daily_saju("2024-02-31", Some("14:00")).unwrap_err();

        assert_eq!(
            error,
            SajuEngineError::InvalidBirthDate("2024-02-31".into())
        );
    }

    #[test]
    fn high_level_api_rejects_invalid_birth_time() {
        let error = generate_daily_saju("2024-02-29", Some("24:00")).unwrap_err();

        assert_eq!(
            error,
            SajuEngineError::InvalidBirthTime("24:00".into())
        );
    }

    #[test]
    fn high_level_api_rejects_invalid_target_birth_date() {
        let error =
            generate_saju_compatibility("1990-05-15", None, "1992-02-31", Some("09:00"))
                .unwrap_err();

        assert_eq!(
            error,
            SajuEngineError::InvalidTargetBirthDate("1992-02-31".into())
        );
    }

    #[test]
    fn high_level_api_rejects_invalid_target_birth_time() {
        let error =
            generate_saju_compatibility("1990-05-15", None, "1992-02-20", Some("25:00"))
                .unwrap_err();

        assert_eq!(
            error,
            SajuEngineError::InvalidTargetBirthTime("25:00".into())
        );
    }

    #[test]
    fn high_level_api_generates_compatibility_without_birth_times() {
        let response =
            generate_saju_compatibility("1990-05-15", None, "1992-08-20", None).unwrap();

        assert_eq!(response.engine_version, SAJU_ENGINE_VERSION);
        assert!(response.result_json["subject_info"].is_object());
        assert!(response.result_json["target_info"].is_object());
    }

    #[test]
    fn public_reading_type_list_matches_validator() {
        for reading_type in SAJU_READING_TYPES {
            assert!(is_valid_reading_type(reading_type));
        }
        assert!(!is_valid_reading_type("saju_unknown"));
    }

    #[test]
    fn birth_parser_tracks_missing_time() {
        let parsed = parse_birth_input(BirthInput {
            birth_date: "1990-5-15",
            birth_time: None,
        })
        .unwrap();

        assert_eq!(parsed.year, 1990);
        assert_eq!(parsed.month, 5);
        assert_eq!(parsed.day, 15);
        assert_eq!(parsed.hour, 12);
        assert_eq!(parsed.minute, 0);
        assert!(!parsed.has_birth_time);
    }
}
