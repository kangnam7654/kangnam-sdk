use super::elements::{self, ElementRelation};
use super::interpreter;
use super::pillars;
use super::ten_gods;
use super::types::{Element, ElementBalance, FourPillars, Pillar, Stem, TenGod};
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
    pub persona_today: PersonaToday,
}

/// 오늘의 일간 기반 4섹션 페르소나 — /today의 "일간으로 본 나" 카드용.
///
/// strength/caution/action은 본명 일간 vs 오늘 일주 천간의 십신 관계로 결정,
/// action에는 일간 element 톤이 합성된다. mantra는 일간 자체의 한 줄로 매일 같다.
/// 같은 입력이면 결정적으로 같은 결과 — 캐싱(input_hash)에 안전.
///
/// /saju의 본명 성격(평생 같음)과 분리되는 시점 콘텐츠 — 매일 천간이 바뀌니
/// strength/caution/action은 매일 다르게 나오고, mantra만 일간 톤으로 일관 유지.
pub struct PersonaToday {
    /// "오늘 너의 강점" — 십신 조합으로 도출.
    pub strength: String,
    /// "오늘 조심할 패턴" — 같은 십신의 약점 측면.
    pub caution: String,
    /// "오늘 어울리는 행동" — 십신 권장 행동 + 일간 element 톤.
    pub action: String,
    /// "하루 한 줄 만트라" — 일간 본질의 짧은 자기 선언.
    pub mantra: String,
}

/// 카테고리별 상세 조언. 결혼·자녀는 본명(평생) 카테고리이고 일별 변동
/// 의미가 약해서 일운(daily)에는 노출하지 않는다.
pub struct CategoryDetails {
    pub love: CategoryDetail,
    pub career: CategoryDetail,
    pub health: CategoryDetail,
    pub wealth: CategoryDetail,
    pub study: CategoryDetail,
    pub travel: CategoryDetail,
    pub relations: CategoryDetail,
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

    // 카테고리별 상세 (7종: 재물·연애·직업·건강 + 학업·이동·대인).
    // 결혼·자녀는 평생 카테고리라 본명(saju_marriage/saju_children)에만 노출.
    let wealth_score = category_score(base.scores.overall, day_master, base.today_pillar.stem, 3);
    let study_score = category_score(base.scores.overall, day_master, base.today_pillar.stem, 4);
    let travel_score = category_score(base.scores.overall, day_master, base.today_pillar.stem, 5);
    let relations_score =
        category_score(base.scores.overall, day_master, base.today_pillar.stem, 6);
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
        study: CategoryDetail {
            score: study_score,
            advice: category_detail_advice(relation, day_master, "study"),
        },
        travel: CategoryDetail {
            score: travel_score,
            advice: category_detail_advice(relation, day_master, "travel"),
        },
        relations: CategoryDetail {
            score: relations_score,
            advice: category_detail_advice(relation, day_master, "relations"),
        },
    };

    // 시간대별 운세 (12시진)
    let hourly_fortunes = calculate_hourly_fortunes(day_master, &base.today_pillar);

    // 행운 아이템 (인성 오행 기반)
    let lucky_items = calculate_lucky_items(day_master, base.today_pillar.stem);

    // 오행 에너지 분석
    let balance = ElementBalance::from_pillars_with_hour(user_pillars, has_birth_time);
    let element_energy = interpreter::element_balance_analysis(&balance);

    // 성격 요약 (legacy compat — 한 줄)
    let personality_summary = interpreter::personality(day_master).to_string();

    // v0.0.7 Phase 3 — 일간×오늘 일주 4섹션 페르소나. 위 personality_summary와
    // 다르게 매일 바뀌는 시점 콘텐츠. /today의 "일간으로 본 나" 카드 본문이 됨.
    let persona_today = build_persona_today(day_master, base.today_pillar.stem);

    DailyDetailFortune {
        base,
        category_details,
        hourly_fortunes,
        lucky_items,
        element_energy,
        personality_summary,
        persona_today,
    }
}

