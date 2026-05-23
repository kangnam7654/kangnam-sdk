use super::elements::{self, ElementRelation};
use super::pillars;
use super::types::{FourPillars, Stem};

/// 월운 단일 달 계산 결과
pub struct MonthlyCalculation {
    pub month: u32,
    pub score: i32,
    pub grade: String,
    pub categories: MonthlyCategories,
}

pub struct MonthlyCategories {
    pub overall: i32,
    pub love: i32,
    pub career: i32,
    pub health: i32,
    pub wealth: i32,
}

/// 연간 12개월 월운 계산
///
/// 각 월의 월주 천간(월간)과 사용자 일간(Day Master)의 오행 관계를 기준으로
/// 카테고리별 점수를 생성한다.
pub fn calculate_monthly_calculation(
    user_pillars: &FourPillars,
    year: i32,
) -> Vec<MonthlyCalculation> {
    let day_master = user_pillars.day.stem;

    (1u32..=12)
        .map(|month| {
            // 월주: 해당 월 1일 기준으로 계산
            // 입춘(2/4) 이전 1월은 전년도 기준이므로 year_pillar 도 같이 계산
            let year_p = pillars::year_pillar(year, month, 1);
            let month_p = pillars::month_pillar(year_p.stem, month, 1);

            // 월간(천간) vs 일간 오행 관계
            let stem_relation = elements::relation(day_master.element(), month_p.stem.element());
            let base = stem_relation.monthly_score_base();

            // 지지 관계로 미세 조정
            let branch_relation =
                elements::relation(user_pillars.day.branch.element(), month_p.branch.element());
            let branch_adj = branch_adjustment(branch_relation);

            let overall = (base + branch_adj).clamp(30, 98);

            // 카테고리별 점수
            let love = category_score(overall, day_master, month_p.stem, 0, stem_relation);
            let career = category_score(overall, day_master, month_p.stem, 1, stem_relation);
            let health = category_score(overall, day_master, month_p.stem, 2, stem_relation);
            let wealth = category_score(overall, day_master, month_p.stem, 3, stem_relation);

            let grade = score_to_grade(overall).to_string();

            MonthlyCalculation {
                month,
                score: overall,
                grade,
                categories: MonthlyCategories {
                    overall,
                    love,
                    career,
                    health,
                    wealth,
                },
            }
        })
        .collect()
}

/// 월운용 기본 점수 — 일운보다 더 넓은 범위 변동
fn branch_adjustment(relation: ElementRelation) -> i32 {
    match relation {
        ElementRelation::Generated => 8,
        ElementRelation::Same => 4,
        ElementRelation::Generates => -3,
        ElementRelation::Controls => 2,
        ElementRelation::Controlled => -8,
    }
}

/// 카테고리별 점수 계산
/// category: 0=love, 1=career, 2=health, 3=wealth
fn category_score(
    base: i32,
    day_master: Stem,
    month_stem: Stem,
    category: u8,
    relation: ElementRelation,
) -> i32 {
    // 십신 기반 가중치: 카테고리마다 잘 맞는 오행 관계가 다름
    let relation_bonus: i32 = match (category, relation) {
        // 연애(재성 관련): 내가 극하는 오행(Controls)이 재성 → 연애운 상승
        (0, ElementRelation::Controls) => 12,
        (0, ElementRelation::Generated) => 8,
        (0, ElementRelation::Controlled) => -10,
        // 직장(관성 관련): 나를 극하는 오행(Controlled)이 관성 → 직장 기회
        (1, ElementRelation::Controlled) => 15,
        (1, ElementRelation::Generated) => 10,
        (1, ElementRelation::Generates) => -8,
        // 건강(인성 관련): 나를 생해주는 오행(Generated)이 인성 → 건강 상승
        (2, ElementRelation::Generated) => 15,
        (2, ElementRelation::Same) => 8,
        (2, ElementRelation::Controlled) => -12,
        // 재물(재성 관련): 내가 극하는 오행(Controls) → 재물운
        (3, ElementRelation::Controls) => 15,
        (3, ElementRelation::Generates) => -5,
        (3, ElementRelation::Controlled) => -8,
        _ => 0,
    };

    // 결정론적 개인 변동 (천간 인덱스 기반, -8 ~ +8)
    let seed = day_master.index() * 10 + month_stem.index() + category as usize;
    let personal = ((seed * 7 + 13) % 17) as i32 - 8;

    (base + relation_bonus + personal).clamp(30, 98)
}

/// 점수 → 등급
pub fn score_to_grade(score: i32) -> &'static str {
    match score {
        80..=i32::MAX => "great",
        60..=79 => "good",
        40..=59 => "normal",
        _ => "caution",
    }
}

impl ElementRelation {
    /// 월운용 기본 점수 — daily_score_base보다 중간값이 더 낮음 (월 단위는 변동폭이 큼)
    pub fn monthly_score_base(self) -> i32 {
        match self {
            ElementRelation::Generated => 82,  // 인성: 도움 받는 달
            ElementRelation::Same => 72,       // 비화: 무난한 달
            ElementRelation::Controls => 68,   // 재성: 재물은 있으나 노력 필요
            ElementRelation::Generates => 62,  // 설기: 에너지 소모가 큰 달
            ElementRelation::Controlled => 52, // 관성: 도전적이고 압박 있는 달
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as saju;

    fn test_pillars() -> FourPillars {
        // 1990-01-15 14:00
        saju::calculate_four_pillars(1990, 1, 15, 14)
    }

    #[test]
    fn test_calculate_monthly_calculation_returns_12_months() {
        let pillars = test_pillars();
        let months = calculate_monthly_calculation(&pillars, 2026);
        assert_eq!(months.len(), 12);
        for (i, m) in months.iter().enumerate() {
            assert_eq!(m.month, (i + 1) as u32);
        }
    }

    #[test]
    fn test_monthly_scores_in_range() {
        let pillars = test_pillars();
        let months = calculate_monthly_calculation(&pillars, 2026);
        for m in &months {
            assert!(
                m.score >= 30 && m.score <= 98,
                "month {} score {} out of range",
                m.month,
                m.score
            );
            assert!(m.categories.overall >= 30 && m.categories.overall <= 98);
            assert!(m.categories.love >= 30 && m.categories.love <= 98);
            assert!(m.categories.career >= 30 && m.categories.career <= 98);
            assert!(m.categories.health >= 30 && m.categories.health <= 98);
            assert!(m.categories.wealth >= 30 && m.categories.wealth <= 98);
        }
    }

    #[test]
    fn test_monthly_grade_matches_score() {
        let pillars = test_pillars();
        let months = calculate_monthly_calculation(&pillars, 2026);
        for m in &months {
            let expected = score_to_grade(m.score);
            assert_eq!(
                m.grade.as_str(),
                expected,
                "month {} grade mismatch",
                m.month
            );
        }
    }

    #[test]
    fn test_monthly_calculation_deterministic() {
        let pillars = test_pillars();
        let first = calculate_monthly_calculation(&pillars, 2026);
        let second = calculate_monthly_calculation(&pillars, 2026);
        for (a, b) in first.iter().zip(second.iter()) {
            assert_eq!(a.score, b.score);
            assert_eq!(a.categories.love, b.categories.love);
        }
    }
}
