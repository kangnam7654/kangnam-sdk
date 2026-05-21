use crate::calendar::normalize_birth_date;
use crate::chart::{ChartInput, calculate_chart};
use crate::engine::{ZIWEI_ENGINE_VERSION, chart_to_json_with_version, is_valid_reading_type};
use crate::types::ZiweiChart;
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, Default)]
pub struct ZiweiEngineRequest<'a> {
    pub reading_type: &'a str,
    pub birth_date: Option<&'a str>,
    pub birth_time: Option<&'a str>,
    pub calendar_type: Option<&'a str>,
    pub is_lunar_leap_month: bool,
    pub gender: Option<&'a str>,
    pub target_year: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct ZiweiEngineResponse {
    pub chart: ZiweiChart,
    pub result_json: Value,
    pub engine_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZiweiEngineError {
    UnknownReadingType(String),
    MissingBirthDate,
    MissingBirthTime,
    InvalidBirthDate(String),
    InvalidBirthTime(String),
    InvalidCalendarType(String),
    EngineReturnedError(String),
}

impl fmt::Display for ZiweiEngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownReadingType(value) => write!(f, "unknown ziwei reading type: {value}"),
            Self::MissingBirthDate => write!(f, "birth_date is required"),
            Self::MissingBirthTime => write!(f, "birth_time is required"),
            Self::InvalidBirthDate(value) => write!(f, "invalid birth_date: {value}"),
            Self::InvalidBirthTime(value) => write!(f, "invalid birth_time: {value}"),
            Self::InvalidCalendarType(value) => write!(f, "invalid calendar_type: {value}"),
            Self::EngineReturnedError(value) => write!(f, "ziwei engine returned error: {value}"),
        }
    }
}

impl std::error::Error for ZiweiEngineError {}

pub fn generate_ziwei_chart(
    birth_date: &str,
    birth_time: &str,
) -> Result<ZiweiEngineResponse, ZiweiEngineError> {
    generate_ziwei_reading(ZiweiEngineRequest {
        reading_type: "ziwei",
        birth_date: Some(birth_date),
        birth_time: Some(birth_time),
        calendar_type: Some("solar"),
        is_lunar_leap_month: false,
        gender: None,
        target_year: None,
    })
}

pub fn generate_ziwei_reading(
    request: ZiweiEngineRequest<'_>,
) -> Result<ZiweiEngineResponse, ZiweiEngineError> {
    if !is_valid_reading_type(request.reading_type) {
        return Err(ZiweiEngineError::UnknownReadingType(
            request.reading_type.to_string(),
        ));
    }

    let birth_date = request
        .birth_date
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(ZiweiEngineError::MissingBirthDate)?;
    let birth_time = request
        .birth_time
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(ZiweiEngineError::MissingBirthTime)?;

    let parsed_date = parse_birth_date(birth_date)?;
    let (hour, minute) = parse_birth_time(birth_time)?;
    let calendar_type = request.calendar_type.unwrap_or("solar");
    if !matches!(
        calendar_type.trim().to_ascii_lowercase().as_str(),
        "" | "solar" | "lunar"
    ) {
        return Err(ZiweiEngineError::InvalidCalendarType(
            calendar_type.to_string(),
        ));
    }

    let normalized = normalize_birth_date(
        parsed_date.0,
        parsed_date.1,
        parsed_date.2,
        Some(calendar_type),
        request.is_lunar_leap_month,
    )
    .ok_or_else(|| ZiweiEngineError::InvalidBirthDate(birth_date.to_string()))?;

    let chart = calculate_chart(ChartInput {
        birth: normalized,
        birth_time: format!("{hour:02}:{minute:02}"),
        hour,
        minute,
        target_year: request.target_year,
        gender: request.gender.map(str::to_string),
    })
    .map_err(|error| ZiweiEngineError::EngineReturnedError(error.to_string()))?;
    let engine_version = ZIWEI_ENGINE_VERSION.to_string();
    let result_json = chart_to_json_with_version(&chart, &engine_version);

    Ok(ZiweiEngineResponse {
        chart,
        result_json,
        engine_version,
    })
}

pub(crate) fn parse_birth_date(value: &str) -> Result<(i32, u32, u32), ZiweiEngineError> {
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != 3 {
        return Err(ZiweiEngineError::InvalidBirthDate(value.to_string()));
    }
    let year = parts[0]
        .parse::<i32>()
        .map_err(|_| ZiweiEngineError::InvalidBirthDate(value.to_string()))?;
    let month = parts[1]
        .parse::<u32>()
        .map_err(|_| ZiweiEngineError::InvalidBirthDate(value.to_string()))?;
    let day = parts[2]
        .parse::<u32>()
        .map_err(|_| ZiweiEngineError::InvalidBirthDate(value.to_string()))?;
    Ok((year, month, day))
}

pub(crate) fn parse_birth_time(value: &str) -> Result<(u32, u32), ZiweiEngineError> {
    let mut parts = value.split(':');
    let hour = parts
        .next()
        .ok_or_else(|| ZiweiEngineError::InvalidBirthTime(value.to_string()))?
        .parse::<u32>()
        .map_err(|_| ZiweiEngineError::InvalidBirthTime(value.to_string()))?;
    let minute = match parts.next() {
        Some(raw) => raw
            .parse::<u32>()
            .map_err(|_| ZiweiEngineError::InvalidBirthTime(value.to_string()))?,
        None => 0,
    };

    if parts.next().is_some() || hour > 23 || minute > 59 {
        return Err(ZiweiEngineError::InvalidBirthTime(value.to_string()));
    }

    Ok((hour, minute))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_time_accepts_hour_or_hour_minute() {
        assert_eq!(parse_birth_time("9").unwrap(), (9, 0));
        assert_eq!(parse_birth_time("09:30").unwrap(), (9, 30));
    }

    #[test]
    fn parse_time_rejects_invalid_values() {
        assert!(parse_birth_time("24:00").is_err());
        assert!(parse_birth_time("12:60").is_err());
        assert!(parse_birth_time("12:00:00").is_err());
    }
}
