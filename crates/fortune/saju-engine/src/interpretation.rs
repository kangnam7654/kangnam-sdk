//! 사주 종합 해석 — 일간·오행·십신을 합성해 5개 영역(본성·관계·일·돈·보완책)
//! 단락과 1줄 헤드라인을 만들어낸다.
//!
//! 카피는 `data/interpretation_copy.json` 에 정통 명리(자평진전·적천수·궁통보감
//! ·명리정종·오행대의)를 출처로 정리해 둔 표 — `include_str!`로 컴파일 타임에
//! 박고 `LazyLock` 으로 1회만 파싱해 캐시한다. 새 종속성을 더하지 않으려고
//! YAML 대신 JSON 으로 묶어 `serde_json::from_str` 로 로드한다.
//!
//! 컴포지션 룰은 의도적으로 단순하다 (일간 1, 강한오행 1, 약한오행 1, 십신
//! count 분기) — 정밀한 격국·용신 판단은 다른 모듈(daeun/shinsal 등)에 두고
//! 여기서는 일반 사용자가 한 화면에서 자기 사주를 "읽을 수 있게" 하는 게 목표.
//!
//! 출력은 `Interpretation` (Serialize) — engine.rs 의 `generate_saju()` 에서
//! 그대로 result_json 에 박힌다. 키는 `interpretation.headline` /
//! `interpretation.sections[].{key,title,body}`.

use crate::types::{ElementBalance, FourPillars, Stem, TenGod};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::LazyLock;

// ─── Loaded copy table ───────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CopyTable {
    day_masters: HashMap<String, DayMasterCopy>,
    elements_strong: HashMap<String, ElementStrongCopy>,
    elements_weak: HashMap<String, ElementWeakCopy>,
    ten_gods: HashMap<String, TenGodCopy>,
}

#[derive(Deserialize)]
struct DayMasterCopy {
    metaphor: String,
    nature: String,
    strength: String,
    shadow: String,
}

#[derive(Deserialize)]
struct ElementStrongCopy {
    dominant_meaning: String,
    dominant_strength: String,
    dominant_caution: String,
    money_pattern: String,
    work_pattern: String,
}

#[derive(Deserialize)]
struct ElementWeakCopy {
    weak_meaning: String,
    remedy_color: String,
    remedy_direction: String,
    remedy_habits: String,
}

#[derive(Deserialize)]
#[allow(dead_code)] // role_at_* / gloss 는 향후 궁위별 detail 확장 때 사용 예정 — 데이터는 보존
struct TenGodCopy {
    gloss: String,
    role_at_year: Option<String>,
    role_at_month: Option<String>,
    role_at_day: Option<String>,
    role_at_hour: Option<String>,
    relation_meaning: String,
    work_meaning: String,
    money_meaning: Option<String>,
}

static COPY: LazyLock<CopyTable> = LazyLock::new(|| {
    let raw = include_str!("../data/interpretation_copy.json");
    serde_json::from_str(raw)
        .expect("interpretation_copy.json must parse — schema drift between data/ and types")
});

// ─── Output shape ────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct InterpretationSection {
    pub key: &'static str,
    pub title: &'static str,
    pub body: String,
}

#[derive(Serialize)]
pub struct Interpretation {
    pub headline: String,
    /// Simple-tier 한 단락 요약. Detail tier 에서는 None.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Detail-tier 5섹션. Simple tier 에서는 비어있고 직렬화에서 빠진다.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<InterpretationSection>,
}

// ─── Element helpers ─────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum ElementKey {
    Wood,
    Fire,
    Earth,
    Metal,
    Water,
}

impl ElementKey {
    fn ko(self) -> &'static str {
        match self {
            ElementKey::Wood => "목",
            ElementKey::Fire => "화",
            ElementKey::Earth => "토",
            ElementKey::Metal => "금",
            ElementKey::Water => "수",
        }
    }

    fn count(self, b: &ElementBalance) -> u8 {
        match self {
            ElementKey::Wood => b.wood,
            ElementKey::Fire => b.fire,
            ElementKey::Earth => b.earth,
            ElementKey::Metal => b.metal,
            ElementKey::Water => b.water,
        }
    }

    const ALL: [ElementKey; 5] = [
        ElementKey::Wood,
        ElementKey::Fire,
        ElementKey::Earth,
        ElementKey::Metal,
        ElementKey::Water,
    ];
}

