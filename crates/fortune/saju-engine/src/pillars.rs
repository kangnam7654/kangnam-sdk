use super::tables;
use super::types::{Branch, FourPillars, Pillar, Stem};
use chrono::{Datelike, NaiveDate};

/// 기준일: 1900-01-31 = 갑자일 (stem=0, branch=0)
const BASE_DATE: (i32, u32, u32) = (1900, 1, 31);

/// 년주 계산
/// 입춘(약 2/4) 이전이면 전년도로 취급
pub fn year_pillar(year: i32, month: u32, day: u32) -> Pillar {
    let effective_year = if month < 2 || (month == 2 && day < 4) {
        year - 1
    } else {
        year
    };
    let stem_idx = ((effective_year - 4) % 10 + 10) as usize % 10;
    let branch_idx = ((effective_year - 4) % 12 + 12) as usize % 12;
    Pillar::new(Stem::from_index(stem_idx), Branch::from_index(branch_idx))
}

/// 월주 계산
pub fn month_pillar(year_stem: Stem, month: u32, day: u32) -> Pillar {
    let month_idx = tables::solar_month_index(month, day);
    let stem = tables::month_stem(year_stem, month_idx);
    let branch = tables::MONTH_BRANCHES[month_idx];
    Pillar::new(stem, branch)
}

/// 일주 계산
/// 1900-01-31(갑자)부터의 일수 차이로 계산
pub fn day_pillar(year: i32, month: u32, day: u32) -> Pillar {
    let base = NaiveDate::from_ymd_opt(BASE_DATE.0, BASE_DATE.1, BASE_DATE.2)
        .expect("Invalid BASE_DATE constant");
    let target = NaiveDate::from_ymd_opt(year, month, day).unwrap_or(base);
    let diff = (target - base).num_days();
    let offset = ((diff % 60) + 60) as usize % 60;
    let stem_idx = offset % 10;
    let branch_idx = offset % 12;
    Pillar::new(Stem::from_index(stem_idx), Branch::from_index(branch_idx))
}

/// 시주 계산
pub fn hour_pillar(day_stem: Stem, hour: u32) -> Pillar {
    let hour_idx = tables::hour_branch_index(hour);
    let stem = tables::hour_stem(day_stem, hour_idx);
    let branch = tables::HOUR_BRANCHES[hour_idx];
    Pillar::new(stem, branch)
}

/// 사주팔자 전체 계산
pub fn calculate_four_pillars(year: i32, month: u32, day: u32, hour: u32) -> FourPillars {
    let year_p = year_pillar(year, month, day);
    let month_p = month_pillar(year_p.stem, month, day);
    let day_p = day_pillar(year, month, day);
    let hour_p = hour_pillar(day_p.stem, hour);
    FourPillars {
        year: year_p,
        month: month_p,
        day: day_p,
        hour: hour_p,
    }
}

/// 오늘의 일주 계산 (KST 기준)
pub fn today_pillar() -> Pillar {
    let kst = chrono::FixedOffset::east_opt(9 * 3600).unwrap();
    let today = chrono::Utc::now().with_timezone(&kst).date_naive();
    day_pillar(today.year(), today.month(), today.day())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Branch, Stem};

    #[test]
    fn test_year_pillar_2024() {
        // 2024년은 갑진(甲辰)년
        let p = year_pillar(2024, 3, 15);
        assert_eq!(p.stem, Stem::Gap);
        assert_eq!(p.branch, Branch::Jin);
    }

    #[test]
    fn test_year_pillar_before_ipchun() {
        // 2024년 1월 15일 → 아직 입춘 전이므로 2023년(계묘) 기준
        let p = year_pillar(2024, 1, 15);
        assert_eq!(p.stem, Stem::Gye);
        assert_eq!(p.branch, Branch::Myo);
    }

    #[test]
    fn test_day_pillar_known_date() {
        // 1900-01-31 = 갑자일
        let p = day_pillar(1900, 1, 31);
        assert_eq!(p.stem, Stem::Gap);
        assert_eq!(p.branch, Branch::Ja);
    }

    #[test]
    fn test_day_pillar_60_days_later() {
        // 60일 후 = 다시 갑자
        let p = day_pillar(1900, 4, 1);
        assert_eq!(p.stem, Stem::Gap);
        assert_eq!(p.branch, Branch::Ja);
    }

    #[test]
    fn test_four_pillars_1990_05_15_14() {
        // 1990년 5월 15일 14시
        let fp = calculate_four_pillars(1990, 5, 15, 14);
        // 1990 = 경오(庚午)년
        assert_eq!(fp.year.stem, Stem::Gyeong);
        assert_eq!(fp.year.branch, Branch::O);
    }

    #[test]
    fn test_hour_pillar() {
        // 갑일 자시(23-01) = 갑자시
        let p = hour_pillar(Stem::Gap, 0);
        assert_eq!(p.stem, Stem::Gap);
        assert_eq!(p.branch, Branch::Ja);

        // 갑일 축시(01-03) = 을축시
        let p = hour_pillar(Stem::Gap, 2);
        assert_eq!(p.stem, Stem::Eul);
        assert_eq!(p.branch, Branch::Chuk);
    }
}
