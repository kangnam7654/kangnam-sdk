use super::elements::{self, ElementRelation};
use super::solar_terms;
use super::types::{Branch, FourPillars, Polarity, Stem};
use chrono::Datelike;

/// 대운 계산값. 해석 문장을 포함하지 않아 엔진 JSON 출력 경로에서 사용한다.
pub struct DaeunCalculation {
    pub start_age: i32,
    pub end_age: i32,
    pub stem: String,
    pub branch: String,
    pub element: String,
    pub score: i32,
    pub is_current: bool,
}

pub fn calculate_daeun_calculation(
    user_pillars: &FourPillars,
    birth_year: i32,
    birth_month: u32,
    birth_day: u32,
    gender: &str,
) -> Vec<DaeunCalculation> {
    calculate_daeun_calculation_with_time(
        user_pillars,
        birth_year,
        birth_month,
        birth_day,
        12,
        0,
        gender,
    )
}

pub fn calculate_daeun_calculation_with_time(
    user_pillars: &FourPillars,
    birth_year: i32,
    birth_month: u32,
    birth_day: u32,
    birth_hour: u32,
    birth_minute: u32,
    gender: &str,
) -> Vec<DaeunCalculation> {
    let is_male = gender.eq_ignore_ascii_case("M") || gender.eq_ignore_ascii_case("male");
    let year_stem_is_yang = user_pillars.year.stem.polarity() == Polarity::Yang;

    // 순행: 양남 또는 음녀
    let forward = (is_male && year_stem_is_yang) || (!is_male && !year_stem_is_yang);

    let days_to_jieqi = solar_terms::adjacent_solar_term_days(
        birth_year,
        birth_month,
        birth_day,
        birth_hour,
        birth_minute,
        forward,
    )
    .unwrap_or_else(|| fallback_days_to_jieqi(birth_year, birth_month, birth_day, forward));

    // 대운 시작 나이: 날수 ÷ 3, 최소 1, 최대 9
    let start_age = ((days_to_jieqi + 2) / 3).clamp(1, 9);

    // 출생 월주 인덱스 (천간+지지 조합의 60갑자 순번)
    // 월주에서 순행/역행으로 10년마다 한 간지씩 이동
    let month_stem_idx = user_pillars.month.stem.index();
    let month_branch_idx = user_pillars.month.branch.index();

    let kst = chrono::FixedOffset::east_opt(9 * 3600).unwrap();
    let current_year = chrono::Utc::now().with_timezone(&kst).year();
    let current_age = current_year - birth_year;

    // 8개 대운 생성 (출생부터 80세+ 커버)
    (0..8_i32)
        .map(|i| {
            let period_start = start_age + i * 10;
            let period_end = period_start + 9;

            // 순행이면 +i, 역행이면 -i 간지 이동
            let stem_idx = if forward {
                (month_stem_idx + 1 + i as usize) % 10
            } else {
                (month_stem_idx + 10 - 1 - i as usize % 10) % 10
            };
            let branch_idx = if forward {
                (month_branch_idx + 1 + i as usize) % 12
            } else {
                (month_branch_idx + 12 - 1 - i as usize % 12) % 12
            };

            let stem = Stem::from_index(stem_idx);
            let branch = Branch::from_index(branch_idx);

            // 점수: 대운 천간 오행 vs 일간 오행 관계
            let day_master = user_pillars.day.stem;
            let stem_relation = elements::relation(day_master.element(), stem.element());
            let branch_relation = elements::relation(day_master.element(), branch.element());
            let score = daeun_score(stem_relation, branch_relation);

            let is_current = (period_start..=period_end).contains(&current_age);

            DaeunCalculation {
                start_age: period_start,
                end_age: period_end,
                stem: stem.korean().to_string(),
                branch: branch.korean().to_string(),
                element: stem.element().korean().to_string(),
                score,
                is_current,
            }
        })
        .collect()
}

fn fallback_days_to_jieqi(birth_year: i32, birth_month: u32, birth_day: u32, forward: bool) -> i32 {
    let jieqi_day: u32 = 5;
    if forward {
        let next_jieqi_day =
            days_in_month(birth_year, birth_month + 1) as i32 - birth_day as i32 + jieqi_day as i32;
        next_jieqi_day.max(1)
    } else {
        (birth_day as i32 - jieqi_day as i32).max(1)
    }
}

