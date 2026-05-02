use super::elements::{self, ElementRelation};
use super::interpreter;
use super::pillars;
use super::types::{Element, ElementBalance, FourPillars, Pillar, Stem};
use chrono::Utc;

/// 오늘의 운세 점수 및 조언 생성
pub struct DailyFortune {
    pub date: String,
    pub today_pillar: Pillar,
    pub day_master: Stem,
    pub relation: ElementRelation,
    pub scores: DailyScores,
    pub advice: String,
    pub caution: String,
}

pub struct DailyScores {
    pub overall: i32,
    pub love: i32,
    pub career: i32,
    pub health: i32,
}

/// 특정 날짜의 운세 계산 (캘린더 용도)
pub fn calculate_daily_for_date(
    user_pillars: &FourPillars,
    year: i32,
    month: u32,
    day: u32,
) -> DailyFortune {
    let today_pillar = pillars::day_pillar(year, month, day);
    let day_master = user_pillars.day.stem;

    // 오늘 일주 천간의 오행 vs 유저 일간의 오행 관계
    let relation = elements::relation(day_master.element(), today_pillar.stem.element());
    let base = relation.daily_score_base();

    // 지지 관계로 미세 조정
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

    // 카테고리별 점수 (기본 점수에서 변동)
    let love = category_score(overall, day_master, today_pillar.stem, 0);
    let career = category_score(overall, day_master, today_pillar.stem, 1);
    let health = category_score(overall, day_master, today_pillar.stem, 2);

    let advice = daily_advice(relation, day_master);
    let caution = daily_caution(relation, day_master);

    let date_str = format!("{:04}-{:02}-{:02}", year, month, day);

    DailyFortune {
        date: date_str,
        today_pillar,
        day_master,
        relation,
        scores: DailyScores {
            overall,
            love,
            career,
            health,
        },
        advice,
        caution,
    }
}

/// 오늘의 운세 계산 (KST 기준)
pub fn calculate_daily(user_pillars: &FourPillars) -> DailyFortune {
    use chrono::Datelike;
    let kst = chrono::FixedOffset::east_opt(9 * 3600).unwrap();
    let today = Utc::now().with_timezone(&kst).date_naive();
    calculate_daily_for_date(user_pillars, today.year(), today.month(), today.day())
}

fn category_score(base: i32, day_master: Stem, today_stem: Stem, category: u8) -> i32 {
    // 간단한 결정론적 변동: 천간 인덱스 조합으로
    let seed = day_master.index() * 10 + today_stem.index() + category as usize;
    let variation = ((seed * 7 + 13) % 21) as i32 - 10; // -10 ~ +10
    (base + variation).clamp(30, 98)
}

fn daily_advice(relation: ElementRelation, day_master: Stem) -> String {
    let elem_advice = match relation {
        ElementRelation::Generated => {
            "오늘은 도움을 받는 기운이 강합니다. 주변의 조언에 귀 기울이세요."
        }
        ElementRelation::Same => "오늘은 자신감이 넘치는 날입니다. 주도적으로 일을 추진하세요.",
        ElementRelation::Generates => {
            "오늘은 베푸는 기운이 강합니다. 나눔을 통해 좋은 인연이 생깁니다."
        }
        ElementRelation::Controls => "오늘은 재물운이 활발합니다. 투자나 거래에 좋은 시기입니다.",
        ElementRelation::Controlled => {
            "오늘은 도전이 있지만 성장의 기회입니다. 겸손한 자세가 행운을 부릅니다."
        }
    };

    let personal = match day_master.element() {
        super::types::Element::Wood => "특히 새로운 시작이나 계획 수립에 좋은 시간입니다.",
        super::types::Element::Fire => "사람들과의 교류가 좋은 기운을 가져옵니다.",
        super::types::Element::Earth => "안정적인 판단이 좋은 결과를 만듭니다.",
        super::types::Element::Metal => "명확한 결단이 필요한 순간, 직감을 믿으세요.",
        super::types::Element::Water => "유연한 사고가 새로운 길을 열어줍니다.",
    };

    format!("{} {}", elem_advice, personal)
}

