use crate::{
    self as saju, branches, calendar, daeun, daily, elements, gongmang, lucky, monthly, pillars,
    profile, shinsal, ten_gods, types::*,
};
use chrono::{Datelike, NaiveDate};
use serde_json::{Value, json};

pub struct SajuEngine;

/// 엔진 버전. 캐시 무효화 기준으로 사용된다.
pub const SAJU_ENGINE_VERSION: &str = "saju-v1.7";

/// Calculation-first saju schema. This path intentionally excludes prose
/// fields so app/backend layers can compose their own user-facing explanation.
pub const SAJU_CORE_SCHEMA_VERSION: &str = "saju_core_v2";

/// Public reading type keys supported by the saju engine.
pub const SAJU_READING_TYPES: [&str; 18] = [
    "daily",
    "daily_detail",
    "saju",
    "saju_wealth",
    "saju_love",
    "saju_marriage",
    "saju_career",
    "saju_health",
    "saju_study",
    "saju_children",
    "saju_travel",
    "saju_relations",
    "weekly",
    "monthly",
    "compatibility",
    "compatibility_detail",
    "monthly_fortune",
    "daeun",
];

/// Returns whether `reading_type` is a supported saju reading type key.
pub fn is_valid_reading_type(reading_type: &str) -> bool {
    SAJU_READING_TYPES.contains(&reading_type)
}

impl SajuEngine {
    pub fn generate(&self, reading_type: &str, input: &Value) -> (Value, String) {
        let version = SAJU_ENGINE_VERSION.to_string();

        match reading_type {
            "daily" => self.generate_daily(input, &version),
            "daily_detail" => self.generate_daily_detail(input, &version),
            "saju" => self.generate_saju(input, &version),
            "saju_wealth" | "saju_love" | "saju_marriage" | "saju_career" | "saju_health"
            | "saju_study" | "saju_children" | "saju_travel" | "saju_relations" => {
                self.generate_natal_category(reading_type, input, &version)
            }
            "weekly" => self.generate_weekly(input, &version),
            "monthly" => self.generate_monthly(input, &version),
            "compatibility" => self.generate_compatibility(input, &version),
            "compatibility_detail" => self.generate_compatibility_detail(input, &version),
            "monthly_fortune" => self.generate_monthly_fortune(input, &version),
            "daeun" => self.generate_daeun(input, &version),
            _ => self.generate_fallback(reading_type, input, &version),
        }
    }

    /// Returns calculation-first natal saju facts without compatibility prose.
    ///
    /// `generate("saju", input)` keeps the historical response shape. New
    /// service layers should prefer this core payload, or the `saju_core`
    /// snapshot embedded in the compatibility response, when building their own
    /// user-facing interpretation.
    pub fn generate_saju_core(&self, input: &Value) -> (Value, String) {
        let version = SAJU_ENGINE_VERSION.to_string();
        let Some(core) = Self::build_saju_core_context(input) else {
            return (
                json!({"error": "사주 분석에는 생년월일시 정보가 필요합니다."}),
                version,
            );
        };

        (saju_core_json(&core), version)
    }
}

fn parse_birth_hour(input: &str) -> Option<(u32, u32)> {
    let mut parts = input.split(':');
    let hour = parts.next()?.parse::<u32>().ok()?;
    if hour > 23 {
        return None;
    }
    let minute = if let Some(m) = parts.next() {
        let m: u32 = m.parse().ok()?;
        if m > 59 {
            return None;
        }
        m
    } else {
        0
    };
    if parts.next().is_some() {
        return None;
    }
    Some((hour, minute))
}

fn normalize_gender(value: Option<&str>) -> Option<&'static str> {
    match value.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "m" | "male" | "man" | "남" | "남성" => Some("male"),
        "f" | "female" | "woman" | "여" | "여성" => Some("female"),
        _ => None,
    }
}

fn relation_adjustment(relation: elements::ElementRelation) -> i32 {
    match relation {
        elements::ElementRelation::Generated => 9,
        elements::ElementRelation::Same => 4,
        elements::ElementRelation::Controls => 2,
        elements::ElementRelation::Generates => -3,
        elements::ElementRelation::Controlled => -8,
    }
}

fn clamp_score(score: i32) -> i32 {
    score.clamp(30, 98)
}

fn pillar_influence_score(pillar: Pillar, today: Pillar) -> i32 {
    let stem_relation = elements::relation(pillar.stem.element(), today.stem.element());
    let branch_relation = elements::relation(pillar.branch.element(), today.branch.element());
    clamp_score(70 + relation_adjustment(stem_relation) + relation_adjustment(branch_relation) / 2)
}

fn pillar_influence_json(position: &str, pillar: Pillar, today: Pillar) -> Value {
    let stem_relation = elements::relation(pillar.stem.element(), today.stem.element());
    let branch_relation = elements::relation(pillar.branch.element(), today.branch.element());
    json!({
        "position": position,
        "pillar": format!("{}", pillar),
        "score": pillar_influence_score(pillar, today),
        "stem_relation": stem_relation.korean(),
        "branch_relation": branch_relation.korean(),
    })
}

fn branch_relation_summary(
    user_pillars: &FourPillars,
    today: Pillar,
    has_birth_time: bool,
) -> Value {
    let mut natal = vec![
        user_pillars.year.branch,
        user_pillars.month.branch,
        user_pillars.day.branch,
    ];
    if has_birth_time {
        natal.push(user_pillars.hour.branch);
    }
    let analysis = branches::analyze(&natal, &[today.branch]);
    let adjustment = (analysis.yukhap_count as i32 * 4) + (analysis.samhap_count as i32 * 3)
        - (analysis.sangchung_count as i32 * 6)
        - (analysis.sanghyeong_count as i32 * 3);

    json!({
        "target_branch": today.branch.korean(),
        "samhap_count": analysis.samhap_count,
        "yukhap_count": analysis.yukhap_count,
        "clash_count": analysis.sangchung_count,
        "punishment_count": analysis.sanghyeong_count,
        "adjustment": adjustment.clamp(-12, 12),
    })
}

