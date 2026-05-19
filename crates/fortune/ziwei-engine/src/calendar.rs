use chrono::NaiveDate;
use rs_klc::LunarSolarConverter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarType {
    Solar,
    Lunar,
}

impl CalendarType {
    pub fn parse(value: Option<&str>) -> Option<Self> {
        match value
            .unwrap_or("solar")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "" | "solar" => Some(Self::Solar),
            "lunar" => Some(Self::Lunar),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Solar => "solar",
            Self::Lunar => "lunar",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedBirthDate {
    pub original_year: i32,
    pub original_month: u32,
    pub original_day: u32,
    pub solar_year: i32,
    pub solar_month: u32,
    pub solar_day: u32,
    pub lunar_year: i32,
    pub lunar_month: u32,
    pub lunar_day: u32,
    pub calendar_type: CalendarType,
    pub is_lunar_leap_month: bool,
}

impl NormalizedBirthDate {
    pub fn original_date_string(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}",
            self.original_year, self.original_month, self.original_day
        )
    }

    pub fn solar_date_string(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}",
            self.solar_year, self.solar_month, self.solar_day
        )
    }

    pub fn lunar_date_string(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}",
            self.lunar_year, self.lunar_month, self.lunar_day
        )
    }

    pub fn was_lunar_converted(&self) -> bool {
        self.calendar_type == CalendarType::Lunar
    }
}

pub fn normalize_birth_date(
    year: i32,
    month: u32,
    day: u32,
    calendar_type: Option<&str>,
    is_lunar_leap_month: bool,
) -> Option<NormalizedBirthDate> {
    let calendar_type = CalendarType::parse(calendar_type)?;
    let mut converter = LunarSolarConverter::new();

    match calendar_type {
        CalendarType::Solar => {
            NaiveDate::from_ymd_opt(year, month, day)?;
            if year < 0 {
                return None;
            }
            if !converter.set_solar_date(year as u32, month, day) {
                return None;
            }

            Some(NormalizedBirthDate {
                original_year: year,
                original_month: month,
                original_day: day,
                solar_year: year,
                solar_month: month,
                solar_day: day,
                lunar_year: converter.lunar_year(),
                lunar_month: converter.lunar_month(),
                lunar_day: converter.lunar_day(),
                calendar_type,
                is_lunar_leap_month: converter.is_intercalation(),
            })
        }
        CalendarType::Lunar => {
            if !converter.set_lunar_date(year, month, day, is_lunar_leap_month) {
                return None;
            }
            let solar_year = i32::try_from(converter.solar_year()).ok()?;
            let solar_month = converter.solar_month();
            let solar_day = converter.solar_day();
            NaiveDate::from_ymd_opt(solar_year, solar_month, solar_day)?;

            Some(NormalizedBirthDate {
                original_year: year,
                original_month: month,
                original_day: day,
                solar_year,
                solar_month,
                solar_day,
                lunar_year: year,
                lunar_month: month,
                lunar_day: day,
                calendar_type,
                is_lunar_leap_month,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_solar_to_lunar_fields() {
        let normalized = normalize_birth_date(2022, 7, 10, Some("solar"), false).unwrap();

        assert_eq!(normalized.solar_date_string(), "2022-07-10");
        assert_eq!(normalized.lunar_date_string(), "2022-06-12");
        assert_eq!(normalized.calendar_type.as_str(), "solar");
    }

    #[test]
    fn converts_lunar_to_solar_fields() {
        let normalized = normalize_birth_date(2022, 6, 12, Some("lunar"), false).unwrap();

        assert_eq!(normalized.solar_date_string(), "2022-07-10");
        assert_eq!(normalized.lunar_date_string(), "2022-06-12");
        assert_eq!(normalized.calendar_type.as_str(), "lunar");
    }
}