fn daily_caution(relation: ElementRelation, day_master: Stem) -> String {
    let elem_caution = match relation {
        ElementRelation::Generated => "지나친 의존은 피하세요. 스스로의 판단도 중요합니다.",
        ElementRelation::Same => "자신감이 과도하면 독선이 될 수 있으니 주의하세요.",
        ElementRelation::Generates => "에너지 소모가 많은 날입니다. 무리하지 마세요.",
        ElementRelation::Controls => "욕심을 부리면 오히려 손해를 볼 수 있습니다.",
        ElementRelation::Controlled => "스트레스 관리에 신경 쓰세요. 충분한 휴식이 필요합니다.",
    };

    let personal = match day_master.element() {
        super::types::Element::Wood => "간과 눈 건강에 유의하세요.",
        super::types::Element::Fire => "심장과 혈압 관리에 주의하세요.",
        super::types::Element::Earth => "소화기 건강에 신경 쓰세요.",
        super::types::Element::Metal => "호흡기와 피부 건강에 유의하세요.",
        super::types::Element::Water => "신장과 수분 섭취에 관심을 가지세요.",
    };

    format!("{} {}", elem_caution, personal)
}

// ========== daily_detail (유료 15P 상세 운세) ==========

/// 상세 운세 전체 결과
pub struct DailyDetailFortune {
    pub base: DailyFortune,
    pub category_details: CategoryDetails,
    pub hourly_fortunes: Vec<HourlyFortune>,
    pub lucky_items: LuckyItems,
    pub element_energy: String,
    pub personality_summary: String,
}

/// 카테고리별 상세 조언
pub struct CategoryDetails {
    pub love: CategoryDetail,
    pub career: CategoryDetail,
    pub health: CategoryDetail,
    pub wealth: CategoryDetail,
}

pub struct CategoryDetail {
    pub score: i32,
    pub advice: String,
}

/// 시간대별 운세
pub struct HourlyFortune {
    pub hour_name: String,
    pub hour_range: String,
    pub score: i32,
    pub description: String,
}

/// 행운 아이템
pub struct LuckyItems {
    pub color: String,
    pub color_hex: String,
    pub number: i32,
    pub direction: String,
}

/// 상세 운세 계산 (KST 기준)
pub fn calculate_daily_detail(
    user_pillars: &FourPillars,
    has_birth_time: bool,
) -> DailyDetailFortune {
    use chrono::Datelike;
    let kst = chrono::FixedOffset::east_opt(9 * 3600).unwrap();
    let today = Utc::now().with_timezone(&kst).date_naive();
    calculate_daily_detail_for_date(
        user_pillars,
        has_birth_time,
        today.year(),
        today.month(),
        today.day(),
    )
}

/// 특정 날짜의 상세 운세 계산
pub fn calculate_daily_detail_for_date(
    user_pillars: &FourPillars,
    has_birth_time: bool,
    year: i32,
    month: u32,
    day: u32,
) -> DailyDetailFortune {
    let base = calculate_daily_for_date(user_pillars, year, month, day);
    let day_master = user_pillars.day.stem;
    let relation = base.relation;

    // 카테고리별 상세 (재물 추가)
    let wealth_score = category_score(base.scores.overall, day_master, base.today_pillar.stem, 3);
    let category_details = CategoryDetails {
        love: CategoryDetail {
            score: base.scores.love,
            advice: category_detail_advice(relation, day_master, "love"),
        },
        career: CategoryDetail {
            score: base.scores.career,
            advice: category_detail_advice(relation, day_master, "career"),
        },
        health: CategoryDetail {
            score: base.scores.health,
            advice: category_detail_advice(relation, day_master, "health"),
        },
        wealth: CategoryDetail {
            score: wealth_score,
            advice: category_detail_advice(relation, day_master, "wealth"),
        },
    };

    // 시간대별 운세 (12시진)
    let hourly_fortunes = calculate_hourly_fortunes(day_master, &base.today_pillar);

    // 행운 아이템 (인성 오행 기반)
    let lucky_items = calculate_lucky_items(day_master, base.today_pillar.stem);

    // 오행 에너지 분석
    let balance = ElementBalance::from_pillars_with_hour(user_pillars, has_birth_time);
    let element_energy = interpreter::element_balance_analysis(&balance);

    // 성격 요약
    let personality_summary = interpreter::personality(day_master).to_string();

    DailyDetailFortune {
        base,
        category_details,
        hourly_fortunes,
        lucky_items,
        element_energy,
        personality_summary,
    }
}