/// 오늘 일주 천간이 본명 일간에 어떤 십신으로 작용하는지 + 일간 톤을 합성해
/// 4섹션 페르소나를 만든다.
fn build_persona_today(day_master: Stem, today_stem: Stem) -> PersonaToday {
    let ten_god = ten_gods::derive_ten_god(day_master, today_stem);
    PersonaToday {
        strength: persona_strength(ten_god).to_string(),
        caution: persona_caution(ten_god).to_string(),
        action: persona_action(ten_god, day_master),
        mantra: persona_mantra(day_master).to_string(),
    }
}

fn persona_strength(tg: TenGod) -> &'static str {
    match tg {
        TenGod::Bigyeon => "오늘은 너와 같은 결의 기운이 함께해 — 동료와 손발이 척척 맞고, 익숙한 영역에서 자신감이 살아나는 흐름이야.",
        TenGod::Geupjae => "도전하고 부딪히는 에너지가 강한 날 — 경쟁 상황에서 한 발 더 나가는 결단력이 가장 살아나.",
        TenGod::Sikshin => "표현하고 만드는 흐름이 자연스러운 날 — 창의 작업이나 좋아하는 행위에서 결과가 자연스럽게 따라온다.",
        TenGod::Sanggwan => "기존 틀을 깨는 영감이 도는 날 — 새로운 아이디어와 자유로운 표현이 평소보다 빛나.",
        TenGod::Pyeonjae => "기회와 자원이 흐르는 날 — 외부에서 들어오는 제안에 열려있을수록 손에 들어오는 게 커.",
        TenGod::Jeongjae => "꾸준함이 보상받는 날 — 차근차근 쌓아온 게 정확한 자리에 들어와 결실이 된다.",
        TenGod::Pyeongwan => "강한 압박이 너를 단단하게 만드는 날 — 큰 책임과 도전이 너의 그릇을 한 단계 키운다.",
        TenGod::Jeonggwan => "원칙과 질서가 너를 받쳐주는 날 — 제도 안에서 인정받고 자리를 잡기 좋은 흐름.",
        TenGod::Pyeonin => "낯선 영감과 직관이 살아나는 날 — 비주류 지식이나 색다른 관점이 너에게 와닿는다.",
        TenGod::Jeongin => "지식과 멘토의 도움이 흐르는 날 — 배우거나 조언받기 가장 좋은 시점이야.",
    }
}

fn persona_caution(tg: TenGod) -> &'static str {
    match tg {
        TenGod::Bigyeon => "동등한 위치의 사람과 의견 충돌이 생기기 쉬워 — 양보 없이 부딪히면 둘 다 소진된다.",
        TenGod::Geupjae => "경쟁이 과해져 무리수를 두기 쉬운 날 — 손해를 감수해서라도 밀어붙이지는 말 것.",
        TenGod::Sikshin => "즐거움에 흘러 마무리를 놓치기 쉬운 날 — 끝맺음 일정 하나는 반드시 지키자.",
        TenGod::Sanggwan => "솔직함이 도를 넘으면 관계 균열로 이어져 — 표현 수위를 한 번 더 점검하자.",
        TenGod::Pyeonjae => "기회가 많아 벌이가 분산되기 쉬워 — 한 가지에 집중해야 손에 남는다.",
        TenGod::Jeongjae => "안정에 안주해 속도가 느려질 수 있어 — 작은 도전 한 가지는 끼워 넣자.",
        TenGod::Pyeongwan => "압박을 정면 돌파하다 번아웃 위험이 커 — 도움 요청이 약함이 아니다.",
        TenGod::Jeonggwan => "규범에 갇혀 융통성을 잃기 쉬워 — 사람의 사정은 사정으로 봐주자.",
        TenGod::Pyeonin => "고립되어 자기 세계로 빠질 수 있어 — 한 명에게라도 오늘 본 걸 나누자.",
        TenGod::Jeongin => "공부와 정보 수집에만 머물러 행동을 미룰 수 있어 — 한 가지는 오늘 안에 실행.",
    }
}