fn dominant_element(b: &ElementBalance) -> ElementKey {
    // ties: prefer earlier in ALL order (목 → 수). The deterministic tie-break
    // matters for cache stability — two equivalent inputs must produce the
    // same JSON byte-for-byte.
    let mut best = ElementKey::Wood;
    let mut best_val = 0u8;
    for e in ElementKey::ALL {
        let v = e.count(b);
        if v > best_val {
            best_val = v;
            best = e;
        }
    }
    best
}

fn weakest_element(b: &ElementBalance) -> ElementKey {
    let mut worst = ElementKey::Wood;
    let mut worst_val = u8::MAX;
    for e in ElementKey::ALL {
        let v = e.count(b);
        if v < worst_val {
            worst_val = v;
            worst = e;
        }
    }
    worst
}

// ─── Ten-god helpers ─────────────────────────────────────────────────────────

/// Strip the 한자 paren suffix from `TenGod::korean()` so we can index the
/// JSON copy table whose keys are bare Korean (`"비견"` not `"비견(比肩)"`).
/// Used directly only by tests; the section composers index by string literal
/// based on count branching.
#[cfg_attr(not(test), allow(dead_code))]
fn ten_god_key(g: TenGod) -> &'static str {
    match g {
        TenGod::Bigyeon => "비견",
        TenGod::Geupjae => "겁재",
        TenGod::Sikshin => "식신",
        TenGod::Sanggwan => "상관",
        TenGod::Pyeonjae => "편재",
        TenGod::Jeongjae => "정재",
        TenGod::Pyeongwan => "편관",
        TenGod::Jeonggwan => "정관",
        TenGod::Pyeonin => "편인",
        TenGod::Jeongin => "정인",
    }
}

#[derive(Default)]
struct GodCounts {
    bigyeon: u32,
    geupjae: u32,
    sikshin: u32,
    sanggwan: u32,
    pyeonjae: u32,
    jeongjae: u32,
    pyeongwan: u32,
    jeonggwan: u32,
    pyeonin: u32,
    jeongin: u32,
}