/// 12시진 시간대별 운세
fn calculate_hourly_fortunes(day_master: Stem, today_pillar: &Pillar) -> Vec<HourlyFortune> {
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
            // 지지 관계 미세 조정
            let branch_rel = elements::relation(day_master.element(), hour_pillar.branch.element());
            let adj = match branch_rel {
                ElementRelation::Generated => 3,
                ElementRelation::Same => 2,
                ElementRelation::Generates => -1,
                ElementRelation::Controls => 0,
                ElementRelation::Controlled => -3,
            };
            let score = (base_score + adj).clamp(30, 98);

            let description = hourly_description(rel, day_master.element());

            HourlyFortune {
                hour_name: name.to_string(),
                hour_range: range.to_string(),
                score,
                description,
            }
        })
        .collect()
}

/// 시간대별 한줄 설명
fn hourly_description(rel: ElementRelation, day_elem: Element) -> String {
    let base = match rel {
        ElementRelation::Generated => "도움과 지원을 받기 좋은 시간입니다.",
        ElementRelation::Same => "자신감이 넘치고 주도적으로 움직이기 좋습니다.",
        ElementRelation::Generates => "창의적인 활동에 적합한 시간입니다.",
        ElementRelation::Controls => "재물운이 활성화되는 시간입니다.",
        ElementRelation::Controlled => "신중하게 행동하는 것이 좋습니다.",
    };
    let tip = match day_elem {
        Element::Wood => "계획 수립이나 학습에 좋습니다.",
        Element::Fire => "사교 활동이나 프레젠테이션에 적합합니다.",
        Element::Earth => "실무 처리나 계약에 유리합니다.",
        Element::Metal => "중요한 결정을 내리기 좋습니다.",
        Element::Water => "아이디어 구상이나 명상에 좋습니다.",
    };
    format!("{} {}", base, tip)
}

/// 카테고리별 상세 조언 (2-3문장)
fn category_detail_advice(relation: ElementRelation, day_master: Stem, category: &str) -> String {
    let elem = day_master.element();
    match category {
        "love" => love_detail_advice(relation, elem),
        "career" => career_detail_advice(relation, elem),
        "health" => health_detail_advice(relation, elem),
        "wealth" => wealth_detail_advice(relation, elem),
        _ => String::new(),
    }
}

fn love_detail_advice(relation: ElementRelation, elem: Element) -> String {
    let base = match relation {
        ElementRelation::Generated => {
            "주변 사람들의 따뜻한 관심이 느껴지는 날입니다. 솔직한 감정 표현이 관계를 더 깊게 만듭니다."
        }
        ElementRelation::Same => {
            "자기 매력이 빛나는 날입니다. 당당한 모습이 상대에게 좋은 인상을 줍니다."
        }
        ElementRelation::Generates => {
            "상대를 위한 배려가 빛을 발합니다. 소소한 선물이나 따뜻한 말 한마디가 큰 감동을 줍니다."
        }
        ElementRelation::Controls => {
            "적극적인 어프로치가 효과적입니다. 자신의 감정을 솔직하게 표현해보세요."
        }
        ElementRelation::Controlled => {
            "감정 조절이 중요한 날입니다. 서두르지 말고 상대의 속도에 맞춰주세요."
        }
    };
    let personal = match elem {
        Element::Wood => "진정성 있는 대화가 마음의 거리를 좁혀줍니다.",
        Element::Fire => "유머와 밝은 에너지가 인연을 끌어당깁니다.",
        Element::Earth => "안정감 있는 태도가 신뢰를 쌓습니다.",
        Element::Metal => "진심을 담은 행동이 말보다 큰 울림을 줍니다.",
        Element::Water => "상대의 이야기에 공감하는 것이 최고의 사랑 표현입니다.",
    };
    format!("{} {}", base, personal)
}