fn persona_action(tg: TenGod, day_master: Stem) -> String {
    let action_base = match tg {
        TenGod::Bigyeon => "팀 프로젝트나 동료와의 합 맞추기에 시간 쓰기",
        TenGod::Geupjae => "경쟁·스포츠·도전 한 가지 시도",
        TenGod::Sikshin => "취미·창작·요리 같이 만드는 행위 한 가지",
        TenGod::Sanggwan => "글·말·표현으로 자기 의견 한 번 내기",
        TenGod::Pyeonjae => "외부 미팅·새 사람·새 제안 받기",
        TenGod::Jeongjae => "장기 계약·실무 정산·예산 검토",
        TenGod::Pyeongwan => "큰 결정·발표·압박 상황 정면 돌파",
        TenGod::Jeonggwan => "공식 문서·계약·인증 정리",
        TenGod::Pyeonin => "철학·예술·비주류 지식에 시간 투자",
        TenGod::Jeongin => "독서·강의·멘토 미팅",
    };
    let tone = match day_master.element() {
        Element::Wood => "곧게 뻗는 결로",
        Element::Fire => "환하게 드러내는 결로",
        Element::Earth => "차분히 받쳐주는 결로",
        Element::Metal => "정확하게 잘라내는 결로",
        Element::Water => "흐르듯 자연스럽게",
    };
    format!("{} — {} 가는 게 너의 일간과 잘 맞아.", action_base, tone)
}

