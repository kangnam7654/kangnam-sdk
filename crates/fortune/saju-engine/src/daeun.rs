#![allow(dead_code)]

use super::elements::{self, ElementRelation};
use super::solar_terms;
use super::types::{Branch, FourPillars, Pillar, Polarity, Stem};
use chrono::Datelike;

/// 대운 한 주기 (10년)
struct DaeunPeriod {
    pub start_age: i32,
    pub end_age: i32,
    pub stem: String,        // 천간 한글
    pub branch: String,      // 지지 한글
    pub element: String,     // 오행
    pub score: i32,          // 0-100
    pub description: String, // 한국어 설명
    pub is_current: bool,    // 현재 대운 여부
}

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

/// 대운(10년 주기) 계산
///
/// 역행(逆行)/순행(順行) 결정:
///   - 양남(陽男) 또는 음녀(陰女) → 순행(forward): 출생 월주에서 앞으로
///   - 음남(陰男) 또는 양녀(陽女) → 역행(backward): 출생 월주에서 뒤로
///
/// 대운 시작 나이(起大運):
///   - 순행이면 출생 후 다음 절기까지의 일수, 역행이면 이전 절기부터 지난 일수
///   - 일수 ÷ 3을 시작 나이로 환산하고 최소 1세로 보정
fn calculate_daeun(
    user_pillars: &FourPillars,
    birth_year: i32,
    birth_month: u32,
    birth_day: u32,
    gender: &str, // "M" or "F"
) -> Vec<DaeunPeriod> {
    calculate_daeun_with_time(
        user_pillars,
        birth_year,
        birth_month,
        birth_day,
        12,
        0,
        gender,
    )
}

