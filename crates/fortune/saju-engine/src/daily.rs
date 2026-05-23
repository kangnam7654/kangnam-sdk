use super::elements::{self, ElementRelation};
use super::pillars;
use super::types::{Element, FourPillars, Pillar, Stem};
use chrono::Utc;

pub struct DailyScores {
    pub overall: i32,
    pub love: i32,
    pub career: i32,
    pub health: i32,
}

/// Calculation-only daily fortune facts. This is the path used by
/// `SajuEngine::generate`; interpretation copy is intentionally absent.
pub struct DailyCalculation {
    pub date: String,
    pub today_pillar: Pillar,
    pub day_master: Stem,
    pub relation: ElementRelation,
    pub scores: DailyScores,
}

pub struct DailyCategoryScores {
    pub love: i32,
    pub career: i32,
    pub health: i32,
    pub wealth: i32,
    pub study: i32,
    pub travel: i32,
    pub relations: i32,
}

pub struct HourlyScore {
    pub hour_name: String,
    pub hour_range: String,
    pub score: i32,
}

pub struct DailyDetailCalculation {
    pub base: DailyCalculation,
    pub category_scores: DailyCategoryScores,
    pub hourly_scores: Vec<HourlyScore>,
    pub lucky_items: LuckyItems,
}

pub fn calculate_daily_calculation_for_date(
    user_pillars: &FourPillars,
    year: i32,
    month: u32,
    day: u32,
) -> DailyCalculation {
    let today_pillar = pillars::day_pillar(year, month, day);
    let day_master = user_pillars.day.stem;
    let relation = elements::relation(day_master.element(), today_pillar.stem.element());
    let base = relation.daily_score_base();
    let branch_relation = elements::relation(
        user_pillars.day.branch.element(),
        today_pillar.branch.element(),
    );
    let branch_adj = match branch_relation {
        ElementRelation::Generated => 5,
        ElementRelation::Same => 3,
        ElementRelation::Generates => -2,
        ElementRelation::Controls => 0,
        ElementRelation::Controlled => -5,
    };
    let overall = (base + branch_adj).clamp(30, 98);
    let love = category_score(overall, day_master, today_pillar.stem, 0);
    let career = category_score(overall, day_master, today_pillar.stem, 1);
    let health = category_score(overall, day_master, today_pillar.stem, 2);

    DailyCalculation {
        date: format!("{year:04}-{month:02}-{day:02}"),
        today_pillar,
        day_master,
        relation,
        scores: DailyScores {
            overall,
            love,
            career,
            health,
        },
    }
}

pub fn calculate_daily_calculation(user_pillars: &FourPillars) -> DailyCalculation {
    use chrono::Datelike;
    let kst = chrono::FixedOffset::east_opt(9 * 3600).unwrap();
    let today = Utc::now().with_timezone(&kst).date_naive();
    calculate_daily_calculation_for_date(user_pillars, today.year(), today.month(), today.day())
}

pub fn calculate_daily_detail_calculation_for_date(
    user_pillars: &FourPillars,
    year: i32,
    month: u32,
    day: u32,
) -> DailyDetailCalculation {
    let base = calculate_daily_calculation_for_date(user_pillars, year, month, day);
    let day_master = user_pillars.day.stem;
    let wealth = category_score(base.scores.overall, day_master, base.today_pillar.stem, 3);
    let study = category_score(base.scores.overall, day_master, base.today_pillar.stem, 4);
    let travel = category_score(base.scores.overall, day_master, base.today_pillar.stem, 5);
    let relations = category_score(base.scores.overall, day_master, base.today_pillar.stem, 6);

    DailyDetailCalculation {
        category_scores: DailyCategoryScores {
            love: base.scores.love,
            career: base.scores.career,
            health: base.scores.health,
            wealth,
            study,
            travel,
            relations,
        },
        hourly_scores: calculate_hourly_scores(day_master, &base.today_pillar),
        lucky_items: calculate_lucky_items(day_master, base.today_pillar.stem),
        base,
    }
}

pub fn calculate_daily_detail_calculation(user_pillars: &FourPillars) -> DailyDetailCalculation {
    use chrono::Datelike;
    let kst = chrono::FixedOffset::east_opt(9 * 3600).unwrap();
    let today = Utc::now().with_timezone(&kst).date_naive();
    calculate_daily_detail_calculation_for_date(
        user_pillars,
        today.year(),
        today.month(),
        today.day(),
    )
}

fn category_score(base: i32, day_master: Stem, today_stem: Stem, category: u8) -> i32 {
    // 간단한 결정론적 변동: 천간 인덱스 조합으로
    let seed = day_master.index() * 10 + today_stem.index() + category as usize;
    let variation = ((seed * 7 + 13) % 21) as i32 - 10; // -10 ~ +10
    (base + variation).clamp(30, 98)
}

/// 행운 아이템
pub struct LuckyItems {
    pub color: String,
    pub color_hex: String,
    pub number: i32,
    pub direction: String,
}