fn career_detail_advice(relation: ElementRelation, elem: Element) -> String {
    let base = match relation {
        ElementRelation::Generated => {
            "상사나 동료의 지원이 기대되는 날입니다. 협업 프로젝트에서 좋은 성과를 낼 수 있습니다."
        }
        ElementRelation::Same => {
            "리더십을 발휘하기 좋은 날입니다. 자신의 아이디어를 적극적으로 제안해보세요."
        }
        ElementRelation::Generates => {
            "창의적인 업무에 몰두하기 좋습니다. 새로운 접근 방식을 시도해보세요."
        }
        ElementRelation::Controls => {
            "실질적인 성과를 만들기 좋은 날입니다. 목표를 구체적으로 설정하고 실행하세요."
        }
        ElementRelation::Controlled => {
            "업무 우선순위를 재정리하세요. 핵심에 집중하면 부담이 줄어듭니다."
        }
    };
    let personal = match elem {
        Element::Wood => "장기 프로젝트의 기획이나 전략 수립에 적합합니다.",
        Element::Fire => "팀 미팅이나 발표에서 돋보이는 시간입니다.",
        Element::Earth => "꼼꼼한 실무 처리가 높은 평가를 받습니다.",
        Element::Metal => "문제 해결이나 분석 업무에서 능력을 발휘합니다.",
        Element::Water => "유연한 대처와 네트워킹이 기회를 만듭니다.",
    };
    format!("{} {}", base, personal)
}

fn health_detail_advice(relation: ElementRelation, elem: Element) -> String {
    let base = match relation {
        ElementRelation::Generated => {
            "전반적으로 활력이 넘치는 날입니다. 가벼운 운동으로 에너지를 더 끌어올리세요."
        }
        ElementRelation::Same => {
            "컨디션이 좋은 날입니다. 평소 미루던 건강 관리를 시작하기 좋습니다."
        }
        ElementRelation::Generates => {
            "에너지 소모가 많은 날입니다. 충분한 수분 섭취와 휴식이 필요합니다."
        }
        ElementRelation::Controls => "활동적이지만 과로에 주의하세요. 일과 휴식의 균형을 맞추세요.",
        ElementRelation::Controlled => {
            "스트레스가 쌓이기 쉬운 날입니다. 명상이나 스트레칭으로 긴장을 풀어주세요."
        }
    };
    let personal = match elem {
        Element::Wood => "간 기능과 시력 관리에 신경 쓰세요. 녹색 채소 섭취가 도움됩니다.",
        Element::Fire => "심장과 혈액순환에 유의하세요. 가벼운 유산소 운동을 추천합니다.",
        Element::Earth => "소화기 건강이 중요합니다. 규칙적인 식사와 따뜻한 음식이 좋습니다.",
        Element::Metal => "호흡기와 피부에 관심을 가지세요. 보습과 환기에 신경 쓰세요.",
        Element::Water => "신장 기능과 수분 균형이 핵심입니다. 물을 자주 마시세요.",
    };
    format!("{} {}", base, personal)
}