fn calculate_daeun_with_time(
    user_pillars: &FourPillars,
    birth_year: i32,
    birth_month: u32,
    birth_day: u32,
    birth_hour: u32,
    birth_minute: u32,
    gender: &str, // "M" or "F"
) -> Vec<DaeunPeriod> {
    calculate_daeun_calculation_with_time(
        user_pillars,
        birth_year,
        birth_month,
        birth_day,
        birth_hour,
        birth_minute,
        gender,
    )
    .into_iter()
    .enumerate()
    .map(|(index, period)| {
        let stem = stem_from_korean(&period.stem).unwrap_or(user_pillars.month.stem);
        let branch = branch_from_korean(&period.branch).unwrap_or(user_pillars.month.branch);
        let pillar = Pillar::new(stem, branch);
        let stem_relation = elements::relation(user_pillars.day.stem.element(), stem.element());
        let branch_relation = elements::relation(user_pillars.day.stem.element(), branch.element());
        let description = daeun_description(stem_relation, branch_relation, &pillar, index as i32);
        DaeunPeriod {
            start_age: period.start_age,
            end_age: period.end_age,
            stem: period.stem,
            branch: period.branch,
            element: period.element,
            score: period.score,
            description,
            is_current: period.is_current,
        }
    })
    .collect()
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

fn stem_from_korean(value: &str) -> Option<Stem> {
    match value {
        "갑" => Some(Stem::Gap),
        "을" => Some(Stem::Eul),
        "병" => Some(Stem::Byeong),
        "정" => Some(Stem::Jeong),
        "무" => Some(Stem::Mu),
        "기" => Some(Stem::Gi),
        "경" => Some(Stem::Gyeong),
        "신" => Some(Stem::Sin),
        "임" => Some(Stem::Im),
        "계" => Some(Stem::Gye),
        _ => None,
    }
}

fn branch_from_korean(value: &str) -> Option<Branch> {
    match value {
        "자" => Some(Branch::Ja),
        "축" => Some(Branch::Chuk),
        "인" => Some(Branch::In),
        "묘" => Some(Branch::Myo),
        "진" => Some(Branch::Jin),
        "사" => Some(Branch::Sa),
        "오" => Some(Branch::O),
        "미" => Some(Branch::Mi),
        "신" => Some(Branch::Sin),
        "유" => Some(Branch::Yu),
        "술" => Some(Branch::Sul),
        "해" => Some(Branch::Hae),
        _ => None,
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

/// 대운 한국어 설명 생성
fn daeun_description(
    stem_rel: ElementRelation,
    branch_rel: ElementRelation,
    pillar: &Pillar,
    period_idx: i32,
) -> String {
    let stem_desc = match stem_rel {
        ElementRelation::Generated => {
            "주변의 지지와 귀인의 도움이 이어지는 시기입니다. 배움과 성장이 풍성하며 신뢰받는 위치에 서게 됩니다."
        }
        ElementRelation::Same => {
            "자신감과 독립심이 강해지는 시기입니다. 스스로의 힘으로 길을 개척하고 동료와 함께 성장합니다."
        }
        ElementRelation::Controls => {
            "재물운과 현실적 성취가 두드러지는 시기입니다. 노력한 만큼 결실을 거두며 경제적 안정을 다집니다."
        }
        ElementRelation::Generates => {
            "창의적 표현과 아이디어가 넘치는 시기입니다. 에너지 소모에 주의하면서 새로운 분야를 개척합니다."
        }
        ElementRelation::Controlled => {
            "외부 규율과 도전이 강해지는 시기입니다. 인내와 겸손으로 압박을 이겨내면 단단한 성장이 따라옵니다."
        }
    };

    let branch_add = match branch_rel {
        ElementRelation::Generated => {
            " 대지(地支)에서도 인성의 기운이 더해져 한층 안정적인 기반이 만들어집니다."
        }
        ElementRelation::Same => {
            " 지지에서 비화의 힘이 뒷받침되어 뜻이 맞는 사람들과의 협력이 빛을 발합니다."
        }
        ElementRelation::Controls => " 지지의 재성 기운이 현실적 이익을 강화합니다.",
        ElementRelation::Generates => " 지지에서 설기가 더해지니 건강과 에너지 관리에 신경 쓰세요.",
        ElementRelation::Controlled => " 지지의 관성이 더해져 규율과 책임이 한층 강조됩니다.",
    };

    let life_phase = match period_idx {
        0 => " 인생의 초년기로 기초를 다지는 중요한 시기입니다.",
        1 => " 청년기의 도약과 첫 번째 사회적 시험이 시작됩니다.",
        2 => " 사회적 역량을 발휘하고 커리어의 방향이 결정되는 시기입니다.",
        3 => " 인생의 정점을 향해 본격적으로 나아가는 중년기입니다.",
        4 => " 경험과 지혜가 쌓이며 안정을 추구하는 시기입니다.",
        5 => " 성숙한 판단력으로 주변을 이끄는 원로의 시기입니다.",
        _ => " 삶을 정리하고 다음 세대에 지혜를 물려주는 시기입니다.",
    };

    format!(
        "{}{}{} ({}{}운)",
        stem_desc,
        branch_add,
        life_phase,
        pillar.stem.korean(),
        pillar.branch.korean()
    )
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
        let periods = calculate_daeun(&pillars, by, bm, bd, "M");
        assert_eq!(periods.len(), 8, "대운은 8개여야 합니다");
    }

    #[test]
    fn test_daeun_periods_are_10_year_intervals() {
        let (pillars, by, bm, bd) = test_pillars_male();
        let periods = calculate_daeun(&pillars, by, bm, bd, "M");
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
        let periods = calculate_daeun(&pillars, by, bm, bd, "M");
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
        let periods = calculate_daeun(&pillars, by, bm, bd, "M");
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
        let periods = calculate_daeun(&pillars, by, bm, bd, "M");
        for (i, p) in periods.iter().enumerate() {
            assert!(!p.stem.is_empty(), "period {} stem is empty", i);
            assert!(!p.branch.is_empty(), "period {} branch is empty", i);
            assert!(!p.element.is_empty(), "period {} element is empty", i);
            assert!(
                !p.description.is_empty(),
                "period {} description is empty",
                i
            );
        }
    }

    #[test]
    fn test_daeun_at_most_one_current() {
        let (pillars, by, bm, bd) = test_pillars_male();
        let periods = calculate_daeun(&pillars, by, bm, bd, "M");
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
        let m_periods = calculate_daeun(&m_pillars, by, bm, bd, "M");

        let (f_pillars, fby, fbm, fbd) = test_pillars_female();
        let f_periods = calculate_daeun(&f_pillars, fby, fbm, fbd, "F");

        // 다른 사람이므로 첫 번째 대운 천간이 다를 수 있음 (최소한 컴파일·실행 확인)
        assert_eq!(m_periods.len(), 8);
        assert_eq!(f_periods.len(), 8);
    }

    #[test]
    fn same_chart_gender_changes_daeun_direction() {
        let (pillars, by, bm, bd) = test_pillars_male();
        let male = calculate_daeun(&pillars, by, bm, bd, "male");
        let female = calculate_daeun(&pillars, by, bm, bd, "female");

        assert_ne!(male[0].stem, female[0].stem);
        assert_ne!(male[0].branch, female[0].branch);
    }

    #[test]
    fn start_age_uses_precise_solar_term_distance() {
        let (pillars, by, bm, bd) = test_pillars_male();
        let days = crate::solar_terms::adjacent_solar_term_days(by, bm, bd, 12, 0, true).unwrap();
        let expected = ((days + 2) / 3).clamp(1, 9);
        let periods = calculate_daeun_with_time(&pillars, by, bm, bd, 12, 0, "male");

        assert_eq!(periods[0].start_age, expected);
    }

    #[test]
    fn test_daeun_deterministic() {
        let (pillars, by, bm, bd) = test_pillars_male();
        let first = calculate_daeun(&pillars, by, bm, bd, "M");
        let second = calculate_daeun(&pillars, by, bm, bd, "M");
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
        let periods = calculate_daeun(&pillars, by, bm, bd, "M");
        assert!(
            periods[0].start_age >= 1 && periods[0].start_age <= 9,
            "start_age {} should be in [1, 9]",
            periods[0].start_age
        );
    }
}
