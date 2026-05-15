use chrono::NaiveDate;
use rs_klc::LunarSolarConverter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BirthCalendar {
    Solar,
    Lunar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalizedBirthDate {
    pub original_year: i32,
    pub original_month: u32,
    pub original_day: u32,
    pub solar_year: i32,
    pub solar_month: u32,
    pub solar_day: u32,
    pub calendar: BirthCalendar,
    pub is_lunar_leap_month: bool,
}

impl NormalizedBirthDate {
    pub fn solar_date_string(self) -> String {
        format!(
            "{:04}-{:02}-{:02}",
            self.solar_year, self.solar_month, self.solar_day
        )
    }

    pub fn calendar_type(self) -> &'static str {
        match self.calendar {
            BirthCalendar::Solar => "solar",
            BirthCalendar::Lunar => "lunar",
        }
    }

    pub fn was_converted(self) -> bool {
        self.calendar == BirthCalendar::Lunar
    }
}

pub fn normalize_birth_date(
    year: i32,
    month: u32,
    day: u32,
    calendar_type: Option<&str>,
    is_lunar_leap_month: bool,
) -> Option<NormalizedBirthDate> {
    let calendar = match calendar_type
        .unwrap_or("solar")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "solar" => BirthCalendar::Solar,
        "lunar" => BirthCalendar::Lunar,
        _ => return None,
    };

    match calendar {
        BirthCalendar::Solar => {
            NaiveDate::from_ymd_opt(year, month, day)?;
            Some(NormalizedBirthDate {
                original_year: year,
                original_month: month,
                original_day: day,
                solar_year: year,
                solar_month: month,
                solar_day: day,
                calendar,
                is_lunar_leap_month: false,
            })
        }
        BirthCalendar::Lunar => {
            let mut converter = LunarSolarConverter::new();
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
                calendar,
                is_lunar_leap_month,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_solar_birth_date_without_conversion() {
        let normalized = normalize_birth_date(1990, 5, 15, Some("solar"), false).unwrap();

        assert_eq!(normalized.solar_date_string(), "1990-05-15");
        assert_eq!(normalized.calendar_type(), "solar");
        assert!(!normalized.was_converted());
    }

    #[test]
    fn converts_lunar_birth_date_to_solar_date() {
        let normalized = normalize_birth_date(2022, 6, 12, Some("lunar"), false).unwrap();

        assert_eq!(normalized.solar_date_string(), "2022-07-10");
        assert_eq!(normalized.calendar_type(), "lunar");
        assert!(normalized.was_converted());
    }

    #[test]
    fn rejects_invalid_lunar_date() {
        assert!(normalize_birth_date(2022, 13, 1, Some("lunar"), false).is_none());
    }
}