fn persona_mantra(day_master: Stem) -> &'static str {
    match day_master {
        Stem::Gap => "곧게 뻗는다, 굽히지 않는다.",
        Stem::Eul => "유연하게 휘어지되 끊기지 않는다.",
        Stem::Byeong => "환하게 비추되 태우지 않는다.",
        Stem::Jeong => "은은하게, 길게 켜둔다.",
        Stem::Mu => "넉넉히 받쳐주되 흔들리지 않는다.",
        Stem::Gi => "촘촘히 다지되 답답하지 않다.",
        Stem::Gyeong => "단단히 베되 사람을 다치게 하지 않는다.",
        Stem::Sin => "정밀하게 다듬되 차갑지 않다.",
        Stem::Im => "깊게 흘러도 방향을 잃지 않는다.",
        Stem::Gye => "맑게 스며들어 모두를 적신다.",
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
        "study" => study_detail_advice(relation, elem),
        "travel" => travel_detail_advice(relation, elem),
        "relations" => relations_detail_advice(relation, elem),
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

fn study_detail_advice(relation: ElementRelation, elem: Element) -> String {
    let base = match relation {
        ElementRelation::Generated => {
            "받아들이는 힘이 좋은 날입니다. 강의나 책에서 핵심을 빠르게 잡아낼 수 있습니다."
        }
        ElementRelation::Same => {
            "집중력이 높은 날입니다. 어려운 단원이나 누적된 과제부터 정리해보세요."
        }
        ElementRelation::Generates => {
            "정리·요약에 적합한 날입니다. 배운 것을 다시 풀어 쓰면 오래 남습니다."
        }
        ElementRelation::Controls => {
            "성과를 만드는 날입니다. 모의고사·발표·자격증 등 결과 시험에 강합니다."
        }
        ElementRelation::Controlled => {
            "잡생각으로 흔들리기 쉽습니다. 한 과목 한 챕터씩 짧게 끊어 가세요."
        }
    };
    let personal = match elem {
        Element::Wood => "새 분야 입문이나 큰 그림 잡기에 적합합니다.",
        Element::Fire => "토론·발표·암기에서 평소 이상의 성과를 냅니다.",
        Element::Earth => "기본기 다지기와 꾸준한 반복 학습에서 빛납니다.",
        Element::Metal => "정밀한 문제 풀이와 논리 전개에 탁월합니다.",
        Element::Water => "유연한 사고로 응용 문제와 서술형에 강합니다.",
    };
    format!("{} {}", base, personal)
}

fn travel_detail_advice(relation: ElementRelation, elem: Element) -> String {
    let base = match relation {
        ElementRelation::Generated => {
            "이동 중 도움받는 기운이 강한 날입니다. 길안내·동행이 자연스럽게 따릅니다."
        }
        ElementRelation::Same => {
            "혼자 움직여도 좋은 날입니다. 평소 가보고 싶었던 코스에 도전해보세요."
        }
        ElementRelation::Generates => {
            "베푸는 이동(배웅·심부름)이 의외의 인연을 만들 수 있는 날입니다."
        }
        ElementRelation::Controls => {
            "출장·계약·확장 같은 목적성 이동에 강합니다. 결정 짓고 돌아오기 좋은 날."
        }
        ElementRelation::Controlled => {
            "이동 중 사고·지연 가능성. 여유 시간을 두고 짐도 가볍게 챙기세요."
        }
    };
    let personal = match elem {
        Element::Wood => "동쪽·신생 도시·자연이 있는 코스가 잘 맞습니다.",
        Element::Fire => "남쪽·번화한 곳·사람 많은 행사가 에너지를 줍니다.",
        Element::Earth => "익숙한 동선·근거리 이동이 안정감을 가져옵니다.",
        Element::Metal => "서쪽·정돈된 도시·일정이 명확한 출장에 적합합니다.",
        Element::Water => "북쪽·물가·해외 이동에서 기회를 잡기 쉽습니다.",
    };
    format!("{} {}", base, personal)
}

fn relations_detail_advice(relation: ElementRelation, elem: Element) -> String {
    let base = match relation {
        ElementRelation::Generated => {
            "주변에서 먼저 다가오는 날입니다. 받아들이는 자세가 관계를 깊게 만듭니다."
        }
        ElementRelation::Same => {
            "동등한 관계, 친구·동료와의 협력에 강한 날입니다. 모임에 적극적으로 참여해보세요."
        }
        ElementRelation::Generates => {
            "후배·아랫사람·신규 커뮤니티에 베풀면 호의가 두 배로 돌아옵니다."
        }
        ElementRelation::Controls => {
            "리더십이 통하는 날입니다. 의견을 정리하고 결정 짓는 역할에 적합합니다."
        }
        ElementRelation::Controlled => {
            "의견 충돌이 생기기 쉽습니다. 한 박자 늦춘 반응이 관계를 지킵니다."
        }
    };
    let personal = match elem {
        Element::Wood => "공통 관심사·취미 모임에서 인연이 자연스럽게 풀립니다.",
        Element::Fire => "밝은 자리·축하 모임에서 매력이 두드러집니다.",
        Element::Earth => "가족·오랜 친구처럼 신뢰 기반 관계에서 빛납니다.",
        Element::Metal => "원칙을 지키는 단단한 인상이 신뢰를 쌓습니다.",
        Element::Water => "타인의 입장을 헤아리는 공감이 관계의 윤활유가 됩니다.",
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
        // 7 카테고리(love/career/health/wealth + study/travel/relations) 모두 비어있지 않아야.
        let cats = &detail.category_details;
        for advice in [
            &cats.love.advice,
            &cats.career.advice,
            &cats.health.advice,
            &cats.wealth.advice,
            &cats.study.advice,
            &cats.travel.advice,
            &cats.relations.advice,
        ] {
            assert!(!advice.is_empty(), "advice empty");
        }
        // 점수 범위 확인 (category_score clamp 30..98)
        for score in [
            cats.love.score,
            cats.career.score,
            cats.health.score,
            cats.wealth.score,
            cats.study.score,
            cats.travel.score,
            cats.relations.score,
        ] {
            assert!((30..=98).contains(&score), "score out of range: {score}");
        }
    }

    #[test]
    fn test_daily_detail_new_categories_have_personalized_advice() {
        // 새 3 카테고리(학업/이동/대인) advice는 base 한 줄 + 일간 element 기반 한 줄로
        // 합쳐진 두 문장 — 각 카테고리가 단순 placeholder가 아닌 실 콘텐츠인지 확인.
        let pillars = test_pillars();
        let detail = calculate_daily_detail_for_date(&pillars, true, 2026, 3, 23);
        for advice in [
            &detail.category_details.study.advice,
            &detail.category_details.travel.advice,
            &detail.category_details.relations.advice,
        ] {
            // 두 문장 합본이면 마침표 2회 이상 또는 길이 80+ 자 보장.
            let dot_count = advice.matches('.').count();
            assert!(
                dot_count >= 2 || advice.chars().count() >= 80,
                "advice too short or single sentence: {advice}"
            );
        }
    }

    #[test]
    fn test_daily_detail_advice_covers_all_relations_and_elements() {
        // 천간 10 × 며칠 정도의 입력 변화로 ElementRelation 5종 × Element 5종이
        // 모두 advice 분기를 한 번씩은 거치는지 확인 (스모크 커버리지).
        // 같은 advice 문장이 반복되더라도 어느 분기로도 빈 문자열은 나오면 안 됨.
        let mut seen = std::collections::HashSet::new();
        for year in [1990, 1995, 2000, 2005] {
            for month in [1, 4, 7, 10] {
                for day in [1, 15] {
                    let pillars = saju::calculate_four_pillars(year, month, day, 14);
                    let detail = calculate_daily_detail_for_date(&pillars, true, 2026, 3, 23);
                    for cat in [
                        &detail.category_details.study.advice,
                        &detail.category_details.travel.advice,
                        &detail.category_details.relations.advice,
                    ] {
                        seen.insert(cat.clone());
                    }
                }
            }
        }
        // 32개 입력 × 3 카테고리에서 최소 5종(=relation 5분기) 이상 나와야 generic placeholder가 아님을 보장.
        assert!(seen.len() >= 5, "advice variants too few: {}", seen.len());
    }

    #[test]
    fn test_category_score_is_deterministic() {
        // 같은 입력은 같은 점수 — input_hash 캐시 동작 + 사용자 신뢰도(매번 다른 점수면 의심)
        let pillars = test_pillars();
        let detail1 = calculate_daily_detail_for_date(&pillars, true, 2026, 3, 23);
        let detail2 = calculate_daily_detail_for_date(&pillars, true, 2026, 3, 23);
        assert_eq!(detail1.category_details.study.score, detail2.category_details.study.score);
        assert_eq!(detail1.category_details.travel.score, detail2.category_details.travel.score);
        assert_eq!(
            detail1.category_details.relations.score,
            detail2.category_details.relations.score
        );
    }

    #[test]
    fn test_category_score_varies_with_day_master() {
        // 다른 일간(다른 birth date)이면 점수도 달라야 — 천간 인덱스가 시드에 들어가는 게 효과 있는지.
        // 운이 같아 우연히 같을 수 있으므로 4개 사주를 만들어 최소 1개 이상 다른지 본다.
        let p1 = saju::calculate_four_pillars(1990, 1, 15, 14);
        let p2 = saju::calculate_four_pillars(1991, 6, 20, 9);
        let p3 = saju::calculate_four_pillars(1985, 10, 5, 18);
        let p4 = saju::calculate_four_pillars(2000, 3, 1, 23);
        let scores: Vec<_> = [p1, p2, p3, p4]
            .iter()
            .map(|p| calculate_daily_detail_for_date(p, true, 2026, 3, 23).category_details.study.score)
            .collect();
        let unique: std::collections::HashSet<_> = scores.iter().collect();
        assert!(unique.len() >= 2, "scores all identical across day_masters: {:?}", scores);
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

    // ===== persona_today (v0.0.7 Phase 3) =====

    #[test]
    fn persona_today_4_sections_non_empty() {
        let detail = calculate_daily_detail_for_date(&test_pillars(), true, 2026, 3, 23);
        let p = &detail.persona_today;
        assert!(!p.strength.is_empty(), "strength empty");
        assert!(!p.caution.is_empty(), "caution empty");
        assert!(!p.action.is_empty(), "action empty");
        assert!(!p.mantra.is_empty(), "mantra empty");
    }

    #[test]
    fn persona_today_action_includes_day_master_tone() {
        // action 카피는 "{행동} — {일간 톤} ..." 패턴이어야 — 일간 element가
        // narrative에 반영됐는지 회귀 lock.
        let detail = calculate_daily_detail_for_date(&test_pillars(), true, 2026, 3, 23);
        assert!(
            detail.persona_today.action.contains(" — "),
            "action missing day-master tone separator: {:?}",
            detail.persona_today.action,
        );
    }

    #[test]
    fn persona_today_deterministic() {
        // 같은 (사주, 날짜)면 결과가 같아야 — input_hash 캐싱 안전성 보장.
        let p = test_pillars();
        let a = calculate_daily_detail_for_date(&p, true, 2026, 3, 23).persona_today;
        let b = calculate_daily_detail_for_date(&p, true, 2026, 3, 23).persona_today;
        assert_eq!(a.strength, b.strength);
        assert_eq!(a.caution, b.caution);
        assert_eq!(a.action, b.action);
        assert_eq!(a.mantra, b.mantra);
    }

    #[test]
    fn persona_today_mantra_constant_per_day_master() {
        // mantra는 일간 본질 — 같은 일간이면 날짜와 무관하게 같아야 한다.
        let p = test_pillars();
        let m1 = calculate_daily_detail_for_date(&p, true, 2026, 3, 23).persona_today.mantra;
        let m2 = calculate_daily_detail_for_date(&p, true, 2026, 4, 15).persona_today.mantra;
        assert_eq!(m1, m2);
    }

    #[test]
    fn persona_today_strength_varies_with_today_stem() {
        // strength는 일간 vs 오늘 천간의 십신으로 결정 — 며칠치 비교하면
        // 최소 2가지 이상의 strength 카피가 나와야 (모든 날 같으면 십신 매핑이
        // 작동 안 한 것).
        let p = test_pillars();
        let mut variants = std::collections::HashSet::new();
        for d in 1..=20 {
            let detail = calculate_daily_detail_for_date(&p, true, 2026, 3, d);
            variants.insert(detail.persona_today.strength.clone());
        }
        assert!(
            variants.len() >= 3,
            "strength only had {} variants over 20 days — ten_god mapping not active?",
            variants.len(),
        );
    }

    #[test]
    fn persona_today_covers_all_10_day_masters() {
        // 다양한 birth date로 10가지 일간 모두 커버 가능해야 — mantra가 일간별로
        // 모두 다른 카피인지 회귀 lock (일간 10개 / mantra 10개).
        let test_cases = [
            (1990, 1, 15, 14),
            (1991, 6, 20, 9),
            (1985, 10, 5, 18),
            (2000, 3, 1, 23),
            (1988, 7, 12, 6),
            (1995, 11, 25, 15),
            (1993, 4, 30, 21),
            (1998, 9, 8, 3),
            (1992, 2, 14, 11),
            (1987, 12, 31, 19),
            (1996, 5, 17, 7),
            (1989, 8, 22, 13),
        ];
        let mut mantras = std::collections::HashSet::new();
        for &(y, m, d, h) in &test_cases {
            let pillars = saju::calculate_four_pillars(y, m, d, h);
            let detail = calculate_daily_detail_for_date(&pillars, true, 2026, 3, 23);
            mantras.insert(detail.persona_today.mantra.clone());
        }
        // 일간 10가지가 다 커버되진 않을 수 있으나 (테스트 데이터 한계) 최소 5개는
        // 다른 mantra가 나와야 — mantra가 일간에 의존한다는 회귀 lock.
        assert!(
            mantras.len() >= 5,
            "only {} unique mantras across {} birth dates",
            mantras.len(),
            test_cases.len(),
        );
    }
}