impl GodCounts {
    fn from_gods(gods: &[(&'static str, TenGod)]) -> Self {
        let mut c = Self::default();
        // 일간(자기 자신)은 항상 비견으로 잡혀 있는데, 이건 "내가 나"인 자리라
        // 인간관계 해석에서 "비견 1개 있음" 으로 세지 않는다 — 위치 라벨이
        // "일간"인 항목은 스킵.
        for (pos, g) in gods {
            if *pos == "일간" {
                continue;
            }
            match g {
                TenGod::Bigyeon => c.bigyeon += 1,
                TenGod::Geupjae => c.geupjae += 1,
                TenGod::Sikshin => c.sikshin += 1,
                TenGod::Sanggwan => c.sanggwan += 1,
                TenGod::Pyeonjae => c.pyeonjae += 1,
                TenGod::Jeongjae => c.jeongjae += 1,
                TenGod::Pyeongwan => c.pyeongwan += 1,
                TenGod::Jeonggwan => c.jeonggwan += 1,
                TenGod::Pyeonin => c.pyeonin += 1,
                TenGod::Jeongin => c.jeongin += 1,
            }
        }
        c
    }
}

// ─── Section composers ───────────────────────────────────────────────────────

fn day_master_key(dm: Stem) -> &'static str {
    dm.korean()
}

fn compose_headline(dm: Stem, dominant: ElementKey) -> String {
    let dm_copy = COPY
        .day_masters
        .get(day_master_key(dm))
        .expect("missing day_master copy");
    let dom_ko = dominant.ko();
    format!(
        "{}({}) — {} · {} 기운이 두텁게 깔린 사주",
        dm.korean(),
        dm.element().korean(),
        dm_copy.metaphor,
        dom_ko,
    )
}

fn compose_nature(dm: Stem, dominant: ElementKey) -> String {
    let dm_copy = COPY
        .day_masters
        .get(day_master_key(dm))
        .expect("missing day_master copy");
    let dom_copy = COPY
        .elements_strong
        .get(dominant.ko())
        .expect("missing elements_strong copy");
    // 일간 본성 + 강점·그림자 + 강한 오행이 더해주는 결 + 강함의 강점·주의.
    format!(
        "{} {} {} 사주 전체에는 {}의 기운이 가장 두텁게 깔려 있어, {} {} {}",
        dm_copy.nature,
        dm_copy.strength,
        dm_copy.shadow,
        dominant.ko(),
        dom_copy.dominant_meaning,
        dom_copy.dominant_strength,
        dom_copy.dominant_caution,
    )
}

fn compose_relation(c: &GodCounts) -> String {
    let mut lines: Vec<String> = Vec::new();

    let peers = c.bigyeon + c.geupjae;
    if peers >= 2 {
        let g = COPY
            .ten_gods
            .get(if c.geupjae >= c.bigyeon { "겁재" } else { "비견" })
            .expect("missing ten_god copy");
        lines.push(g.relation_meaning.clone());
    } else if peers == 1 {
        let key = if c.bigyeon == 1 { "비견" } else { "겁재" };
        lines.push(
            COPY.ten_gods
                .get(key)
                .expect("missing ten_god copy")
                .relation_meaning
                .clone(),
        );
    } else {
        lines.push(
            "또래나 동료보다 자기 페이스로 움직이는 결이 강해, 적지만 깊은 관계를 더 편하게 여깁니다.".into(),
        );
    }

    let mentors = c.jeongin + c.pyeonin;
    if mentors >= 1 {
        let key = if c.jeongin >= c.pyeonin { "정인" } else { "편인" };
        lines.push(
            COPY.ten_gods
                .get(key)
                .expect("missing ten_god copy")
                .relation_meaning
                .clone(),
        );
    }

    let expressors = c.sikshin + c.sanggwan;
    if expressors >= 1 {
        let key = if c.sikshin >= c.sanggwan { "식신" } else { "상관" };
        lines.push(
            COPY.ten_gods
                .get(key)
                .expect("missing ten_god copy")
                .relation_meaning
                .clone(),
        );
    }

    lines.join(" ")
}

fn compose_work(c: &GodCounts, dominant: ElementKey) -> String {
    let mut lines: Vec<String> = Vec::new();

    let officers = c.jeonggwan;
    let challenges = c.pyeongwan;
    if officers >= 1 && challenges >= 1 {
        lines.push(
            COPY.ten_gods
                .get("정관")
                .expect("missing ten_god copy")
                .work_meaning
                .clone(),
        );
        lines.push(
            COPY.ten_gods
                .get("편관")
                .expect("missing ten_god copy")
                .work_meaning
                .clone(),
        );
    } else if officers >= 1 {
        lines.push(
            COPY.ten_gods
                .get("정관")
                .expect("missing ten_god copy")
                .work_meaning
                .clone(),
        );
    } else if challenges >= 1 {
        lines.push(
            COPY.ten_gods
                .get("편관")
                .expect("missing ten_god copy")
                .work_meaning
                .clone(),
        );
    }

    let academic = c.jeongin + c.pyeonin;
    if academic >= 1 {
        let key = if c.jeongin >= c.pyeonin { "정인" } else { "편인" };
        lines.push(
            COPY.ten_gods
                .get(key)
                .expect("missing ten_god copy")
                .work_meaning
                .clone(),
        );
    }

    let creative = c.sikshin + c.sanggwan;
    if creative >= 1 {
        let key = if c.sikshin >= c.sanggwan { "식신" } else { "상관" };
        lines.push(
            COPY.ten_gods
                .get(key)
                .expect("missing ten_god copy")
                .work_meaning
                .clone(),
        );
    }

    if lines.is_empty() {
        lines.push("정해진 틀보다 자유로운 자리에서 본인 페이스로 움직일 수 있는 일이 잘 맞습니다.".into());
    }

    let dom_copy = COPY
        .elements_strong
        .get(dominant.ko())
        .expect("missing elements_strong copy");
    lines.push(dom_copy.work_pattern.clone());

    lines.join(" ")
}

fn compose_money(c: &GodCounts, dominant: ElementKey) -> String {
    let mut lines: Vec<String> = Vec::new();

    let steady = c.jeongjae;
    let flexible = c.pyeonjae;
    if steady >= 1 && flexible >= 1 {
        if let Some(m) = &COPY.ten_gods.get("정재").and_then(|t| t.money_meaning.clone()) {
            lines.push(m.clone());
        }
        if let Some(m) = &COPY.ten_gods.get("편재").and_then(|t| t.money_meaning.clone()) {
            lines.push(m.clone());
        }
    } else if steady >= 1 {
        if let Some(m) = &COPY.ten_gods.get("정재").and_then(|t| t.money_meaning.clone()) {
            lines.push(m.clone());
        }
    } else if flexible >= 1 {
        if let Some(m) = &COPY.ten_gods.get("편재").and_then(|t| t.money_meaning.clone()) {
            lines.push(m.clone());
        }
    } else {
        lines.push(
            "재물 욕심이 크지 않은 결이라, 의미와 가치를 우선하면 결과적으로 돈도 자연스럽게 따라오는 흐름입니다.".into(),
        );
    }

    let dom_copy = COPY
        .elements_strong
        .get(dominant.ko())
        .expect("missing elements_strong copy");
    lines.push(dom_copy.money_pattern.clone());

    lines.join(" ")
}

fn compose_remedies(weakest: ElementKey) -> String {
    let w = COPY
        .elements_weak
        .get(weakest.ko())
        .expect("missing elements_weak copy");
    format!(
        "{} 보완 색은 {}, 보완 방향은 {}. 일상에서는 {}.",
        w.weak_meaning, w.remedy_color, w.remedy_direction, w.remedy_habits,
    )
}

// ─── Public entry ────────────────────────────────────────────────────────────

/// Detail tier — headline + 5섹션 (본성·관계·일·돈·보완책). Paid `saju_full` 응답.
pub fn compose_detail(
    pillars: &FourPillars,
    balance: &ElementBalance,
    gods: &[(&'static str, TenGod)],
) -> Interpretation {
    let dm = pillars.day.stem;
    let dominant = dominant_element(balance);
    let weakest = weakest_element(balance);
    let counts = GodCounts::from_gods(gods);

    Interpretation {
        headline: compose_headline(dm, dominant),
        summary: None,
        sections: vec![
            InterpretationSection {
                key: "nature",
                title: "본성 — 어떤 사람인가",
                body: compose_nature(dm, dominant),
            },
            InterpretationSection {
                key: "relation",
                title: "관계 — 사람과의 결",
                body: compose_relation(&counts),
            },
            InterpretationSection {
                key: "work",
                title: "일 — 어디서 빛나는가",
                body: compose_work(&counts, dominant),
            },
            InterpretationSection {
                key: "money",
                title: "돈 — 재물의 흐름",
                body: compose_money(&counts, dominant),
            },
            InterpretationSection {
                key: "remedies",
                title: "보완책 — 일상에서 채울 것",
                body: compose_remedies(weakest),
            },
        ],
    }
}

/// Simple tier — headline + 한 단락 요약. Free `saju` 응답.
///
/// 일간 metaphor + 강한 오행이 더해주는 결 + 가장 두드러진 십신 한 줄 + 약한 오행
/// remedy 압축의 3~4문장 묶음. detail tier 의 building block 들을 재사용해 카피
/// 출처는 동일하다 (interpretation_copy.json).
pub fn compose_simple(
    pillars: &FourPillars,
    balance: &ElementBalance,
    gods: &[(&'static str, TenGod)],
) -> Interpretation {
    let dm = pillars.day.stem;
    let dominant = dominant_element(balance);
    let weakest = weakest_element(balance);
    let counts = GodCounts::from_gods(gods);

    let dm_copy = COPY
        .day_masters
        .get(day_master_key(dm))
        .expect("missing day_master copy");
    let dom_copy = COPY
        .elements_strong
        .get(dominant.ko())
        .expect("missing elements_strong copy");
    let weak_copy = COPY
        .elements_weak
        .get(weakest.ko())
        .expect("missing elements_weak copy");

    let mut sentences: Vec<String> = Vec::with_capacity(3);

    sentences.push(format!(
        "{}({}) 일간은 {}로 표현되는 결입니다.",
        dm.korean(),
        dm.element().korean(),
        dm_copy.metaphor,
    ));

    sentences.push(format!(
        "{} 기운이 두텁게 깔려 {}",
        dominant.ko(),
        dom_copy.dominant_strength,
    ));

    if let Some(gk) = dominant_god_key(&counts) {
        if let Some(g) = COPY.ten_gods.get(gk) {
            sentences.push(format!("{} — 십신 중 가장 두드러진 결입니다.", g.gloss));
        }
    }

    sentences.push(format!(
        "보완책으로는 약한 {} 기운을 위해 {} 색을 가까이 두고, {} 같은 습관을 더해보세요.",
        weakest.ko(),
        weak_copy.remedy_color,
        weak_copy.remedy_habits,
    ));

    Interpretation {
        headline: compose_headline(dm, dominant),
        summary: Some(sentences.join(" ")),
        sections: vec![],
    }
}

/// Pick the most-frequent ten-god (excluding 일간 self) deterministically.
/// Tie-break: canonical order (비견 → 정인). Returns None if all counts are 0.
fn dominant_god_key(c: &GodCounts) -> Option<&'static str> {
    let pairs: [(&'static str, u32); 10] = [
        ("비견", c.bigyeon),
        ("겁재", c.geupjae),
        ("식신", c.sikshin),
        ("상관", c.sanggwan),
        ("편재", c.pyeonjae),
        ("정재", c.jeongjae),
        ("편관", c.pyeongwan),
        ("정관", c.jeonggwan),
        ("편인", c.pyeonin),
        ("정인", c.jeongin),
    ];
    let mut best: Option<(&'static str, u32)> = None;
    for (k, v) in pairs {
        if v == 0 {
            continue;
        }
        match best {
            None => best = Some((k, v)),
            Some((_, bv)) if v > bv => best = Some((k, v)),
            _ => {}
        }
    }
    best.map(|(k, _)| k)
}

/// Convert to `serde_json::Value` for embedding in the engine response.
pub fn to_json(it: &Interpretation) -> Value {
    serde_json::to_value(it).expect("Interpretation serializes infallibly")
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pillars::calculate_four_pillars;
    use crate::ten_gods::analyze_ten_gods;

    #[test]
    fn copy_table_loads() {
        // Forces LazyLock to materialize — failure here means data/ JSON is
        // out of sync with the struct definitions in this file.
        let _ = COPY.day_masters.len();
        assert_eq!(COPY.day_masters.len(), 10);
        assert_eq!(COPY.elements_strong.len(), 5);
        assert_eq!(COPY.elements_weak.len(), 5);
        assert_eq!(COPY.ten_gods.len(), 10);
    }

    #[test]
    fn all_day_master_keys_present() {
        for s in Stem::ALL {
            assert!(
                COPY.day_masters.contains_key(s.korean()),
                "day_master copy missing for {}",
                s.korean()
            );
        }
    }

    #[test]
    fn all_ten_god_keys_present() {
        for g in [
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
        ] {
            assert!(
                COPY.ten_gods.contains_key(ten_god_key(g)),
                "ten_god copy missing for {}",
                ten_god_key(g)
            );
        }
    }

    #[test]
    fn compose_detail_produces_nonempty_sections_for_known_chart() {
        // 1991-11-09 12:00 — used as the dev test fixture in the lunawave web app.
        let pillars = calculate_four_pillars(1991, 11, 9, 12);
        let balance = ElementBalance::from_pillars_with_hour(&pillars, true);
        let gods = analyze_ten_gods(&pillars, true);
        let it = compose_detail(&pillars, &balance, &gods);

        assert!(!it.headline.is_empty(), "headline must not be empty");
        assert!(it.summary.is_none(), "detail tier must not include summary");
        assert_eq!(it.sections.len(), 5);
        for s in &it.sections {
            assert!(!s.body.is_empty(), "section {} body must not be empty", s.key);
            assert!(
                s.body.chars().count() < 1000,
                "section {} body too long ({} chars) — copy bloat?",
                s.key,
                s.body.chars().count()
            );
        }
    }

    #[test]
    fn detail_deterministic_for_same_input() {
        let pillars = calculate_four_pillars(1991, 11, 9, 12);
        let balance = ElementBalance::from_pillars_with_hour(&pillars, true);
        let gods = analyze_ten_gods(&pillars, true);
        let a = serde_json::to_string(&compose_detail(&pillars, &balance, &gods)).unwrap();
        let b = serde_json::to_string(&compose_detail(&pillars, &balance, &gods)).unwrap();
        assert_eq!(a, b, "compose_detail() must be deterministic for cache stability");
    }

    #[test]
    fn compose_simple_has_headline_and_summary_only() {
        let pillars = calculate_four_pillars(1991, 11, 9, 12);
        let balance = ElementBalance::from_pillars_with_hour(&pillars, true);
        let gods = analyze_ten_gods(&pillars, true);
        let it = compose_simple(&pillars, &balance, &gods);

        assert!(!it.headline.is_empty(), "headline must not be empty");
        let summary = it.summary.expect("simple tier must include summary");
        assert!(!summary.is_empty(), "summary must not be empty");
        assert!(
            it.sections.is_empty(),
            "simple tier must NOT include sections (got {})",
            it.sections.len()
        );
    }

    #[test]
    fn compose_simple_summary_under_cap() {
        // 한 단락 요약 — copy 출처를 그대로 인용해 길이가 늘어나면 cap 을 밟는다.
        // 350자 cap 은 web 카드 한 컨테이너에 한 화면 안에 들어가는 분량 기준.
        let pillars = calculate_four_pillars(1991, 11, 9, 12);
        let balance = ElementBalance::from_pillars_with_hour(&pillars, true);
        let gods = analyze_ten_gods(&pillars, true);
        let it = compose_simple(&pillars, &balance, &gods);
        let summary = it.summary.unwrap();
        assert!(
            summary.chars().count() < 350,
            "simple summary too long ({} chars) — copy bloat?",
            summary.chars().count()
        );
    }

    #[test]
    fn compose_simple_serialization_skips_empty_sections() {
        // skip_serializing_if 가 제대로 걸렸는지 — saju 응답 JSON 에 sections
        // 키가 있으면 클라이언트가 잘못 dispatch 한다.
        let pillars = calculate_four_pillars(1991, 11, 9, 12);
        let balance = ElementBalance::from_pillars_with_hour(&pillars, true);
        let gods = analyze_ten_gods(&pillars, true);
        let it = compose_simple(&pillars, &balance, &gods);
        let v = serde_json::to_value(&it).unwrap();
        let obj = v.as_object().unwrap();
        assert!(obj.contains_key("headline"));
        assert!(obj.contains_key("summary"));
        assert!(!obj.contains_key("sections"), "simple JSON must omit sections");
    }

    #[test]
    fn compose_detail_serialization_skips_summary() {
        let pillars = calculate_four_pillars(1991, 11, 9, 12);
        let balance = ElementBalance::from_pillars_with_hour(&pillars, true);
        let gods = analyze_ten_gods(&pillars, true);
        let it = compose_detail(&pillars, &balance, &gods);
        let v = serde_json::to_value(&it).unwrap();
        let obj = v.as_object().unwrap();
        assert!(obj.contains_key("headline"));
        assert!(obj.contains_key("sections"));
        assert!(!obj.contains_key("summary"), "detail JSON must omit summary");
    }

    #[test]
    fn simple_deterministic_for_same_input() {
        let pillars = calculate_four_pillars(1991, 11, 9, 12);
        let balance = ElementBalance::from_pillars_with_hour(&pillars, true);
        let gods = analyze_ten_gods(&pillars, true);
        let a = serde_json::to_string(&compose_simple(&pillars, &balance, &gods)).unwrap();
        let b = serde_json::to_string(&compose_simple(&pillars, &balance, &gods)).unwrap();
        assert_eq!(a, b, "compose_simple() must be deterministic for cache stability");
    }
}