/// 월의 일수 (정밀 절기 테이블 범위 밖 fallback용)
fn days_in_month(year: i32, month: u32) -> u32 {
    let m = ((month - 1) % 12) + 1;
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// 대운 점수 계산
/// 천간 관계 70%, 지지 관계 30% 가중치
fn daeun_score(stem_rel: ElementRelation, branch_rel: ElementRelation) -> i32 {
    let stem_base = match stem_rel {
        ElementRelation::Generated => 85,  // 인성: 지원·지식·귀인의 운
        ElementRelation::Same => 72,       // 비화: 독립·비견의 운
        ElementRelation::Controls => 68,   // 재성: 재물·성취의 운
        ElementRelation::Generates => 60,  // 설기: 소모·표현의 운
        ElementRelation::Controlled => 55, // 관성: 규율·도전의 운
    };

    let branch_adj = match branch_rel {
        ElementRelation::Generated => 6,
        ElementRelation::Same => 3,
        ElementRelation::Controls => 2,
        ElementRelation::Generates => -3,
        ElementRelation::Controlled => -6,
    };

    (stem_base + branch_adj).clamp(30, 98)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as saju;

    fn test_pillars_male() -> (FourPillars, i32, u32, u32) {
        // 1990-05-15 (양간 연주 → 갑오년 = 양남 순행)
        let pillars = saju::calculate_four_pillars(1990, 5, 15, 12);
        (pillars, 1990, 5, 15)
    }

    fn test_pillars_female() -> (FourPillars, i32, u32, u32) {
        // 1995-03-20
        let pillars = saju::calculate_four_pillars(1995, 3, 20, 10);
        (pillars, 1995, 3, 20)
    }

    #[test]
    fn test_daeun_returns_8_periods() {
        let (pillars, by, bm, bd) = test_pillars_male();
        let periods = calculate_daeun_calculation(&pillars, by, bm, bd, "M");
        assert_eq!(periods.len(), 8, "대운은 8개여야 합니다");
    }

    #[test]
    fn test_daeun_periods_are_10_year_intervals() {
        let (pillars, by, bm, bd) = test_pillars_male();
        let periods = calculate_daeun_calculation(&pillars, by, bm, bd, "M");
        for (i, p) in periods.iter().enumerate() {
            assert_eq!(
                p.end_age - p.start_age,
                9,
                "period {} should span 10 years (end-start=9), got {}-{}",
                i,
                p.start_age,
                p.end_age
            );
        }
    }

    #[test]
    fn test_daeun_periods_are_contiguous() {
        let (pillars, by, bm, bd) = test_pillars_male();
        let periods = calculate_daeun_calculation(&pillars, by, bm, bd, "M");
        for i in 1..periods.len() {
            assert_eq!(
                periods[i].start_age,
                periods[i - 1].end_age + 1,
                "periods should be contiguous: period {} starts at {} but period {} ends at {}",
                i,
                periods[i].start_age,
                i - 1,
                periods[i - 1].end_age
            );
        }
    }

    #[test]
    fn test_daeun_scores_in_range() {
        let (pillars, by, bm, bd) = test_pillars_male();
        let periods = calculate_daeun_calculation(&pillars, by, bm, bd, "M");
        for (i, p) in periods.iter().enumerate() {
            assert!(
                p.score >= 30 && p.score <= 98,
                "period {} score {} out of range [30, 98]",
                i,
                p.score
            );
        }
    }

    #[test]
    fn test_daeun_non_empty_fields() {
        let (pillars, by, bm, bd) = test_pillars_male();
        let periods = calculate_daeun_calculation(&pillars, by, bm, bd, "M");
        for (i, p) in periods.iter().enumerate() {
            assert!(!p.stem.is_empty(), "period {} stem is empty", i);
            assert!(!p.branch.is_empty(), "period {} branch is empty", i);
            assert!(!p.element.is_empty(), "period {} element is empty", i);
        }
    }

    #[test]
    fn test_daeun_at_most_one_current() {
        let (pillars, by, bm, bd) = test_pillars_male();
        let periods = calculate_daeun_calculation(&pillars, by, bm, bd, "M");
        let current_count = periods.iter().filter(|p| p.is_current).count();
        assert!(
            current_count <= 1,
            "at most one period can be current, got {}",
            current_count
        );
    }

    #[test]
    fn test_daeun_female_different_from_male() {
        let (m_pillars, by, bm, bd) = test_pillars_male();
        let m_periods = calculate_daeun_calculation(&m_pillars, by, bm, bd, "M");

        let (f_pillars, fby, fbm, fbd) = test_pillars_female();
        let f_periods = calculate_daeun_calculation(&f_pillars, fby, fbm, fbd, "F");

        // 다른 사람이므로 첫 번째 대운 천간이 다를 수 있음 (최소한 컴파일·실행 확인)
        assert_eq!(m_periods.len(), 8);
        assert_eq!(f_periods.len(), 8);
    }

    #[test]
    fn same_chart_gender_changes_daeun_direction() {
        let (pillars, by, bm, bd) = test_pillars_male();
        let male = calculate_daeun_calculation(&pillars, by, bm, bd, "male");
        let female = calculate_daeun_calculation(&pillars, by, bm, bd, "female");

        assert_ne!(male[0].stem, female[0].stem);
        assert_ne!(male[0].branch, female[0].branch);
    }

    #[test]
    fn start_age_uses_precise_solar_term_distance() {
        let (pillars, by, bm, bd) = test_pillars_male();
        let days = crate::solar_terms::adjacent_solar_term_days(by, bm, bd, 12, 0, true).unwrap();
        let expected = ((days + 2) / 3).clamp(1, 9);
        let periods = calculate_daeun_calculation_with_time(&pillars, by, bm, bd, 12, 0, "male");

        assert_eq!(periods[0].start_age, expected);
    }

    #[test]
    fn test_daeun_deterministic() {
        let (pillars, by, bm, bd) = test_pillars_male();
        let first = calculate_daeun_calculation(&pillars, by, bm, bd, "M");
        let second = calculate_daeun_calculation(&pillars, by, bm, bd, "M");
        for (a, b) in first.iter().zip(second.iter()) {
            assert_eq!(a.start_age, b.start_age);
            assert_eq!(a.score, b.score);
            assert_eq!(a.stem, b.stem);
            assert_eq!(a.branch, b.branch);
        }
    }

    #[test]
    fn test_start_age_valid_range() {
        let (pillars, by, bm, bd) = test_pillars_male();
        let periods = calculate_daeun_calculation(&pillars, by, bm, bd, "M");
        assert!(
            periods[0].start_age >= 1 && periods[0].start_age <= 9,
            "start_age {} should be in [1, 9]",
            periods[0].start_age
        );
    }
}