fn current_daeun_context(
    user_pillars: &FourPillars,
    birth_year: i32,
    birth_month: u32,
    birth_day: u32,
    birth_hour: u32,
    birth_minute: u32,
    gender: Option<&str>,
) -> (Option<Value>, i32) {
    let Some(gender) = gender else {
        return (None, 0);
    };
    let periods = daeun::calculate_daeun_calculation_with_time(
        user_pillars,
        birth_year,
        birth_month,
        birth_day,
        birth_hour,
        birth_minute,
        gender,
    );
    let Some(period) = periods.iter().find(|p| p.is_current) else {
        return (None, 0);
    };
    let adjustment = ((period.score - 70) / 4).clamp(-10, 8);
    (
        Some(json!({
            "start_age": period.start_age,
            "end_age": period.end_age,
            "pillar": format!("{}{}", period.stem, period.branch),
            "stem": period.stem,
            "branch": period.branch,
            "element": period.element,
            "score": period.score,
            "adjustment": adjustment,
        })),
        adjustment,
    )
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

fn approximate_daeun_start_date(
    birth_year: i32,
    birth_month: u32,
    birth_day: u32,
    start_age: i32,
) -> String {
    let target_year = birth_year + start_age;
    let day = birth_day.min(days_in_month(target_year, birth_month));
    NaiveDate::from_ymd_opt(target_year, birth_month, day)
        .map(|date| date.to_string())
        .unwrap_or_else(|| format!("{target_year:04}-01-01"))
}

struct SajuCoreContext {
    birth_year: i32,
    birth_month: u32,
    birth_day: u32,
    birth_hour: u32,
    birth_minute: u32,
    target_year: i32,
    has_birth_time: bool,
    pillars: FourPillars,
    day_master: Stem,
    balance: ElementBalance,
    ten_gods: Vec<(&'static str, TenGod)>,
    gongmang: gongmang::GongmangFacts,
    shinsal: Vec<shinsal::ShinsalFacts>,
    lucky: lucky::LuckyCoreItems,
    gender: String,
    normalized_gender: Option<&'static str>,
    birth: calendar::NormalizedBirthDate,
}

const ELEMENT_ORDER: [Element; 5] = [
    Element::Wood,
    Element::Fire,
    Element::Earth,
    Element::Metal,
    Element::Water,
];

const TEN_GOD_ORDER: [TenGod; 10] = [
    TenGod::Bigyeon,
    TenGod::Geupjae,
    TenGod::Sikshin,
    TenGod::Sanggwan,
    TenGod::Pyeonjae,
    TenGod::Jeongjae,
    TenGod::Pyeongwan,
    TenGod::Jeonggwan,
    TenGod::Pyeonin,
    TenGod::Jeongin,
];

fn saju_core_json(core: &SajuCoreContext) -> Value {
    let (four_pillars, four_pillars_detail) =
        four_pillars_core_json(&core.pillars, core.has_birth_time);

    json!({
        "schema_version": SAJU_CORE_SCHEMA_VERSION,
        "calculation_profile": profile::calculation_profile_json(),
        "four_pillars": four_pillars,
        "four_pillars_detail": four_pillars_detail,
        "has_birth_time": core.has_birth_time,
        "day_master": day_master_core_json(core.day_master),
        "element_balance": element_balance_core_json(&core.balance),
        "dominant_element": element_summary_json(&core.balance, dominant_element_for(&core.balance)),
        "weakest_element": element_summary_json(&core.balance, weakest_element_for(&core.balance)),
        "ten_gods": ten_gods_positions_json(&core.ten_gods),
        "ten_gods_summary": ten_gods_summary_core_json(&core.ten_gods),
        "gender": core.gender,
        "target_year": core.target_year,
        "calculation_basis": calculation_basis_json(core),
        "daeun_summary": daeun_summary_core_json(core),
        "gongmang": gongmang_core_json(&core.gongmang),
        "shinsal": core.shinsal.iter().map(shinsal_core_json).collect::<Vec<_>>(),
        "lucky": lucky_core_json(&core.lucky),
        "manseoryok": manseoryok_json(core),
        "signals": saju_signals_json(core),
        "evidence": saju_evidence_json(core),
    })
}

fn manseoryok_json(core: &SajuCoreContext) -> Value {
    json!({
        "schema_version": "saju_manseoryok_v1",
        "hidden_stems": hidden_stems_manse_json(core),
        "twelve_stages": twelve_stages_json(core),
        "branch_interactions": branch_interactions_manse_json(core),
        "useful_elements": useful_elements_json(core),
        "gyeokguk": gyeokguk_json(core),
        "johu_eokbu_tonggwan": johu_eokbu_tonggwan_json(core),
        "fortune_cycles": fortune_cycles_manse_json(core),
        "calculation_basis": {
            "calculation_profile_id": profile::SAJU_PROFILE_ID,
            "calculation_profile_version": profile::SAJU_PROFILE_VERSION,
            "hidden_stem_rule": "branch藏干_primary_secondary_residual_table_v1",
            "twelve_stage_rule": "day_stem_twelve_growth_stage_table_v1",
            "useful_element_rule": "strength_score_plus_seasonal_adjustment_v1",
            "gyeokguk_rule": "month_branch_primary_hidden_stem_ten_god_v1",
            "cycle_rule": "daeun_plus_next_10_annual_12_monthly_10_daily_flows",
            "precision_note": "solar_term_boundary_uses_engine_precise_pillar_path_and daeun adjacent solar-term days when available",
        }
    })
}

fn hidden_stems_manse_json(core: &SajuCoreContext) -> Vec<Value> {
    pillar_entries(&core.pillars, core.has_birth_time)
        .into_iter()
        .map(|(position, pillar)| {
            let stems = hidden_stems_for_branch(pillar.branch);
            json!({
                "position": position,
                "branch": branch_info(pillar.branch),
                "stems": stems.iter().enumerate().map(|(idx, stem)| {
                    json!({
                        "stem": day_master_info(*stem),
                        "priority": if idx == 0 { "primary" } else if idx == 1 { "secondary" } else { "residual" },
                        "ten_god": ten_gods::derive_ten_god(core.day_master, *stem).korean(),
                    })
                }).collect::<Vec<_>>(),
            })
        })
        .collect()
}

fn twelve_stages_json(core: &SajuCoreContext) -> Vec<Value> {
    pillar_entries(&core.pillars, core.has_birth_time)
        .into_iter()
        .map(|(position, pillar)| {
            let stage = twelve_stage(core.day_master, pillar.branch);
            json!({
                "position": position,
                "branch": branch_info(pillar.branch),
                "stage": stage,
            })
        })
        .collect()
}

fn branch_interactions_manse_json(core: &SajuCoreContext) -> Value {
    let branches = pillar_entries(&core.pillars, core.has_birth_time)
        .into_iter()
        .map(|(_, pillar)| pillar.branch)
        .collect::<Vec<_>>();
    let analysis = branches::analyze(&branches, &[]);
    json!({
        "samhap": analysis.samhap.iter().map(|item| match item {
            branches::SamhapResult::Full(element) => json!({"type": "full", "element": element.korean()}),
            branches::SamhapResult::Half(element) => json!({"type": "half", "element": element.korean()}),
        }).collect::<Vec<_>>(),
        "yukhap_count": analysis.yukhap_count,
        "clash_count": analysis.sangchung_count,
        "punishment_count": analysis.sanghyeong_count,
        "harms": branch_harms(&branches),
        "breaks": branch_breaks(&branches),
    })
}

fn useful_elements_json(core: &SajuCoreContext) -> Value {
    let strength = strength_score(core);
    let day_element = core.day_master.element();
    let season_element = core.pillars.month.branch.element();
    let useful = if strength >= 62 {
        vec![generates(day_element), controls(day_element)]
    } else if strength <= 42 {
        vec![generating(day_element), day_element]
    } else {
        vec![
            weakest_element_for(&core.balance),
            season_balancer(season_element),
        ]
    };
    let avoid = if strength >= 62 {
        vec![day_element, generating(day_element)]
    } else if strength <= 42 {
        vec![controls(day_element), generates(day_element)]
    } else {
        vec![dominant_element_for(&core.balance)]
    };
    json!({
        "strength_score": strength,
        "strength_label": if strength >= 62 { "신강" } else if strength <= 42 { "신약" } else { "중화" },
        "yongsin_candidates": useful.iter().map(|element| element.korean()).collect::<Vec<_>>(),
        "gisin_candidates": avoid.iter().map(|element| element.korean()).collect::<Vec<_>>(),
        "huisin_candidates": vec![weakest_element_for(&core.balance).korean()],
    })
}

fn gyeokguk_json(core: &SajuCoreContext) -> Value {
    let primary = hidden_stems_for_branch(core.pillars.month.branch)
        .first()
        .copied()
        .unwrap_or(core.pillars.month.stem);
    let ten_god = ten_gods::derive_ten_god(core.day_master, primary);
    json!({
        "basis": "month_branch_primary_hidden_stem",
        "month_branch": branch_info(core.pillars.month.branch),
        "primary_hidden_stem": day_master_info(primary),
        "ten_god": ten_god.korean(),
        "label": gyeok_label(ten_god),
    })
}

fn johu_eokbu_tonggwan_json(core: &SajuCoreContext) -> Value {
    let month = core.pillars.month.branch;
    let strength = strength_score(core);
    let dominant = dominant_element_for(&core.balance);
    let weakest = weakest_element_for(&core.balance);
    let cold_hot = match month {
        Branch::Hae | Branch::Ja | Branch::Chuk => "cold",
        Branch::Sa | Branch::O | Branch::Mi => "hot",
        _ => "moderate",
    };
    let johu = match cold_hot {
        "cold" => Element::Fire,
        "hot" => Element::Water,
        _ => weakest_element_for(&core.balance),
    };
    json!({
        "johu": {"season_temperature": cold_hot, "candidate": johu.korean()},
        "strength_score": strength,
        "dominant_element": dominant.korean(),
        "weakest_element": weakest.korean(),
    })
}

fn fortune_cycles_manse_json(core: &SajuCoreContext) -> Value {
    let daeun = core
        .normalized_gender
        .map(|gender| {
            daeun::calculate_daeun_calculation_with_time(
                &core.pillars,
                core.birth_year,
                core.birth_month,
                core.birth_day,
                core.birth_hour,
                core.birth_minute,
                gender,
            )
            .into_iter()
            .map(|period| daeun_period_core_json(&period, core.day_master))
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let current_year = core.target_year;
    let annual = (0..10)
        .map(|offset| annual_flow_json(core, current_year + offset))
        .collect::<Vec<_>>();
    let monthly = monthly::calculate_monthly_calculation(&core.pillars, current_year)
        .into_iter()
        .map(|month| {
            json!({
                "month": month.month,
                "score": month.score,
                "grade": month.grade,
                "categories": {
                    "overall": month.categories.overall,
                    "love": month.categories.love,
                    "career": month.categories.career,
                    "health": month.categories.health,
                    "wealth": month.categories.wealth,
                },
            })
        })
        .collect::<Vec<_>>();
    let daily = (0..10)
        .map(|offset| {
            let date = NaiveDate::from_ymd_opt(core.target_year, 1, 1).unwrap()
                + chrono::Duration::days(offset);
            let fortune = daily::calculate_daily_calculation_for_date(
                &core.pillars,
                date.year(),
                date.month(),
                date.day(),
            );
            json!({
                "date": date.to_string(),
                "pillar": format!("{}", fortune.today_pillar),
                "score": fortune.scores.overall,
                "relation": fortune.relation.korean(),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "daeun": daeun,
        "annual": annual,
        "monthly": monthly,
        "daily": daily,
    })
}

fn pillar_entries(pillars: &FourPillars, has_birth_time: bool) -> Vec<(&'static str, Pillar)> {
    let mut entries = vec![
        ("year", pillars.year),
        ("month", pillars.month),
        ("day", pillars.day),
    ];
    if has_birth_time {
        entries.push(("hour", pillars.hour));
    }
    entries
}

fn hidden_stems_for_branch(branch: Branch) -> &'static [Stem] {
    match branch {
        Branch::Ja => &[Stem::Gye],
        Branch::Chuk => &[Stem::Gi, Stem::Gye, Stem::Sin],
        Branch::In => &[Stem::Gap, Stem::Byeong, Stem::Mu],
        Branch::Myo => &[Stem::Eul],
        Branch::Jin => &[Stem::Mu, Stem::Eul, Stem::Gye],
        Branch::Sa => &[Stem::Byeong, Stem::Gyeong, Stem::Mu],
        Branch::O => &[Stem::Jeong, Stem::Gi],
        Branch::Mi => &[Stem::Gi, Stem::Jeong, Stem::Eul],
        Branch::Sin => &[Stem::Gyeong, Stem::Im, Stem::Mu],
        Branch::Yu => &[Stem::Sin],
        Branch::Sul => &[Stem::Mu, Stem::Sin, Stem::Jeong],
        Branch::Hae => &[Stem::Im, Stem::Gap],
    }
}

fn twelve_stage(day_master: Stem, branch: Branch) -> &'static str {
    const STAGES: [&str; 12] = [
        "장생", "목욕", "관대", "건록", "제왕", "쇠", "병", "사", "묘", "절", "태", "양",
    ];
    let (start, forward) = match day_master {
        Stem::Gap => (Branch::Hae, true),
        Stem::Eul => (Branch::O, false),
        Stem::Byeong | Stem::Mu => (Branch::In, true),
        Stem::Jeong | Stem::Gi => (Branch::Yu, false),
        Stem::Gyeong => (Branch::Sa, true),
        Stem::Sin => (Branch::Ja, false),
        Stem::Im => (Branch::Sin, true),
        Stem::Gye => (Branch::Myo, false),
    };
    let offset = if forward {
        (branch.index() + 12 - start.index()) % 12
    } else {
        (start.index() + 12 - branch.index()) % 12
    };
    STAGES[offset]
}

fn branch_harms(branches: &[Branch]) -> Vec<Value> {
    const HARMS: [(Branch, Branch); 6] = [
        (Branch::Ja, Branch::Mi),
        (Branch::Chuk, Branch::O),
        (Branch::In, Branch::Sa),
        (Branch::Myo, Branch::Jin),
        (Branch::Sin, Branch::Hae),
        (Branch::Yu, Branch::Sul),
    ];
    branch_pair_hits(branches, &HARMS, "harm")
}

fn branch_breaks(branches: &[Branch]) -> Vec<Value> {
    const BREAKS: [(Branch, Branch); 6] = [
        (Branch::Ja, Branch::Yu),
        (Branch::Chuk, Branch::Jin),
        (Branch::In, Branch::Hae),
        (Branch::Myo, Branch::O),
        (Branch::Sa, Branch::Sin),
        (Branch::Mi, Branch::Sul),
    ];
    branch_pair_hits(branches, &BREAKS, "break")
}

fn branch_pair_hits(branches: &[Branch], pairs: &[(Branch, Branch)], kind: &str) -> Vec<Value> {
    pairs
        .iter()
        .filter(|(a, b)| branches.contains(a) && branches.contains(b))
        .map(|(a, b)| json!({"type": kind, "branches": [a.korean(), b.korean()]}))
        .collect()
}

fn strength_score(core: &SajuCoreContext) -> i32 {
    let day = core.day_master.element();
    let same = element_count(&core.balance, day) as i32;
    let resource = element_count(&core.balance, generating(day)) as i32;
    let season = if core.pillars.month.branch.element() == day {
        12
    } else {
        0
    };
    (35 + same * 8 + resource * 6 + season).clamp(20, 85)
}

fn generating(element: Element) -> Element {
    match element {
        Element::Wood => Element::Water,
        Element::Fire => Element::Wood,
        Element::Earth => Element::Fire,
        Element::Metal => Element::Earth,
        Element::Water => Element::Metal,
    }
}

fn generates(element: Element) -> Element {
    match element {
        Element::Wood => Element::Fire,
        Element::Fire => Element::Earth,
        Element::Earth => Element::Metal,
        Element::Metal => Element::Water,
        Element::Water => Element::Wood,
    }
}

fn controls(element: Element) -> Element {
    match element {
        Element::Wood => Element::Earth,
        Element::Fire => Element::Metal,
        Element::Earth => Element::Water,
        Element::Metal => Element::Wood,
        Element::Water => Element::Fire,
    }
}

fn season_balancer(element: Element) -> Element {
    match element {
        Element::Water => Element::Fire,
        Element::Fire => Element::Water,
        Element::Wood => Element::Metal,
        Element::Metal => Element::Fire,
        Element::Earth => Element::Wood,
    }
}

fn gyeok_label(ten_god: TenGod) -> &'static str {
    match ten_god {
        TenGod::Bigyeon => "건록/비견격 후보",
        TenGod::Geupjae => "양인/겁재격 후보",
        TenGod::Sikshin => "식신격 후보",
        TenGod::Sanggwan => "상관격 후보",
        TenGod::Pyeonjae => "편재격 후보",
        TenGod::Jeongjae => "정재격 후보",
        TenGod::Pyeongwan => "편관격 후보",
        TenGod::Jeonggwan => "정관격 후보",
        TenGod::Pyeonin => "편인격 후보",
        TenGod::Jeongin => "정인격 후보",
    }
}

fn annual_flow_json(core: &SajuCoreContext, year: i32) -> Value {
    let year_pillar = pillars::year_pillar(year, 7, 1);
    let relation = ten_gods::derive_ten_god(core.day_master, year_pillar.stem);
    let branch_analysis = branches::analyze(
        &pillar_entries(&core.pillars, core.has_birth_time)
            .into_iter()
            .map(|(_, pillar)| pillar.branch)
            .collect::<Vec<_>>(),
        &[year_pillar.branch],
    );
    json!({
        "year": year,
        "pillar": format!("{}", year_pillar),
        "stem_ten_god": relation.korean(),
        "branch": branch_info(year_pillar.branch),
        "interactions": {
            "samhap_count": branch_analysis.samhap_count,
            "yukhap_count": branch_analysis.yukhap_count,
            "clash_count": branch_analysis.sangchung_count,
            "punishment_count": branch_analysis.sanghyeong_count,
        }
    })
}

fn four_pillars_core_json(pillars: &FourPillars, has_birth_time: bool) -> (Value, Value) {
    let mut four_pillars = json!({
        "year": format!("{}", pillars.year),
        "month": format!("{}", pillars.month),
        "day": format!("{}", pillars.day),
    });
    let mut four_pillars_detail = json!({
        "year": {"stem": day_master_info(pillars.year.stem), "branch": branch_info(pillars.year.branch)},
        "month": {"stem": day_master_info(pillars.month.stem), "branch": branch_info(pillars.month.branch)},
        "day": {"stem": day_master_info(pillars.day.stem), "branch": branch_info(pillars.day.branch)},
    });

    if has_birth_time {
        four_pillars
            .as_object_mut()
            .unwrap()
            .insert("hour".into(), json!(format!("{}", pillars.hour)));
        four_pillars_detail.as_object_mut().unwrap().insert(
            "hour".into(),
            json!({"stem": day_master_info(pillars.hour.stem), "branch": branch_info(pillars.hour.branch)}),
        );
    }

    (four_pillars, four_pillars_detail)
}

fn calculation_basis_json(core: &SajuCoreContext) -> Value {
    json!({
        "calculation_profile_id": profile::SAJU_PROFILE_ID,
        "calculation_profile_version": profile::SAJU_PROFILE_VERSION,
        "calendar_type": core.birth.calendar_type(),
        "normalized_birth_date": core.birth.solar_date_string(),
        "is_lunar_converted": core.birth.was_converted(),
        "is_lunar_leap_month": core.birth.is_lunar_leap_month,
        "birth_time_status": if core.has_birth_time { "known" } else { "unknown" },
        "timezone": "KST",
    })
}

fn day_master_core_json(day_master: Stem) -> Value {
    json!({
        "stem": day_master.korean(),
        "hanja": day_master.hanja(),
        "element": day_master.element().korean(),
        "polarity": day_master.polarity().korean(),
    })
}

fn element_balance_core_json(balance: &ElementBalance) -> Value {
    json!({
        "wood": balance.wood,
        "fire": balance.fire,
        "earth": balance.earth,
        "metal": balance.metal,
        "water": balance.water,
        "total_count": balance.wood + balance.fire + balance.earth + balance.metal + balance.water,
        "counts": ELEMENT_ORDER
            .iter()
            .map(|element| element_summary_json(balance, *element))
            .collect::<Vec<_>>(),
        "dominant_element": element_summary_json(balance, dominant_element_for(balance)),
        "weakest_element": element_summary_json(balance, weakest_element_for(balance)),
    })
}

fn element_key(element: Element) -> &'static str {
    match element {
        Element::Wood => "wood",
        Element::Fire => "fire",
        Element::Earth => "earth",
        Element::Metal => "metal",
        Element::Water => "water",
    }
}

fn element_count(balance: &ElementBalance, element: Element) -> u8 {
    match element {
        Element::Wood => balance.wood,
        Element::Fire => balance.fire,
        Element::Earth => balance.earth,
        Element::Metal => balance.metal,
        Element::Water => balance.water,
    }
}

fn element_summary_json(balance: &ElementBalance, element: Element) -> Value {
    json!({
        "key": element_key(element),
        "element": element.korean(),
        "count": element_count(balance, element),
    })
}

fn dominant_element_for(balance: &ElementBalance) -> Element {
    let mut best = Element::Wood;
    let mut best_count = element_count(balance, best);
    for element in ELEMENT_ORDER.into_iter().skip(1) {
        let count = element_count(balance, element);
        if count > best_count {
            best = element;
            best_count = count;
        }
    }
    best
}

fn weakest_element_for(balance: &ElementBalance) -> Element {
    let mut weakest = Element::Wood;
    let mut weakest_count = element_count(balance, weakest);
    for element in ELEMENT_ORDER.into_iter().skip(1) {
        let count = element_count(balance, element);
        if count < weakest_count {
            weakest = element;
            weakest_count = count;
        }
    }
    weakest
}

fn ten_gods_positions_json(gods: &[(&'static str, TenGod)]) -> Vec<Value> {
    gods.iter()
        .map(|(position, god)| json!({"position": position, "god": god.korean()}))
        .collect()
}

fn ten_god_count(gods: &[(&'static str, TenGod)], target: TenGod) -> usize {
    gods.iter().filter(|(_, god)| *god == target).count()
}

fn strongest_ten_god(gods: &[(&'static str, TenGod)]) -> Option<(TenGod, usize)> {
    let mut strongest = None;
    for god in TEN_GOD_ORDER {
        let count = ten_god_count(gods, god);
        if count == 0 {
            continue;
        }
        if strongest
            .map(|(_, strongest_count)| count > strongest_count)
            .unwrap_or(true)
        {
            strongest = Some((god, count));
        }
    }
    strongest
}

fn ten_gods_summary_core_json(gods: &[(&'static str, TenGod)]) -> Value {
    let counts = TEN_GOD_ORDER
        .iter()
        .map(|god| json!({"god": god.korean(), "count": ten_god_count(gods, *god)}))
        .collect::<Vec<_>>();
    let prominent = TEN_GOD_ORDER
        .iter()
        .filter_map(|god| {
            let count = ten_god_count(gods, *god);
            (count > 0).then(|| json!({"god": god.korean(), "count": count}))
        })
        .collect::<Vec<_>>();
    let missing = TEN_GOD_ORDER
        .iter()
        .filter(|god| ten_god_count(gods, **god) == 0)
        .map(|god| json!({"god": god.korean()}))
        .collect::<Vec<_>>();
    let strongest =
        strongest_ten_god(gods).map(|(god, count)| json!({"god": god.korean(), "count": count}));

    json!({
        "source": "heavenly_stems",
        "includes_day_master_self": true,
        "positions": ten_gods_positions_json(gods),
        "counts": counts,
        "prominent": prominent,
        "missing": missing,
        "strongest": strongest,
    })
}

fn daeun_period_core_json(period: &daeun::DaeunCalculation, day_master: Stem) -> Value {
    let ten_god =
        stem_from_korean(&period.stem).map(|stem| ten_gods::derive_ten_god(day_master, stem));
    json!({
        "start_age": period.start_age,
        "end_age": period.end_age,
        "pillar": format!("{}{}", period.stem, period.branch),
        "stem": period.stem,
        "branch": period.branch,
        "element": period.element,
        "ten_god": ten_god.map(|god| god.korean()),
        "score": period.score,
        "is_current": period.is_current,
    })
}

fn daeun_summary_core_json(core: &SajuCoreContext) -> Value {
    let Some(gender) = core.normalized_gender else {
        return json!({
            "available": false,
            "missing_inputs": ["gender"],
        });
    };
    let periods = daeun::calculate_daeun_calculation_with_time(
        &core.pillars,
        core.birth_year,
        core.birth_month,
        core.birth_day,
        core.birth_hour,
        core.birth_minute,
        gender,
    );
    let current_index = periods.iter().position(|p| p.is_current);
    let current = current_index.and_then(|idx| periods.get(idx));
    let next = current_index.and_then(|idx| periods.get(idx + 1));
    let start_age = periods.first().map(|p| p.start_age);

    json!({
        "available": true,
        "start_age": start_age,
        "daeun_start": start_age.map(|age| json!({
            "age": age,
            "approximate_start_year": core.birth_year + age,
            "approximate_start_date": approximate_daeun_start_date(core.birth_year, core.birth_month, core.birth_day, age),
        })),
        "current_period_index": current_index,
        "current": current.map(|p| daeun_period_core_json(p, core.day_master)),
        "next": next.map(|p| daeun_period_core_json(p, core.day_master)),
        "periods": periods.iter().map(|p| daeun_period_core_json(p, core.day_master)).collect::<Vec<_>>(),
    })
}

fn gongmang_palace_key(palace: gongmang::Palace) -> &'static str {
    match palace {
        gongmang::Palace::Year => "year",
        gongmang::Palace::Month => "month",
        gongmang::Palace::Hour => "hour",
    }
}

fn gongmang_core_json(g: &gongmang::GongmangFacts) -> Value {
    json!({
        "group_index": g.group_index,
        "group_name": g.group_name,
        "empty_branches": g.empty_branches.iter().map(|b| branch_info(*b)).collect::<Vec<_>>(),
        "affected_palaces": g.affected_palaces.iter().map(|p| gongmang_palace_key(*p)).collect::<Vec<_>>(),
        "affected_ten_gods": g.affected_ten_gods.iter().map(|t| t.korean()).collect::<Vec<_>>(),
    })
}

fn shinsal_kind_labels(kind: shinsal::ShinsalKind) -> (&'static str, &'static str) {
    match kind {
        shinsal::ShinsalKind::Geop => ("geop", "겁살"),
        shinsal::ShinsalKind::Jae => ("jae", "재살"),
        shinsal::ShinsalKind::Cheon => ("cheon", "천살"),
        shinsal::ShinsalKind::Ji => ("ji", "지살"),
        shinsal::ShinsalKind::Dohwa => ("dohwa", "도화살"),
        shinsal::ShinsalKind::Wol => ("wol", "월살"),
        shinsal::ShinsalKind::Mangsin => ("mangsin", "망신살"),
        shinsal::ShinsalKind::Jangseong => ("jangseong", "장성살"),
        shinsal::ShinsalKind::Banan => ("banan", "반안살"),
        shinsal::ShinsalKind::Yeokma => ("yeokma", "역마살"),
        shinsal::ShinsalKind::Yukae => ("yukae", "육해살"),
        shinsal::ShinsalKind::Hwagae => ("hwagae", "화개살"),
        shinsal::ShinsalKind::Baekho => ("baekho", "백호살"),
        shinsal::ShinsalKind::Cheoneul => ("cheoneul", "천을귀인"),
    }
}

fn shinsal_position_key(position: shinsal::ShinsalPosition) -> &'static str {
    match position {
        shinsal::ShinsalPosition::Year => "year",
        shinsal::ShinsalPosition::Month => "month",
        shinsal::ShinsalPosition::Day => "day",
        shinsal::ShinsalPosition::Hour => "hour",
    }
}

fn shinsal_core_json(s: &shinsal::ShinsalFacts) -> Value {
    let (kind_slug, kind_korean) = shinsal_kind_labels(s.kind);
    json!({
        "kind": kind_slug,
        "kind_korean": kind_korean,
        "positions": s.positions.iter().map(|p| shinsal_position_key(*p)).collect::<Vec<_>>(),
        "intensity": s.intensity,
    })
}

fn lucky_core_json(l: &lucky::LuckyCoreItems) -> Value {
    json!({
        "primary": lucky_triple_to_json(&l.primary),
        "supplementary": lucky_triple_to_json(&l.supplementary),
    })
}

fn saju_signals_json(core: &SajuCoreContext) -> Vec<Value> {
    let dominant = dominant_element_for(&core.balance);
    let weakest = weakest_element_for(&core.balance);
    let mut signals = vec![
        json!({
            "kind": "day_master",
            "stem": core.day_master.korean(),
            "element": core.day_master.element().korean(),
            "polarity": core.day_master.polarity().korean(),
            "evidence": ["four_pillars.day.stem"],
        }),
        json!({
            "kind": "dominant_element",
            "element": dominant.korean(),
            "count": element_count(&core.balance, dominant),
            "evidence": ["element_balance.counts"],
        }),
        json!({
            "kind": "weakest_element",
            "element": weakest.korean(),
            "count": element_count(&core.balance, weakest),
            "evidence": ["element_balance.counts"],
        }),
    ];

    if let Some((god, count)) = strongest_ten_god(&core.ten_gods) {
        signals.push(json!({
            "kind": "strongest_ten_god",
            "god": god.korean(),
            "count": count,
            "evidence": ["ten_gods_summary.counts"],
        }));
    }

    signals.push(json!({
        "kind": "gongmang",
        "group_index": core.gongmang.group_index,
        "empty_branches": core.gongmang.empty_branches.iter().map(|b| b.korean()).collect::<Vec<_>>(),
        "affected_palaces": core.gongmang.affected_palaces.iter().map(|p| gongmang_palace_key(*p)).collect::<Vec<_>>(),
        "evidence": ["four_pillars.day", "gongmang.empty_branches"],
    }));

    signals
}

fn saju_evidence_json(core: &SajuCoreContext) -> Vec<Value> {
    let mut evidence = vec![
        json!({
            "kind": "birth",
            "calendar_type": core.birth.calendar_type(),
            "normalized_birth_date": core.birth.solar_date_string(),
            "birth_time_status": if core.has_birth_time { "known" } else { "unknown" },
        }),
        pillar_evidence_json("year", core.pillars.year),
        pillar_evidence_json("month", core.pillars.month),
        pillar_evidence_json("day", core.pillars.day),
    ];

    if core.has_birth_time {
        evidence.push(pillar_evidence_json("hour", core.pillars.hour));
    }

    for element in ELEMENT_ORDER {
        evidence.push(json!({
            "kind": "element_count",
            "element": element.korean(),
            "key": element_key(element),
            "count": element_count(&core.balance, element),
        }));
    }

    for (position, god) in &core.ten_gods {
        evidence.push(json!({
            "kind": "ten_god",
            "position": position,
            "god": god.korean(),
        }));
    }

    evidence
}

fn pillar_evidence_json(position: &str, pillar: Pillar) -> Value {
    json!({
        "kind": "pillar",
        "position": position,
        "pillar": format!("{}", pillar),
        "stem": day_master_info(pillar.stem),
        "branch": branch_info(pillar.branch),
    })
}

fn daily_v2_context(
    user_pillars: &FourPillars,
    today: Pillar,
    has_birth_time: bool,
    birth_year: i32,
    birth_month: u32,
    birth_day: u32,
    birth_hour: u32,
    birth_minute: u32,
    gender: Option<&str>,
) -> (Value, Option<Value>, i32, i32) {
    let mut weighted_total = 0_i32;
    let mut weight_sum = 0_i32;
    let positions = [
        ("year", user_pillars.year, 20),
        ("month", user_pillars.month, 25),
        ("day", user_pillars.day, 40),
    ];
    let mut pillars_json = Vec::new();
    for (position, pillar, weight) in positions {
        let score = pillar_influence_score(pillar, today);
        weighted_total += score * weight;
        weight_sum += weight;
        pillars_json.push(pillar_influence_json(position, pillar, today));
    }
    if has_birth_time {
        let score = pillar_influence_score(user_pillars.hour, today);
        weighted_total += score * 15;
        weight_sum += 15;
        pillars_json.push(pillar_influence_json("hour", user_pillars.hour, today));
    }

    let natal_score = if weight_sum == 0 {
        70
    } else {
        weighted_total / weight_sum
    };
    let branch_summary = branch_relation_summary(user_pillars, today, has_birth_time);
    let branch_adjustment = branch_summary
        .get("adjustment")
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;
    let (daeun_json, daeun_adjustment) = current_daeun_context(
        user_pillars,
        birth_year,
        birth_month,
        birth_day,
        birth_hour,
        birth_minute,
        gender,
    );
    let total_adjustment =
        ((natal_score - 70) / 3 + branch_adjustment + daeun_adjustment).clamp(-18, 18);

    (
        json!({
            "today_pillar": format!("{}", today),
            "natal_score": natal_score,
            "pillars": pillars_json,
            "branch_relations": branch_summary,
            "daeun_adjustment": daeun_adjustment,
            "total_adjustment": total_adjustment,
        }),
        daeun_json,
        total_adjustment,
        daeun_adjustment,
    )
}

impl SajuEngine {
    fn is_lunar_leap_month(input: &Value) -> bool {
        input
            .get("is_lunar_leap_month")
            .or_else(|| input.get("lunar_leap_month"))
            .or_else(|| {
                input
                    .get("options")
                    .and_then(|o| o.get("is_lunar_leap_month"))
            })
            .or_else(|| input.get("options").and_then(|o| o.get("lunar_leap_month")))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    fn parse_normalized_birth_date(input: &Value) -> Option<calendar::NormalizedBirthDate> {
        let birth_date = input.get("birth_date").and_then(|v| v.as_str())?;
        let parts: Vec<&str> = birth_date.split('-').collect();
        if parts.len() != 3 {
            return None;
        }

        let year: i32 = parts[0].parse().ok()?;
        let month: u32 = parts[1].parse().ok()?;
        let day: u32 = parts[2].parse().ok()?;
        let calendar_type = input.get("calendar_type").and_then(|v| v.as_str());
        calendar::normalize_birth_date(
            year,
            month,
            day,
            calendar_type,
            Self::is_lunar_leap_month(input),
        )
    }

    /// birth_date ("YYYY-MM-DD"), birth_time ("HH:MM" or "HH") 파싱
    /// 반환: (year, month, day, hour, minute, has_birth_time)
    fn parse_birth_data(input: &Value) -> Option<(i32, u32, u32, u32, u32, bool)> {
        let birth = Self::parse_normalized_birth_date(input)?;
        NaiveDate::from_ymd_opt(birth.solar_year, birth.solar_month, birth.solar_day)?;

        let parsed = match input
            .get("birth_time")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(t) => Some(parse_birth_hour(t)?),
            None => None,
        };

        let has_birth_time = parsed.is_some();
        // 시간 미상이면 오시(정오) 기본값
        let (hour, minute) = parsed.unwrap_or((12, 0));

        Some((
            birth.solar_year,
            birth.solar_month,
            birth.solar_day,
            hour,
            minute,
            has_birth_time,
        ))
    }

    fn build_saju_core_context(input: &Value) -> Option<SajuCoreContext> {
        let (year, month, day, hour, minute, has_birth_time) = Self::parse_birth_data(input)?;
        let pillars = saju::calculate_four_pillars_precise(year, month, day, hour, minute);
        let day_master = pillars.day.stem;
        let balance = ElementBalance::from_pillars_with_hour(&pillars, has_birth_time);
        let ten_gods = ten_gods::analyze_ten_gods(&pillars, has_birth_time);
        let gongmang = gongmang::calculate(&pillars, has_birth_time);
        let shinsal = shinsal::calculate(&pillars, has_birth_time);
        let lucky = lucky::calculate(&pillars, has_birth_time);
        let gender = input
            .get("gender")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let normalized_gender = normalize_gender(input.get("gender").and_then(|v| v.as_str()));
        let birth = Self::parse_normalized_birth_date(input)?;
        let target_year = input
            .get("target_year")
            .or_else(|| {
                input
                    .get("options")
                    .and_then(|options| options.get("target_year"))
            })
            .and_then(|value| value.as_i64())
            .and_then(|value| i32::try_from(value).ok())
            .unwrap_or(year);

        Some(SajuCoreContext {
            birth_year: year,
            birth_month: month,
            birth_day: day,
            birth_hour: hour,
            birth_minute: minute,
            target_year,
            has_birth_time,
            pillars,
            day_master,
            balance,
            ten_gods,
            gongmang,
            shinsal,
            lucky,
            gender,
            normalized_gender,
            birth,
        })
    }

    fn generate_daily(&self, input: &Value, version: &str) -> (Value, String) {
        let Some((year, month, day, hour, minute, _)) = Self::parse_birth_data(input) else {
            return (
                json!({
                    "error": "생년월일 정보가 필요합니다"
                }),
                version.to_string(),
            );
        };

        let user_pillars = saju::calculate_four_pillars_precise(year, month, day, hour, minute);
        let fortune = daily::calculate_daily_calculation(&user_pillars);

        let result = json!({
            "date": fortune.date,
            "today_pillar": format!("{}", fortune.today_pillar),
            "day_master": format!("{} {}", fortune.day_master.korean(), fortune.day_master.element().korean()),
            "relation": fortune.relation.korean(),
            "scores": {
                "overall": fortune.scores.overall,
                "love": fortune.scores.love,
                "career": fortune.scores.career,
                "health": fortune.scores.health,
            },
        });

        (result, version.to_string())
    }

    fn generate_daily_detail(&self, input: &Value, version: &str) -> (Value, String) {
        let Some((year, month, day, hour, minute, has_birth_time)) = Self::parse_birth_data(input)
        else {
            return (
                json!({
                    "error": "생년월일 정보가 필요합니다"
                }),
                version.to_string(),
            );
        };

        let user_pillars = saju::calculate_four_pillars_precise(year, month, day, hour, minute);
        let detail = daily::calculate_daily_detail_calculation(&user_pillars);
        let birth = Self::parse_normalized_birth_date(input).expect("birth data parsed above");
        let gender = normalize_gender(input.get("gender").and_then(|v| v.as_str()));
        let (daily_influences, current_daeun, score_delta, _daeun_delta) = daily_v2_context(
            &user_pillars,
            detail.base.today_pillar,
            has_birth_time,
            year,
            month,
            day,
            hour,
            minute,
            gender,
        );
        let mut missing_inputs = Vec::new();
        if !has_birth_time {
            missing_inputs.push("birth_time");
        }
        if gender.is_none() {
            missing_inputs.push("gender");
        }

        let overall_score = clamp_score(detail.base.scores.overall + score_delta);
        let love_score = clamp_score(detail.base.scores.love + score_delta);
        let career_score = clamp_score(detail.base.scores.career + score_delta);
        let health_score = clamp_score(detail.base.scores.health + score_delta / 2);
        let wealth_score = clamp_score(detail.category_scores.wealth + score_delta);
        let study_score = clamp_score(detail.category_scores.study + score_delta / 2);
        let travel_score = clamp_score(detail.category_scores.travel + score_delta / 2);
        let relations_score = clamp_score(detail.category_scores.relations + score_delta);

        let lk = lucky::calculate(&user_pillars, has_birth_time);

        let hourly: Vec<Value> = detail
            .hourly_scores
            .iter()
            .map(|h| {
                json!({
                    "hour_name": h.hour_name,
                    "hour_range": h.hour_range,
                    "score": h.score,
                })
            })
            .collect();

        let result = json!({
            "date": detail.base.date,
            "today_pillar": format!("{}", detail.base.today_pillar),
            "day_master": format!("{} {}", detail.base.day_master.korean(), detail.base.day_master.element().korean()),
            "relation": detail.base.relation.korean(),
            "scores": {
                "overall": overall_score,
                "love": love_score,
                "career": career_score,
                "health": health_score,
                "wealth": wealth_score,
            },
            "precision": {
                "level": if has_birth_time && gender.is_some() { "full" } else { "partial" },
                "birth_time_status": if has_birth_time { "known" } else { "unknown" },
                "daeun_status": if gender.is_some() { "included" } else { "omitted" },
                "calendar_type": birth.calendar_type(),
                "normalized_birth_date": birth.solar_date_string(),
                "is_lunar_converted": birth.was_converted(),
                "is_lunar_leap_month": birth.is_lunar_leap_month,
            },
            "missing_inputs": missing_inputs,
            "daily_influences": daily_influences,
            "current_daeun": current_daeun,
            "category_details": {
                "love": { "score": love_score },
                "career": { "score": career_score },
                "health": { "score": health_score },
                "wealth": { "score": wealth_score },
                "study": { "score": study_score },
                "travel": { "score": travel_score },
                "relations": { "score": relations_score },
            },
            "hourly_fortunes": hourly,
            "lucky_items": {
                "color": detail.lucky_items.color,
                "color_hex": detail.lucky_items.color_hex,
                "number": detail.lucky_items.number,
                "direction": detail.lucky_items.direction,
            },
            "lucky": lucky_core_json(&lk),
        });

        (result, version.to_string())
    }

    fn generate_saju(&self, input: &Value, version: &str) -> (Value, String) {
        let Some(core) = Self::build_saju_core_context(input) else {
            return (
                json!({"error": "사주 분석에는 생년월일시 정보가 필요합니다."}),
                version.to_string(),
            );
        };

        (saju_core_json(&core), version.to_string())
    }

    fn generate_natal_category(
        &self,
        reading_type: &str,
        input: &Value,
        version: &str,
    ) -> (Value, String) {
        let Some(core) = Self::build_saju_core_context(input) else {
            return (
                json!({"error": "사주 분석에는 생년월일시 정보가 필요합니다."}),
                version.to_string(),
            );
        };

        let mut result = saju_core_json(&core);
        if let Some(object) = result.as_object_mut() {
            object.insert("reading_type".into(), json!(reading_type));
            object.insert(
                "domain".into(),
                json!(reading_type.strip_prefix("saju_").unwrap_or(reading_type)),
            );
        }

        (result, version.to_string())
    }

    fn generate_weekly(&self, input: &Value, version: &str) -> (Value, String) {
        let Some((year, month, day, hour, minute, _)) = Self::parse_birth_data(input) else {
            return (
                json!({"error": "생년월일 정보가 필요합니다"}),
                version.to_string(),
            );
        };

        let user_pillars = saju::calculate_four_pillars_precise(year, month, day, hour, minute);
        let kst = chrono::FixedOffset::east_opt(9 * 3600).unwrap();
        let today = chrono::Utc::now().with_timezone(&kst).date_naive();

        let days: Vec<Value> = (0..7)
            .map(|offset| {
                let date = today + chrono::Duration::days(offset);
                let fortune = daily::calculate_daily_calculation_for_date(
                    &user_pillars,
                    date.year(),
                    date.month(),
                    date.day(),
                );
                json!({
                    "date": date.format("%Y-%m-%d").to_string(),
                    "scores": {
                        "overall": fortune.scores.overall,
                        "love": fortune.scores.love,
                        "career": fortune.scores.career,
                        "health": fortune.scores.health,
                    },
                    "grade": score_to_grade(fortune.scores.overall),
                })
            })
            .collect();

        let avg_score = days
            .iter()
            .filter_map(|d| {
                d.get("scores")
                    .and_then(|s| s.get("overall"))
                    .and_then(|v| v.as_i64())
            })
            .sum::<i64>()
            / 7;

        let result = json!({
            "period": format!("{} ~ {}", today.format("%Y-%m-%d"), (today + chrono::Duration::days(6)).format("%Y-%m-%d")),
            "average_score": avg_score,
            "days": days,
        });

        (result, version.to_string())
    }

    fn generate_monthly(&self, input: &Value, version: &str) -> (Value, String) {
        let Some((year, month, day, hour, minute, _)) = Self::parse_birth_data(input) else {
            return (
                json!({"error": "생년월일 정보가 필요합니다"}),
                version.to_string(),
            );
        };

        let user_pillars = saju::calculate_four_pillars_precise(year, month, day, hour, minute);
        let kst = chrono::FixedOffset::east_opt(9 * 3600).unwrap();
        let now = chrono::Utc::now().with_timezone(&kst).date_naive();
        let target_year = now.year();
        let target_month = now.month();
        let total_days = days_in_month(target_year, target_month);

        // 주간별 요약 (4~5주)
        let mut weeks: Vec<Value> = Vec::new();
        let mut week_scores: Vec<i32> = Vec::new();
        let mut all_scores: Vec<i32> = Vec::new();

        for d in 1..=total_days {
            let fortune = daily::calculate_daily_calculation_for_date(
                &user_pillars,
                target_year,
                target_month,
                d,
            );
            all_scores.push(fortune.scores.overall);
            week_scores.push(fortune.scores.overall);

            if week_scores.len() == 7 || d == total_days {
                let avg = week_scores.iter().sum::<i32>() / week_scores.len() as i32;
                weeks.push(json!({
                    "week": weeks.len() + 1,
                    "average_score": avg,
                    "grade": score_to_grade(avg),
                }));
                week_scores.clear();
            }
        }

        let monthly_avg = all_scores.iter().sum::<i32>() / all_scores.len() as i32;
        let best_day = all_scores
            .iter()
            .enumerate()
            .max_by_key(|(_, s)| *s)
            .map(|(i, _)| i + 1)
            .unwrap_or(1);
        let worst_day = all_scores
            .iter()
            .enumerate()
            .min_by_key(|(_, s)| *s)
            .map(|(i, _)| i + 1)
            .unwrap_or(1);

        let result = json!({
            "year": target_year,
            "month": target_month,
            "average_score": monthly_avg,
            "grade": score_to_grade(monthly_avg),
            "best_day": best_day,
            "worst_day": worst_day,
            "weeks": weeks,
        });

        (result, version.to_string())
    }

    fn generate_compatibility(&self, input: &Value, version: &str) -> (Value, String) {
        let Some(compat) = Self::compute_compatibility(input) else {
            return (
                json!({"error": "생년월일 정보가 필요합니다"}),
                version.to_string(),
            );
        };
        (compat.to_basic_json(), version.to_string())
    }

    fn generate_compatibility_detail(&self, input: &Value, version: &str) -> (Value, String) {
        let Some(compat) = Self::compute_compatibility(input) else {
            return (
                json!({"error": "생년월일 정보가 필요합니다"}),
                version.to_string(),
            );
        };
        (compat.to_detail_json(), version.to_string())
    }

    fn generate_monthly_fortune(&self, input: &Value, version: &str) -> (Value, String) {
        let Some((birth_year, birth_month, birth_day, birth_hour, birth_minute, _)) =
            Self::parse_birth_data(input)
        else {
            return (
                json!({
                    "error": "생년월일 정보가 필요합니다",
                    "year": 0,
                    "current_month": 0,
                    "current_month_summary": {
                        "score": 75,
                        "grade": "good"
                    },
                    "months": []
                }),
                version.to_string(),
            );
        };

        let user_pillars = saju::calculate_four_pillars_precise(
            birth_year,
            birth_month,
            birth_day,
            birth_hour,
            birth_minute,
        );

        let kst = chrono::FixedOffset::east_opt(9 * 3600).unwrap();
        let now = chrono::Utc::now().with_timezone(&kst).date_naive();

        // options.year 우선, 없으면 현재 KST 연도
        let year = input
            .get("options")
            .and_then(|o| o.get("year"))
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| now.year() as i64) as i32;

        let current_month = now.month();

        let months_data = monthly::calculate_monthly_calculation(&user_pillars, year);

        // 이번 달 summary (current_month가 요청 연도에 속할 때만 유효)
        let current_summary = months_data
            .iter()
            .find(|m| m.month == current_month && year == now.year())
            .or_else(|| months_data.first())
            .map(|m| {
                json!({
                    "score": m.score,
                    "grade": m.grade,
                })
            })
            .unwrap_or_else(|| {
                json!({
                    "score": 75,
                    "grade": "good",
                })
            });

        let months_json: Vec<Value> = months_data
            .iter()
            .map(|m| {
                json!({
                    "month": m.month,
                    "score": m.score,
                    "grade": m.grade,
                    "categories": {
                        "overall": m.categories.overall,
                        "love": m.categories.love,
                        "career": m.categories.career,
                        "health": m.categories.health,
                        "wealth": m.categories.wealth,
                    },
                })
            })
            .collect();

        let result = json!({
            "year": year,
            "current_month": current_month,
            "current_month_summary": current_summary,
            "months": months_json,
        });

        (result, version.to_string())
    }

    fn generate_daeun(&self, input: &Value, version: &str) -> (Value, String) {
        let Some((birth_year, birth_month, birth_day, birth_hour, birth_minute, _)) =
            Self::parse_birth_data(input)
        else {
            return (
                json!({
                    "error": "생년월일 정보가 필요합니다",
                    "periods": [],
                    "current_period_index": null
                }),
                version.to_string(),
            );
        };

        let gender = input.get("gender").and_then(|v| v.as_str()).unwrap_or("M");

        let user_pillars = saju::calculate_four_pillars_precise(
            birth_year,
            birth_month,
            birth_day,
            birth_hour,
            birth_minute,
        );

        let periods = daeun::calculate_daeun_calculation_with_time(
            &user_pillars,
            birth_year,
            birth_month,
            birth_day,
            birth_hour,
            birth_minute,
            gender,
        );

        let current_period_index: Option<usize> = periods.iter().position(|p| p.is_current);

        let periods_json: Vec<Value> = periods
            .iter()
            .map(|p| {
                json!({
                    "start_age": p.start_age,
                    "end_age": p.end_age,
                    "stem": p.stem,
                    "branch": p.branch,
                    "element": p.element,
                    "score": p.score,
                    "is_current": p.is_current,
                })
            })
            .collect();

        let result = json!({
            "periods": periods_json,
            "current_period_index": current_period_index,
        });

        (result, version.to_string())
    }

    fn generate_fallback(
        &self,
        reading_type: &str,
        _input: &Value,
        version: &str,
    ) -> (Value, String) {
        let result = json!({
            "reading_type": reading_type,
            "score": 75,
        });
        (result, version.to_string())
    }
}

/// 궁합 분석 중간 결과
struct CompatibilityData {
    score: i32,
    grade: &'static str,
    love: i32,
    communication: i32,
    values: i32,
    lifestyle: i32,
    subject_info: Value,
    target_info: Value,
    balance1: ElementBalance,
    balance2: ElementBalance,
    branch_analysis: branches::BranchAnalysis,
    ten_god_interactions: Vec<(TenGod, TenGod, i32)>,
}

impl CompatibilityData {
    fn to_basic_json(&self) -> Value {
        json!({
            "score": self.score,
            "grade": self.grade,
            "categories": {
                "love": self.love,
                "communication": self.communication,
                "values": self.values,
                "lifestyle": self.lifestyle,
            },
            "subject_info": self.subject_info,
            "target_info": self.target_info,
        })
    }

    fn to_detail_json(&self) -> Value {
        let branch_relations: Vec<Value> = self.build_branch_relations();
        let ten_god_list: Vec<Value> = self
            .ten_god_interactions
            .iter()
            .map(|(sg, tg, score)| {
                json!({
                    "subject_god": sg.korean(),
                    "target_god": tg.korean(),
                    "score": score,
                })
            })
            .collect();

        let weakest = self.balance1.weakest();
        let lucky_color = element_to_color(weakest);
        let lucky_direction = element_to_direction(weakest);

        json!({
            "score": self.score,
            "grade": self.grade,
            "categories": {
                "love": { "score": self.love },
                "communication": { "score": self.communication },
                "values": { "score": self.values },
                "lifestyle": { "score": self.lifestyle },
            },
            "element_comparison": {
                "subject": {
                    "wood": self.balance1.wood,
                    "fire": self.balance1.fire,
                    "earth": self.balance1.earth,
                    "metal": self.balance1.metal,
                    "water": self.balance1.water,
                },
                "target": {
                    "wood": self.balance2.wood,
                    "fire": self.balance2.fire,
                    "earth": self.balance2.earth,
                    "metal": self.balance2.metal,
                    "water": self.balance2.water,
                },
            },
            "branch_relations": branch_relations,
            "ten_god_interactions": ten_god_list,
            "lucky_elements": {
                "color": lucky_color,
                "element": weakest.korean(),
                "direction": lucky_direction,
            },
            "subject_info": self.subject_info,
            "target_info": self.target_info,
        })
    }

    fn build_branch_relations(&self) -> Vec<Value> {
        let mut relations = Vec::new();
        for s in &self.branch_analysis.samhap {
            let label = match s {
                branches::SamhapResult::Full(e) => format!("삼합(三合) - {}국", e.korean()),
                branches::SamhapResult::Half(e) => format!("반합(半合) - {}국", e.korean()),
            };
            relations.push(json!({"type": label, "branches": [], "effect": "positive"}));
        }
        for y in &self.branch_analysis.yukhap {
            relations.push(json!({
                "type": "육합(六合)",
                "branches": [y.pair.0.korean(), y.pair.1.korean()],
                "effect": "positive",
            }));
        }
        for c in &self.branch_analysis.clashes {
            relations.push(json!({
                "type": "상충(相沖)",
                "branches": [c.pair.0.korean(), c.pair.1.korean()],
                "effect": "negative",
            }));
        }
        for p in &self.branch_analysis.punishments {
            let br_names: Vec<&str> = p.branches.iter().map(|b| b.korean()).collect();
            relations.push(json!({
                "type": p.punishment_type.korean(),
                "branches": br_names,
                "effect": "negative",
            }));
        }
        relations
    }
}

impl SajuEngine {
    /// 궁합 핵심 계산 (기본/상세 공용)
    fn compute_compatibility(input: &Value) -> Option<CompatibilityData> {
        let (year1, month1, day1, hour1, minute1, _) = Self::parse_birth_data(input)?;
        let pillars1 = saju::calculate_four_pillars_precise(year1, month1, day1, hour1, minute1);
        let day_master1 = pillars1.day.stem;
        let has_hour1 = input.get("birth_time").and_then(|v| v.as_str()).is_some();
        let balance1 = ElementBalance::from_pillars_with_hour(&pillars1, has_hour1);

        let target_date = input
            .get("options")
            .and_then(|o| o.get("target_birth_date"))
            .and_then(|v| v.as_str())?;
        let parts: Vec<&str> = target_date.split('-').collect();
        if parts.len() != 3 {
            return None;
        }

        let y2: i32 = parts[0].parse().ok()?;
        let m2: u32 = parts[1].parse().ok()?;
        let d2: u32 = parts[2].parse().ok()?;
        let h2 = input
            .get("options")
            .and_then(|o| o.get("target_birth_time"))
            .and_then(|v| v.as_str())
            .and_then(|t| t.split(':').next()?.parse::<u32>().ok())
            .unwrap_or(12);
        let has_hour2 = input
            .get("options")
            .and_then(|o| o.get("target_birth_time"))
            .and_then(|v| v.as_str())
            .is_some();

        let pillars2 = saju::calculate_four_pillars_precise(y2, m2, d2, h2, 0);
        let day_master2 = pillars2.day.stem;
        let balance2 = ElementBalance::from_pillars_with_hour(&pillars2, has_hour2);

        // 1. 오행 보완 점수
        let elem_score = calculate_compatibility_score(&balance1, &balance2);
        let rel = saju::elements::relation(day_master1.element(), day_master2.element());
        let rel_bonus = match rel {
            saju::elements::ElementRelation::Generated => 15,
            saju::elements::ElementRelation::Generates => 10,
            saju::elements::ElementRelation::Same => 5,
            saju::elements::ElementRelation::Controls => -5,
            saju::elements::ElementRelation::Controlled => -10,
        };
        let base_score = (elem_score + rel_bonus).clamp(30, 98);

        // 2. 지지 관계 분석
        let subj_branches = collect_branches(&pillars1, has_hour1);
        let tgt_branches = collect_branches(&pillars2, has_hour2);
        let branch_analysis = branches::analyze(&subj_branches, &tgt_branches);

        // 3. 십신 상호작용
        let ten_god_interactions =
            analyze_cross_ten_gods(&pillars1, &pillars2, has_hour1, has_hour2);
        let ten_god_bonus: i32 = ten_god_interactions.iter().map(|(_, _, s)| s).sum();

        // 4. 카테고리별 독립 점수
        let love = (base_score
            + branch_analysis.yukhap_count as i32 * 8
            + branch_analysis.samhap_count as i32 * 6
            - branch_analysis.sangchung_count as i32 * 4)
            .clamp(30, 98);

        // 천간 상생/상극 카운트
        let (sangsaeng, sanggeuk) =
            count_stem_relations(&pillars1, &pillars2, has_hour1, has_hour2);
        let communication = (base_score + sangsaeng as i32 * 6 - sanggeuk as i32 * 4).clamp(30, 98);

        let values = (base_score + ten_god_bonus).clamp(30, 98);

        let balance_diff: i32 = [
            (balance1.wood as i32 - balance2.wood as i32).abs(),
            (balance1.fire as i32 - balance2.fire as i32).abs(),
            (balance1.earth as i32 - balance2.earth as i32).abs(),
            (balance1.metal as i32 - balance2.metal as i32).abs(),
            (balance1.water as i32 - balance2.water as i32).abs(),
        ]
        .iter()
        .sum();
        let complement_bonus = if balance_diff <= 5 { 3 } else { 0 };
        let lifestyle = (base_score - balance_diff * 2 + complement_bonus).clamp(30, 98);

        // 종합 점수 = 카테고리 가중 평균
        let score =
            ((love * 30 + communication * 25 + values * 25 + lifestyle * 20) / 100).clamp(30, 98);
        let grade = score_to_grade(score);

        let target_name = input
            .get("options")
            .and_then(|o| o.get("target_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("상대방");

        let subject_info = json!({
            "name": "나",
            "day_master": day_master1.korean(),
            "element": day_master1.element().korean(),
            "animal": pillars1.year.branch.animal(),
        });
        let target_info = json!({
            "name": target_name,
            "day_master": day_master2.korean(),
            "element": day_master2.element().korean(),
            "animal": pillars2.year.branch.animal(),
        });

        Some(CompatibilityData {
            score,
            grade,
            love,
            communication,
            values,
            lifestyle,
            subject_info,
            target_info,
            balance1,
            balance2,
            branch_analysis,
            ten_god_interactions,
        })
    }
}

fn collect_branches(pillars: &FourPillars, include_hour: bool) -> Vec<Branch> {
    let mut v = vec![
        pillars.year.branch,
        pillars.month.branch,
        pillars.day.branch,
    ];
    if include_hour {
        v.push(pillars.hour.branch);
    }
    v
}

fn collect_stems(pillars: &FourPillars, include_hour: bool) -> Vec<Stem> {
    let mut v = vec![pillars.year.stem, pillars.month.stem, pillars.day.stem];
    if include_hour {
        v.push(pillars.hour.stem);
    }
    v
}

fn analyze_cross_ten_gods(
    p1: &FourPillars,
    p2: &FourPillars,
    has_hour1: bool,
    has_hour2: bool,
) -> Vec<(TenGod, TenGod, i32)> {
    let dm1 = p1.day.stem;
    let dm2 = p2.day.stem;
    let stems2 = collect_stems(p2, has_hour2);
    let stems1 = collect_stems(p1, has_hour1);

    let my_gods: Vec<TenGod> = stems2
        .iter()
        .map(|&s| ten_gods::derive_ten_god(dm1, s))
        .collect();
    let their_gods: Vec<TenGod> = stems1
        .iter()
        .map(|&s| ten_gods::derive_ten_god(dm2, s))
        .collect();

    let mut interactions = Vec::new();
    let bonus_pairs: &[(TenGod, TenGod, i32)] = &[
        (TenGod::Jeonggwan, TenGod::Jeongjae, 5),
        (TenGod::Sikshin, TenGod::Pyeonin, 4),
        (TenGod::Jeongin, TenGod::Jeonggwan, 4),
        (TenGod::Bigyeon, TenGod::Bigyeon, 3),
    ];
    let penalty_pairs: &[(TenGod, TenGod, i32)] = &[
        (TenGod::Sanggwan, TenGod::Pyeongwan, -5),
        (TenGod::Geupjae, TenGod::Pyeonjae, -4),
        (TenGod::Sanggwan, TenGod::Jeonggwan, -3),
    ];

    for &mg in &my_gods {
        for &tg in &their_gods {
            for &(a, b, score) in bonus_pairs {
                if (mg == a && tg == b) || (mg == b && tg == a) {
                    interactions.push((mg, tg, score));
                }
            }
            for &(a, b, score) in penalty_pairs {
                if (mg == a && tg == b) || (mg == b && tg == a) {
                    interactions.push((mg, tg, score));
                }
            }
        }
    }
    interactions.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
    interactions
}

fn count_stem_relations(p1: &FourPillars, p2: &FourPillars, h1: bool, h2: bool) -> (usize, usize) {
    let stems1 = collect_stems(p1, h1);
    let stems2 = collect_stems(p2, h2);
    let mut sangsaeng = 0usize;
    let mut sanggeuk = 0usize;
    for &s1 in &stems1 {
        for &s2 in &stems2 {
            if saju::elements::generates(s1.element(), s2.element())
                || saju::elements::generates(s2.element(), s1.element())
            {
                sangsaeng += 1;
            }
            if saju::elements::controls(s1.element(), s2.element())
                || saju::elements::controls(s2.element(), s1.element())
            {
                sanggeuk += 1;
            }
        }
    }
    (sangsaeng, sanggeuk)
}

fn element_to_color(e: Element) -> &'static str {
    match e {
        Element::Wood => "초록색",
        Element::Fire => "빨간색",
        Element::Earth => "노란색",
        Element::Metal => "흰색",
        Element::Water => "파란색",
    }
}

fn element_to_direction(e: Element) -> &'static str {
    match e {
        Element::Wood => "동쪽",
        Element::Fire => "남쪽",
        Element::Earth => "중앙",
        Element::Metal => "서쪽",
        Element::Water => "북쪽",
    }
}

fn day_master_info(stem: Stem) -> Value {
    json!({
        "korean": stem.korean(),
        "hanja": stem.hanja(),
        "element": stem.element().korean(),
    })
}

fn branch_info(branch: Branch) -> Value {
    json!({
        "korean": branch.korean(),
        "hanja": branch.hanja(),
        "animal": branch.animal(),
        "element": branch.element().korean(),
    })
}

fn lucky_triple_to_json(t: &lucky::LuckyTriple) -> Value {
    json!({
        "element": t.element.korean(),
        "color": t.color,
        "numbers": t.numbers,
        "direction": t.direction,
    })
}

/// 해당 월의 일 수 계산
fn days_in_month(year: i32, month: u32) -> u32 {
    // 다음 달 1일에서 하루를 빼면 이번 달 마지막 날
    let next_month_year = if month == 12 { year + 1 } else { year };
    let next_month = if month == 12 { 1 } else { month + 1 };
    chrono::NaiveDate::from_ymd_opt(next_month_year, next_month, 1)
        .and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .unwrap_or(30)
}

/// 오행 밸런스 상호보완 점수 계산
fn calculate_compatibility_score(b1: &ElementBalance, b2: &ElementBalance) -> i32 {
    // 서로의 약한 오행을 보완해주는 정도 계산
    let elements = [
        (b1.wood, b2.wood),
        (b1.fire, b2.fire),
        (b1.earth, b2.earth),
        (b1.metal, b2.metal),
        (b1.water, b2.water),
    ];

    let mut complement_score = 0i32;
    for (a, b) in &elements {
        let diff = (*a as i32 - *b as i32).abs();
        // 차이가 적당하면 상호보완 → 높은 점수
        complement_score += match diff {
            0..=1 => 14,
            2..=3 => 12,
            4..=5 => 10,
            _ => 6,
        };
    }

    complement_score.clamp(30, 98)
}

/// 점수를 등급 문자열로 변환
fn score_to_grade(score: i32) -> &'static str {
    match score {
        80..=i32::MAX => "great",
        60..=79 => "good",
        40..=59 => "normal",
        _ => "caution",
    }
}

#[cfg(test)]
mod content_depth_tests {
    //! Calculation-only JSON contract locks.

    use super::*;
    use serde_json::json;

    fn saju_input_with_time() -> Value {
        json!({
            "birth_date": "1990-05-15",
            "birth_time": "14:00",
            "gender": "male",
            "calendar_type": "solar",
        })
    }

    #[test]
    fn saju_response_includes_gongmang_shinsal_lucky() {
        let (result, _v) = SajuEngine.generate("saju", &saju_input_with_time());

        let gm = result.get("gongmang").expect("gongmang 필드 필수");
        assert!(gm.get("group_index").is_some(), "gongmang.group_index");
        assert!(gm.get("group_name").is_some(), "gongmang.group_name");
        assert!(
            gm.get("empty_branches")
                .and_then(|v| v.as_array())
                .is_some_and(|a| a.len() == 2),
            "공망 지지는 항상 2개"
        );
        assert!(gm.get("interpretation").is_none(), "gongmang prose");

        let ss = result
            .get("shinsal")
            .and_then(|v| v.as_array())
            .expect("shinsal은 배열");
        // 신살은 0개일 수 있으나 배열 자체는 항상 존재.
        for item in ss {
            assert!(item.get("kind").is_some(), "각 신살에 kind 슬러그");
            assert!(item.get("kind_korean").is_some(), "각 신살에 kind_korean");
            assert!(item.get("positions").is_some(), "각 신살에 positions");
            assert!(item.get("modern_take").is_none(), "각 신살 prose");
        }

        let lk = result.get("lucky").expect("lucky 필드 필수");
        let primary = lk.get("primary").expect("lucky.primary");
        assert!(primary.get("element").is_some());
        assert!(primary.get("color").is_some());
        assert!(
            primary
                .get("numbers")
                .and_then(|v| v.as_array())
                .is_some_and(|a| a.len() == 2),
            "행운 숫자 2개"
        );
        assert!(primary.get("direction").is_some());
        assert!(lk.get("supplementary").is_some());
        assert!(lk.get("interpretation").is_none());
    }

    #[test]
    fn daily_detail_response_includes_new_lucky_alongside_legacy() {
        let (result, _v) = SajuEngine.generate("daily_detail", &saju_input_with_time());

        // 레거시 lucky_items는 그대로 — iOS 호환.
        let legacy = result.get("lucky_items").expect("legacy lucky_items 유지");
        assert!(legacy.get("color").is_some());
        assert!(legacy.get("color_hex").is_some());
        assert!(legacy.get("number").is_some());

        // 신규 lucky (primary + supplementary).
        let new_lucky = result.get("lucky").expect("신규 lucky 필드");
        assert!(new_lucky.get("primary").is_some());
        assert!(new_lucky.get("supplementary").is_some());
    }

    #[test]
    fn daily_response_is_calculation_only() {
        let (result, version) = SajuEngine.generate("daily", &saju_input_with_time());

        assert_eq!(version, SAJU_ENGINE_VERSION);
        assert!(result["scores"]["overall"].is_number());
        assert!(result.get("lead").is_none());
        assert!(result.get("advice").is_none());
        assert!(result.get("caution").is_none());
    }

    #[test]
    fn daily_detail_response_is_calculation_only() {
        let (result, version) = SajuEngine.generate("daily_detail", &saju_input_with_time());

        assert_eq!(version, SAJU_ENGINE_VERSION);
        assert!(result.get("lead").is_none());
        assert!(result.get("advice").is_none());
        assert!(result.get("caution").is_none());
        assert!(result.get("persona_today").is_none());
        assert!(result.get("personality_summary").is_none());
        assert!(result["category_details"]["love"].get("advice").is_none());
    }

    #[test]
    fn daily_detail_v2_includes_precision_influences_and_daeun() {
        let (result, _version) = SajuEngine.generate("daily_detail", &saju_input_with_time());

        assert_eq!(result["precision"]["level"], "full");
        assert_eq!(result["precision"]["birth_time_status"], "known");
        assert_eq!(result["precision"]["daeun_status"], "included");
        assert!(
            result["missing_inputs"]
                .as_array()
                .is_some_and(|items| items.is_empty())
        );
        assert!(result.get("daily_influences").is_some());
        assert!(
            result["daily_influences"]["pillars"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item["position"] == "hour"))
        );
        assert!(result.get("current_daeun").is_some());
        assert!(
            result["scores"]["overall"]
                .as_i64()
                .is_some_and(|s| (30..=98).contains(&s))
        );
    }

    #[test]
    fn daily_detail_without_birth_time_marks_partial_and_excludes_hour() {
        let (result, _version) = SajuEngine.generate(
            "daily_detail",
            &json!({
                "birth_date": "1990-05-15",
                "gender": "male",
                "calendar_type": "solar",
            }),
        );

        assert_eq!(result["precision"]["level"], "partial");
        assert_eq!(result["precision"]["birth_time_status"], "unknown");
        assert!(
            result["missing_inputs"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item == "birth_time"))
        );
        assert!(
            result["daily_influences"]["pillars"]
                .as_array()
                .is_some_and(|items| !items.iter().any(|item| item["position"] == "hour"))
        );
    }

    #[test]
    fn daily_detail_gender_changes_daeun_context() {
        let male_input = saju_input_with_time();
        let mut female_input = saju_input_with_time();
        female_input["gender"] = json!("female");

        let (male, _version) = SajuEngine.generate("daily_detail", &male_input);
        let (female, _version) = SajuEngine.generate("daily_detail", &female_input);

        assert_ne!(male["current_daeun"], female["current_daeun"]);
    }

    #[test]
    fn saju_core_response_contains_deterministic_structured_facts() {
        let input = saju_input_with_time();
        let (first, version) = SajuEngine.generate_saju_core(&input);
        let (second, second_version) = SajuEngine.generate_saju_core(&input);

        assert_eq!(version, SAJU_ENGINE_VERSION);
        assert_eq!(second_version, SAJU_ENGINE_VERSION);
        assert_eq!(first, second);
        assert_eq!(first["schema_version"], SAJU_CORE_SCHEMA_VERSION);
        assert!(first["calculation_basis"].is_object());
        assert!(first["day_master"].is_object());
        assert!(first["element_balance"].is_object());
        assert_eq!(first["element_balance"]["total_count"], 8);
        assert!(first["dominant_element"].is_object());
        assert!(first["weakest_element"].is_object());
        assert_eq!(
            first["ten_gods_summary"]["counts"].as_array().map(Vec::len),
            Some(10)
        );
        assert!(
            first["signals"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item["kind"] == "day_master"))
        );
        assert!(
            first["evidence"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item["kind"] == "pillar"))
        );

        assert!(first.get("interpretation").is_none());
        assert!(first.get("lead").is_none());
        assert!(first.get("personality").is_none());
        assert!(first.get("fortune_outlook").is_none());
        assert!(first["element_balance"].get("analysis").is_none());
        assert!(first["gongmang"].get("interpretation").is_none());
        assert!(first["lucky"].get("interpretation").is_none());
    }

    #[test]
    fn saju_response_is_core_payload_without_legacy_prose() {
        let (result, version) = SajuEngine.generate("saju", &saju_input_with_time());

        assert_eq!(version, SAJU_ENGINE_VERSION);
        assert_eq!(result["schema_version"], SAJU_CORE_SCHEMA_VERSION);
        assert!(result.get("saju_core").is_none());
        assert!(result.get("legacy_prose").is_none());
        assert!(result.get("interpretation").is_none());
        assert!(result.get("lead").is_none());
        assert!(result.get("personality").is_none());
        assert!(result.get("fortune_outlook").is_none());
        assert!(result["element_balance"].get("analysis").is_none());
        assert!(result["gongmang"].get("interpretation").is_none());
        assert!(result["lucky"].get("interpretation").is_none());
    }

    #[test]
    fn service_layers_can_compose_from_structured_saju_core_without_prose() {
        let (core, _version) = SajuEngine.generate_saju_core(&saju_input_with_time());

        let day_stem = core["day_master"]["stem"]
            .as_str()
            .expect("structured day master stem");
        let dominant = core["dominant_element"]["element"]
            .as_str()
            .expect("structured dominant element");
        let weakest = core["weakest_element"]["element"]
            .as_str()
            .expect("structured weakest element");
        let strongest_ten_god = core["ten_gods_summary"]["strongest"]["god"]
            .as_str()
            .expect("structured strongest ten god");
        let service_payload = json!({
            "day_stem": day_stem,
            "dominant_element": dominant,
            "weakest_element": weakest,
            "strongest_ten_god": strongest_ten_god,
        });

        assert!(
            service_payload["day_stem"]
                .as_str()
                .is_some_and(|v| !v.is_empty())
        );
        assert!(core["evidence"].as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item["kind"] == "element_count"
                    && item["element"] == service_payload["dominant_element"]
            })
        }));
        assert!(core["signals"].as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item["kind"] == "strongest_ten_god"
                    && item["god"] == service_payload["strongest_ten_god"]
            })
        }));
        assert!(core.get("interpretation").is_none());
    }

    #[test]
    fn saju_core_includes_manseoryok_grade_sections() {
        let (core, _version) = SajuEngine.generate_saju_core(&saju_input_with_time());
        let manse = core.get("manseoryok").expect("manseoryok section");

        assert_eq!(
            core["calculation_profile"]["id"],
            profile::SAJU_PROFILE_ID,
            "saju core must declare its open-source compatibility target"
        );
        assert_eq!(
            core["calculation_profile"]["compatibility_target"],
            profile::SAJU_COMPATIBILITY_TARGET
        );
        assert_eq!(
            core["calculation_basis"]["calculation_profile_id"],
            profile::SAJU_PROFILE_ID
        );
        assert_eq!(manse["schema_version"], "saju_manseoryok_v1");
        assert_eq!(
            manse["calculation_basis"]["calculation_profile_version"],
            profile::SAJU_PROFILE_VERSION
        );
        assert_eq!(
            manse["hidden_stems"].as_array().map(Vec::len),
            Some(4),
            "known birth time should expose four hidden-stem sections"
        );
        assert_eq!(manse["twelve_stages"].as_array().map(Vec::len), Some(4));
        assert!(
            manse["useful_elements"]["yongsin_candidates"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
        );
        assert!(
            manse["gyeokguk"]["label"]
                .as_str()
                .is_some_and(|label| label.contains("후보"))
        );
        assert_eq!(
            manse["fortune_cycles"]["monthly"].as_array().map(Vec::len),
            Some(12)
        );
        assert_eq!(
            manse["fortune_cycles"]["daily"].as_array().map(Vec::len),
            Some(10)
        );
    }

    #[test]
    fn saju_core_matches_6tail_lunar_python_readme_fixture() {
        // 6tail/lunar-python README fixture:
        // Lunar.fromYmd(1986, 4, 21) => 1986-05-29 00:00, 丙寅年 癸巳月 癸酉日 子时.
        let input = json!({
            "birth_date": "1986-04-21",
            "birth_time": "00:00",
            "calendar_type": "lunar",
            "is_lunar_leap_month": false,
        });
        let (core, _version) = SajuEngine.generate_saju_core(&input);

        assert_eq!(core["calculation_basis"]["is_lunar_converted"], true);
        assert_eq!(
            core["calculation_basis"]["normalized_birth_date"],
            "1986-05-29"
        );
        assert_eq!(core["four_pillars"]["year"], "병인 (丙寅)");
        assert_eq!(core["four_pillars"]["month"], "계사 (癸巳)");
        assert_eq!(core["four_pillars"]["day"], "계유 (癸酉)");
        assert_eq!(core["four_pillars"]["hour"], "임자 (壬子)");
        assert_eq!(core["calculation_profile"]["id"], profile::SAJU_PROFILE_ID);
    }

    #[test]
    fn saju_core_matches_6tail_lunar_javascript_eightchar_fixture() {
        // 6tail/lunar-javascript EightChar fixture:
        // Solar.fromYmdHms(2005, 12, 23, 8, 37, 0)
        // => 乙酉 戊子 辛巳 壬辰, hidden stems 辛 / 癸 / 丙庚戊 / 戊乙癸.
        let input = json!({
            "birth_date": "2005-12-23",
            "birth_time": "08:37",
            "calendar_type": "solar",
        });
        let (core, _version) = SajuEngine.generate_saju_core(&input);

        assert_eq!(core["four_pillars"]["year"], "을유 (乙酉)");
        assert_eq!(core["four_pillars"]["month"], "무자 (戊子)");
        assert_eq!(core["four_pillars"]["day"], "신사 (辛巳)");
        assert_eq!(core["four_pillars"]["hour"], "임진 (壬辰)");
        let hidden = core["manseoryok"]["hidden_stems"].as_array().unwrap();
        assert_eq!(hidden[0]["stems"][0]["stem"]["korean"], "신");
        assert_eq!(hidden[1]["stems"][0]["stem"]["korean"], "계");
        assert_eq!(hidden[2]["stems"][0]["stem"]["korean"], "병");
        assert_eq!(hidden[2]["stems"][1]["stem"]["korean"], "경");
        assert_eq!(hidden[2]["stems"][2]["stem"]["korean"], "무");
        assert_eq!(hidden[3]["stems"][0]["stem"]["korean"], "무");
        assert_eq!(hidden[3]["stems"][1]["stem"]["korean"], "을");
        assert_eq!(hidden[3]["stems"][2]["stem"]["korean"], "계");
    }

    #[test]
    fn saju_core_matches_additional_6tail_eightchar_fixtures() {
        let fixtures = [
            (
                json!({
                    "birth_date": "1988-02-15",
                    "birth_time": "22:30",
                    "calendar_type": "solar",
                }),
                ["무진 (戊辰)", "갑인 (甲寅)", "경자 (庚子)", "정해 (丁亥)"],
            ),
            (
                json!({
                    "birth_date": "1988-02-02",
                    "birth_time": "22:30",
                    "calendar_type": "solar",
                }),
                ["정묘 (丁卯)", "계축 (癸丑)", "정해 (丁亥)", "신해 (辛亥)"],
            ),
            (
                json!({
                    "birth_date": "2019-12-12",
                    "birth_time": "11:22",
                    "calendar_type": "lunar",
                    "is_lunar_leap_month": false,
                }),
                ["기해 (己亥)", "정축 (丁丑)", "무신 (戊申)", "무오 (戊午)"],
            ),
            (
                json!({
                    "birth_date": "1999-06-07",
                    "birth_time": "09:11",
                    "calendar_type": "solar",
                }),
                ["기묘 (己卯)", "경오 (庚午)", "경인 (庚寅)", "신사 (辛巳)"],
            ),
        ];

        for (input, expected) in fixtures {
            let (core, _version) = SajuEngine.generate_saju_core(&input);
            assert_eq!(core["four_pillars"]["year"], expected[0]);
            assert_eq!(core["four_pillars"]["month"], expected[1]);
            assert_eq!(core["four_pillars"]["day"], expected[2]);
            assert_eq!(core["four_pillars"]["hour"], expected[3]);
        }
    }

    #[test]
    fn daily_detail_lunar_input_is_converted_to_solar_birth_date() {
        let (result, _version) = SajuEngine.generate(
            "daily_detail",
            &json!({
                "birth_date": "2022-06-12",
                "birth_time": "14:00",
                "gender": "male",
                "calendar_type": "lunar",
            }),
        );

        assert_eq!(result["precision"]["calendar_type"], "lunar");
        assert_eq!(result["precision"]["normalized_birth_date"], "2022-07-10");
        assert_eq!(result["precision"]["is_lunar_converted"], true);
    }

    #[test]
    fn saju_response_excludes_backend_copy_fields() {
        let (result, version) = SajuEngine.generate("saju", &saju_input_with_time());

        assert_eq!(version, SAJU_ENGINE_VERSION);
        assert!(result.get("interpretation").is_none());
        assert!(result.get("headline").is_none());
        assert!(result.get("sections").is_none());
        assert!(result.get("ai_prompts").is_none());
        assert!(result.get("lead").is_none());
    }

    #[test]
    fn natal_category_reading_is_generated_by_saju_engine() {
        let (result, version) = SajuEngine.generate("saju_wealth", &saju_input_with_time());

        assert_eq!(version, SAJU_ENGINE_VERSION);
        assert_eq!(result["reading_type"], "saju_wealth");
        assert_eq!(result["domain"], "wealth");
        assert_eq!(result["schema_version"], SAJU_CORE_SCHEMA_VERSION);
        assert!(result.get("headline").is_none());
        assert!(result.get("sections").is_none());
    }

    #[test]
    fn shinsal_kind_slugs_are_lowercase_english() {
        let (result, _v) = SajuEngine.generate("saju", &saju_input_with_time());
        let ss = result.get("shinsal").and_then(|v| v.as_array()).unwrap();
        let allowed = [
            "geop",
            "jae",
            "cheon",
            "ji",
            "dohwa",
            "wol",
            "mangsin",
            "jangseong",
            "banan",
            "yeokma",
            "yukae",
            "hwagae",
            "baekho",
            "cheoneul",
        ];
        for item in ss {
            let k = item.get("kind").and_then(|v| v.as_str()).unwrap();
            assert!(allowed.contains(&k), "예상치 못한 신살 슬러그: {}", k);
        }
    }

    #[test]
    fn gongmang_palaces_are_lowercase() {
        let (result, _v) = SajuEngine.generate("saju", &saju_input_with_time());
        let palaces = result
            .get("gongmang")
            .and_then(|g| g.get("affected_palaces"))
            .and_then(|v| v.as_array())
            .unwrap();
        let allowed = ["year", "month", "hour"];
        for p in palaces {
            let s = p.as_str().unwrap();
            assert!(allowed.contains(&s), "예상치 못한 궁 슬러그: {}", s);
        }
    }

    #[test]
    fn invalid_birth_date_returns_error_instead_of_fallback_chart() {
        let (result, _v) = SajuEngine.generate(
            "saju",
            &json!({
                "birth_date": "2024-02-31",
                "birth_time": "14:00",
            }),
        );

        assert!(result.get("error").is_some());
        assert!(result.get("four_pillars").is_none());
    }

    #[test]
    fn invalid_birth_time_returns_error_instead_of_wrapping_hour() {
        let (result, _v) = SajuEngine.generate(
            "daily",
            &json!({
                "birth_date": "2024-02-29",
                "birth_time": "24:00",
            }),
        );

        assert!(result.get("error").is_some());
    }
}