/// 12시진 시간대별 운세
fn calculate_hourly_scores(day_master: Stem, today_pillar: &Pillar) -> Vec<HourlyScore> {
    const HOURS: [(u32, &str, &str); 12] = [
        (0, "자시(子時)", "23:00-01:00"),
        (2, "축시(丑時)", "01:00-03:00"),
        (4, "인시(寅時)", "03:00-05:00"),
        (6, "묘시(卯時)", "05:00-07:00"),
        (8, "진시(辰時)", "07:00-09:00"),
        (10, "사시(巳時)", "09:00-11:00"),
        (12, "오시(午時)", "11:00-13:00"),
        (14, "미시(未時)", "13:00-15:00"),
        (16, "신시(申時)", "15:00-17:00"),
        (18, "유시(酉時)", "17:00-19:00"),
        (20, "술시(戌時)", "19:00-21:00"),
        (22, "해시(亥時)", "21:00-23:00"),
    ];

    HOURS
        .iter()
        .map(|&(hour, name, range)| {
            let hour_pillar = pillars::hour_pillar(today_pillar.stem, hour);
            let hour_elem = hour_pillar.stem.element();
            let day_elem = day_master.element();
            let rel = elements::relation(day_elem, hour_elem);
            let base_score = rel.daily_score_base();
            let branch_rel = elements::relation(day_master.element(), hour_pillar.branch.element());
            let adj = match branch_rel {
                ElementRelation::Generated => 3,
                ElementRelation::Same => 2,
                ElementRelation::Generates => -1,
                ElementRelation::Controls => 0,
                ElementRelation::Controlled => -3,
            };

            HourlyScore {
                hour_name: name.to_string(),
                hour_range: range.to_string(),
                score: (base_score + adj).clamp(30, 98),
            }
        })
        .collect()
}

/// 행운 아이템 계산 (오행 인성 기반, 결정론적)
fn calculate_lucky_items(day_master: Stem, today_stem: Stem) -> LuckyItems {
    // 인성(나를 생해주는 오행)이 행운의 기운
    let lucky_element = generating_element(day_master.element());

    let (color, color_hex) = match lucky_element {
        Element::Wood => ("초록", "#4ADE80"),
        Element::Fire => ("빨강", "#F87171"),
        Element::Earth => ("노랑", "#FBBF24"),
        Element::Metal => ("흰색", "#E5E7EB"),
        Element::Water => ("파랑", "#60A5FA"),
    };

    let direction = match lucky_element {
        Element::Wood => "동쪽",
        Element::Fire => "남쪽",
        Element::Earth => "중앙",
        Element::Metal => "서쪽",
        Element::Water => "북쪽",
    };

    // 숫자: 천간 인덱스 조합 기반 (1-9)
    let seed = day_master.index() * 10 + today_stem.index();
    let number = ((seed * 7 + 3) % 9 + 1) as i32;

    LuckyItems {
        color: color.to_string(),
        color_hex: color_hex.to_string(),
        number,
        direction: direction.to_string(),
    }
}

/// 나를 생해주는 오행 (인성)
fn generating_element(elem: Element) -> Element {
    match elem {
        Element::Wood => Element::Water,  // 수생목
        Element::Fire => Element::Wood,   // 목생화
        Element::Earth => Element::Fire,  // 화생토
        Element::Metal => Element::Earth, // 토생금
        Element::Water => Element::Metal, // 금생수
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{self as saju, types::*};

    fn test_pillars() -> FourPillars {
        saju::calculate_four_pillars(1990, 1, 15, 14)
    }

    #[test]
    fn daily_calculation_for_date_is_deterministic() {
        let pillars = test_pillars();
        let first = calculate_daily_calculation_for_date(&pillars, 2026, 3, 23);
        let second = calculate_daily_calculation_for_date(&pillars, 2026, 3, 23);

        assert_eq!(first.date, second.date);
        assert_eq!(
            first.today_pillar.to_string(),
            second.today_pillar.to_string()
        );
        assert_eq!(first.scores.overall, second.scores.overall);
        assert_eq!(first.scores.love, second.scores.love);
        assert_eq!(first.scores.career, second.scores.career);
        assert_eq!(first.scores.health, second.scores.health);
    }

    #[test]
    fn daily_detail_calculation_has_category_scores() {
        let detail = calculate_daily_detail_calculation_for_date(&test_pillars(), 2026, 3, 23);
        for score in [
            detail.category_scores.love,
            detail.category_scores.career,
            detail.category_scores.health,
            detail.category_scores.wealth,
            detail.category_scores.study,
            detail.category_scores.travel,
            detail.category_scores.relations,
        ] {
            assert!((30..=98).contains(&score), "score out of range: {score}");
        }
    }

    #[test]
    fn daily_detail_calculation_has_12_hourly_scores() {
        let detail = calculate_daily_detail_calculation_for_date(&test_pillars(), 2026, 3, 23);
        assert_eq!(detail.hourly_scores.len(), 12);
        for hour in &detail.hourly_scores {
            assert!(!hour.hour_name.is_empty());
            assert!(!hour.hour_range.is_empty());
            assert!((30..=98).contains(&hour.score));
        }
    }

    #[test]
    fn daily_detail_calculation_lucky_items_are_facts_only() {
        let detail = calculate_daily_detail_calculation_for_date(&test_pillars(), 2026, 3, 23);
        assert!(!detail.lucky_items.color.is_empty());
        assert!(detail.lucky_items.color_hex.starts_with('#'));
        assert!((1..=9).contains(&detail.lucky_items.number));
        assert!(!detail.lucky_items.direction.is_empty());
    }

    #[test]
    fn category_score_varies_with_day_master() {
        let p1 = saju::calculate_four_pillars(1990, 1, 15, 14);
        let p2 = saju::calculate_four_pillars(1991, 6, 20, 9);
        let p3 = saju::calculate_four_pillars(1985, 10, 5, 18);
        let p4 = saju::calculate_four_pillars(2000, 3, 1, 23);
        let scores: Vec<_> = [p1, p2, p3, p4]
            .iter()
            .map(|p| {
                calculate_daily_detail_calculation_for_date(p, 2026, 3, 23)
                    .category_scores
                    .study
            })
            .collect();
        let unique: std::collections::HashSet<_> = scores.iter().collect();
        assert!(
            unique.len() >= 2,
            "scores all identical across day_masters: {:?}",
            scores
        );
    }
}