fn wealth_detail_advice(relation: ElementRelation, elem: Element) -> String {
    let base = match relation {
        ElementRelation::Generated => {
            "자산 관리에 유리한 날입니다. 장기 투자나 저축 계획을 세워보세요."
        }
        ElementRelation::Same => {
            "안정적인 재물운입니다. 현재 상태를 유지하며 불필요한 지출을 줄이세요."
        }
        ElementRelation::Generates => {
            "지출이 많아질 수 있는 날입니다. 계획에 없던 소비는 하루 미루세요."
        }
        ElementRelation::Controls => {
            "재물운이 가장 좋은 날입니다. 적극적인 투자나 비즈니스 제안에 열려 있으세요."
        }
        ElementRelation::Controlled => {
            "예상치 못한 지출에 주의하세요. 큰 금액의 결정은 내일로 미루는 것이 안전합니다."
        }
    };
    let personal = match elem {
        Element::Wood => "새로운 수익원을 발굴하기 좋은 시기입니다.",
        Element::Fire => "대인관계를 통한 재물 기회에 주목하세요.",
        Element::Earth => "안정적이고 꾸준한 저축이 큰 자산이 됩니다.",
        Element::Metal => "분석적 판단으로 현명한 소비를 할 수 있습니다.",
        Element::Water => "다양한 포트폴리오로 리스크를 분산하세요.",
    };
    format!("{} {}", base, personal)
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
        // 1990-01-15 14:00 (갑목 일간 기준 테스트)
        saju::calculate_four_pillars(1990, 1, 15, 14)
    }

    #[test]
    fn test_daily_detail_has_12_hourly_fortunes() {
        let pillars = test_pillars();
        let detail = calculate_daily_detail_for_date(&pillars, true, 2026, 3, 23);
        assert_eq!(detail.hourly_fortunes.len(), 12);
        // 각 시진 이름이 비어있지 않은지 확인
        for h in &detail.hourly_fortunes {
            assert!(!h.hour_name.is_empty());
            assert!(!h.hour_range.is_empty());
            assert!(!h.description.is_empty());
            assert!(h.score >= 30 && h.score <= 98);
        }
    }

    #[test]
    fn test_daily_detail_lucky_items_has_all_fields() {
        let pillars = test_pillars();
        let detail = calculate_daily_detail_for_date(&pillars, true, 2026, 3, 23);
        assert!(!detail.lucky_items.color.is_empty());
        assert!(detail.lucky_items.color_hex.starts_with('#'));
        assert!(detail.lucky_items.number >= 1 && detail.lucky_items.number <= 9);
        assert!(!detail.lucky_items.direction.is_empty());
    }

    #[test]
    fn test_daily_detail_category_details_all_non_empty() {
        let pillars = test_pillars();
        let detail = calculate_daily_detail_for_date(&pillars, true, 2026, 3, 23);
        assert!(!detail.category_details.love.advice.is_empty());
        assert!(!detail.category_details.career.advice.is_empty());
        assert!(!detail.category_details.health.advice.is_empty());
        assert!(!detail.category_details.wealth.advice.is_empty());
        // 점수 범위 확인
        for score in [
            detail.category_details.love.score,
            detail.category_details.career.score,
            detail.category_details.health.score,
            detail.category_details.wealth.score,
        ] {
            assert!((30..=98).contains(&score));
        }
    }

    #[test]
    fn test_daily_detail_is_superset_of_daily() {
        let pillars = test_pillars();
        let daily = calculate_daily_for_date(&pillars, 2026, 3, 23);
        let detail = calculate_daily_detail_for_date(&pillars, true, 2026, 3, 23);
        // base 필드가 동일해야 함
        assert_eq!(detail.base.date, daily.date);
        assert_eq!(detail.base.scores.overall, daily.scores.overall);
        assert_eq!(detail.base.scores.love, daily.scores.love);
        assert_eq!(detail.base.scores.career, daily.scores.career);
        assert_eq!(detail.base.scores.health, daily.scores.health);
        assert_eq!(detail.base.advice, daily.advice);
        assert_eq!(detail.base.caution, daily.caution);
        // 추가 필드 존재 확인
        assert!(!detail.element_energy.is_empty());
        assert!(!detail.personality_summary.is_empty());
    }
}
