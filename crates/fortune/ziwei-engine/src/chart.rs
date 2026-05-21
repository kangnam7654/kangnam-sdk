use crate::calendar::NormalizedBirthDate;
use crate::profile::{ZIWEI_STAR_STATE_SOURCE_POLICY, calculation_profile};
use crate::types::{
    AnnualFlow, BirthData, Branch, ChartLord, DecadeCycle, DomainFact, Element, FiveElementBureau,
    FourTransformation, MajorStar, NamedStarPlacement, Palace, PalaceName, StarPlacement, StarRef,
    StarState, Stem, Transformation, Triad, TriadSummary, ZiweiChart,
};
use std::fmt;

pub const ZIWEI_SCHEMA_VERSION: &str = "ziwei_chart_v4";

const CHART_BRANCHES: [Branch; 12] = [
    Branch::Yin,
    Branch::Mao,
    Branch::Chen,
    Branch::Si,
    Branch::Wu,
    Branch::Wei,
    Branch::Shen,
    Branch::You,
    Branch::Xu,
    Branch::Hai,
    Branch::Zi,
    Branch::Chou,
];

const PALACE_SEQUENCE: [PalaceName; 12] = [
    PalaceName::Life,
    PalaceName::Siblings,
    PalaceName::Spouse,
    PalaceName::Children,
    PalaceName::Wealth,
    PalaceName::Health,
    PalaceName::Travel,
    PalaceName::Friends,
    PalaceName::Career,
    PalaceName::Property,
    PalaceName::Fortune,
    PalaceName::Parents,
];

const STEMS: [Stem; 10] = [
    Stem::Jia,
    Stem::Yi,
    Stem::Bing,
    Stem::Ding,
    Stem::Wu,
    Stem::Ji,
    Stem::Geng,
    Stem::Xin,
    Stem::Ren,
    Stem::Gui,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartInput {
    pub birth: NormalizedBirthDate,
    pub birth_time: String,
    pub hour: u32,
    pub minute: u32,
    pub target_year: Option<i32>,
    pub gender: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChartError {
    InvalidBirthHour(u32),
    InvalidBirthMinute(u32),
    InvalidLunarDay(u32),
}

impl fmt::Display for ChartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBirthHour(value) => write!(f, "invalid birth hour: {value}"),
            Self::InvalidBirthMinute(value) => write!(f, "invalid birth minute: {value}"),
            Self::InvalidLunarDay(value) => write!(f, "invalid lunar day: {value}"),
        }
    }
}

impl std::error::Error for ChartError {}

pub fn calculate_chart(input: ChartInput) -> Result<ZiweiChart, ChartError> {
    if input.minute > 59 {
        return Err(ChartError::InvalidBirthMinute(input.minute));
    }
    let hour_branch =
        branch_for_hour(input.hour).ok_or(ChartError::InvalidBirthHour(input.hour))?;
    if !(1..=30).contains(&input.birth.lunar_day) {
        return Err(ChartError::InvalidLunarDay(input.birth.lunar_day));
    }

    let hour_index = hour_order_index(hour_branch);
    let lunar_month_index = (input.birth.lunar_month as i32 - 1).rem_euclid(12);
    let life_index = wrap(lunar_month_index - hour_index as i32);
    let body_index = wrap(lunar_month_index + hour_index as i32);
    let life_palace = CHART_BRANCHES[life_index];
    let body_palace = CHART_BRANCHES[body_index];

    let lunar_year_stem = stem_for_year(input.birth.lunar_year);
    let palace_stems = palace_stems(lunar_year_stem);
    let life_stem = palace_stems[life_index];
    let five_element_bureau = five_element_bureau(life_stem, life_palace);
    let ziwei_index = ziwei_star_index(five_element_bureau.number, input.birth.lunar_day);
    let tianfu_index = wrap(-(ziwei_index as i32));
    let star_placements = major_star_placements(ziwei_index, tianfu_index);
    let lunar_year_branch = branch_for_year(input.birth.lunar_year);
    let auxiliary_star_placements = auxiliary_star_placements(
        input.birth.lunar_month,
        hour_branch,
        lunar_year_stem,
        lunar_year_branch,
    );
    let malefic_star_placements =
        malefic_star_placements(lunar_year_stem, lunar_year_branch, hour_branch);
    let named_star_placements = auxiliary_star_placements
        .iter()
        .chain(malefic_star_placements.iter())
        .cloned()
        .collect::<Vec<_>>();
    let transformations =
        four_transformations(lunar_year_stem, &star_placements, &named_star_placements);
    let triads = triads();

    let palaces: Vec<Palace> = CHART_BRANCHES
        .iter()
        .enumerate()
        .map(|(idx, branch)| {
            let palace_offset = wrap(life_index as i32 - idx as i32);
            Palace {
                branch: *branch,
                stem: palace_stems[idx],
                name: PALACE_SEQUENCE[palace_offset],
                major_stars: star_placements
                    .iter()
                    .filter(|placement| placement.branch == *branch)
                    .map(|placement| placement.star)
                    .collect(),
                auxiliary_stars: auxiliary_star_placements
                    .iter()
                    .filter(|placement| placement.branch == *branch)
                    .map(|placement| placement.star.clone())
                    .collect(),
                malefic_stars: malefic_star_placements
                    .iter()
                    .filter(|placement| placement.branch == *branch)
                    .map(|placement| placement.star.clone())
                    .collect(),
                transformations: transformations
                    .iter()
                    .filter(|transformation| transformation.branch == Some(*branch))
                    .cloned()
                    .collect(),
                is_life_palace: idx == life_index,
                is_body_palace: idx == body_index,
            }
        })
        .collect();

    let star_states = star_states(&star_placements);
    let triad_summaries = triad_summaries(
        &triads,
        &star_placements,
        &auxiliary_star_placements,
        &malefic_star_placements,
        &transformations,
    );
    let decade_cycles = decade_cycles(
        five_element_bureau.number,
        life_palace,
        lunar_year_stem,
        &palace_stems,
        input.gender.as_deref(),
        &star_placements,
        &named_star_placements,
    );
    let annual_flow = input
        .target_year
        .map(|target_year| annual_flow(target_year, &star_placements, &named_star_placements));
    let domain_facts = domain_facts(&palaces);
    let chart_lords = chart_lords(life_palace, lunar_year_branch);

    Ok(ZiweiChart {
        chart_type: "ziwei".to_string(),
        schema_version: ZIWEI_SCHEMA_VERSION.to_string(),
        calculation_profile: calculation_profile(),
        birth: BirthData {
            original_date: input.birth.original_date_string(),
            solar_date: input.birth.solar_date_string(),
            lunar_date: input.birth.lunar_date_string(),
            birth_time: input.birth_time,
            hour_branch,
            calendar_type: input.birth.calendar_type.as_str().to_string(),
            is_lunar_leap_month: input.birth.is_lunar_leap_month,
            was_lunar_converted: input.birth.was_lunar_converted(),
        },
        life_palace,
        body_palace,
        five_element_bureau,
        ziwei_star: CHART_BRANCHES[ziwei_index],
        tianfu_star: CHART_BRANCHES[tianfu_index],
        palaces,
        major_star_placements: star_placements,
        auxiliary_star_placements,
        malefic_star_placements,
        transformations,
        triads,
        star_states,
        triad_summaries,
        decade_cycles,
        annual_flow,
        chart_lords,
        domain_facts,
    })
}

pub fn branch_for_hour(hour: u32) -> Option<Branch> {
    Some(match hour {
        23 | 0 => Branch::Zi,
        1 | 2 => Branch::Chou,
        3 | 4 => Branch::Yin,
        5 | 6 => Branch::Mao,
        7 | 8 => Branch::Chen,
        9 | 10 => Branch::Si,
        11 | 12 => Branch::Wu,
        13 | 14 => Branch::Wei,
        15 | 16 => Branch::Shen,
        17 | 18 => Branch::You,
        19 | 20 => Branch::Xu,
        21 | 22 => Branch::Hai,
        _ => return None,
    })
}

fn hour_order_index(branch: Branch) -> usize {
    match branch {
        Branch::Zi => 0,
        Branch::Chou => 1,
        Branch::Yin => 2,
        Branch::Mao => 3,
        Branch::Chen => 4,
        Branch::Si => 5,
        Branch::Wu => 6,
        Branch::Wei => 7,
        Branch::Shen => 8,
        Branch::You => 9,
        Branch::Xu => 10,
        Branch::Hai => 11,
    }
}

fn stem_for_year(year: i32) -> Stem {
    STEMS[(year - 4).rem_euclid(10) as usize]
}

fn branch_for_year(year: i32) -> Branch {
    const YEAR_BRANCHES: [Branch; 12] = [
        Branch::Zi,
        Branch::Chou,
        Branch::Yin,
        Branch::Mao,
        Branch::Chen,
        Branch::Si,
        Branch::Wu,
        Branch::Wei,
        Branch::Shen,
        Branch::You,
        Branch::Xu,
        Branch::Hai,
    ];
    YEAR_BRANCHES[(year - 4).rem_euclid(12) as usize]
}

fn palace_stems(year_stem: Stem) -> [Stem; 12] {
    let yin_stem_index = ((year_stem.index() + 1) * 2 + 1 - 1) % 10;
    let mut stems = [Stem::Jia; 12];
    for (idx, stem) in stems.iter_mut().enumerate() {
        *stem = STEMS[(yin_stem_index + idx) % 10];
    }
    stems
}

fn five_element_bureau(stem: Stem, branch: Branch) -> FiveElementBureau {
    let (element, na_yin) = na_yin(stem, branch);
    let number = element.bureau_number();
    let label = format!(
        "{}{}국",
        element.korean(),
        match number {
            2 => "이",
            3 => "삼",
            4 => "사",
            5 => "오",
            6 => "육",
            _ => "",
        }
    );

    FiveElementBureau {
        element,
        number,
        label,
        na_yin: na_yin.to_string(),
    }
}

fn na_yin(stem: Stem, branch: Branch) -> (Element, &'static str) {
    let index = sexagenary_index(stem, branch);
    const TABLE: [(Element, &str); 30] = [
        (Element::Metal, "해중금"),
        (Element::Fire, "노중화"),
        (Element::Wood, "대림목"),
        (Element::Earth, "노방토"),
        (Element::Metal, "검봉금"),
        (Element::Fire, "산두화"),
        (Element::Water, "간하수"),
        (Element::Earth, "성두토"),
        (Element::Metal, "백랍금"),
        (Element::Wood, "양류목"),
        (Element::Water, "천중수"),
        (Element::Earth, "옥상토"),
        (Element::Fire, "벽력화"),
        (Element::Wood, "송백목"),
        (Element::Water, "장류수"),
        (Element::Metal, "사중금"),
        (Element::Fire, "산하화"),
        (Element::Wood, "평지목"),
        (Element::Earth, "벽상토"),
        (Element::Metal, "금박금"),
        (Element::Fire, "복등화"),
        (Element::Water, "천하수"),
        (Element::Earth, "대역토"),
        (Element::Metal, "차천금"),
        (Element::Wood, "상자목"),
        (Element::Water, "대계수"),
        (Element::Earth, "사중토"),
        (Element::Fire, "천상화"),
        (Element::Wood, "석류목"),
        (Element::Water, "대해수"),
    ];
    TABLE[index / 2]
}

fn sexagenary_index(stem: Stem, branch: Branch) -> usize {
    (0..60)
        .find(|idx| idx % 10 == stem.index() && idx % 12 == branch.cycle_index())
        .expect("stem and branch parity must form a sexagenary pair")
}

fn ziwei_star_index(bureau_number: u8, lunar_day: u32) -> usize {
    let bureau = i32::from(bureau_number);
    let day = lunar_day as i32;
    let quotient = day / bureau;
    let remainder = day % bureau;
    let ceiling_multiple = quotient + i32::from(remainder > 0);
    let diff = ceiling_multiple * bureau - day;
    let step = if diff % 2 == 0 {
        ceiling_multiple + diff
    } else {
        ceiling_multiple - diff
    };

    wrap(step - 1)
}

fn major_star_placements(ziwei_index: usize, tianfu_index: usize) -> Vec<StarPlacement> {
    let specs = [
        (MajorStar::ZiWei, ziwei_index as i32),
        (MajorStar::TianJi, ziwei_index as i32 - 1),
        (MajorStar::TaiYang, ziwei_index as i32 - 3),
        (MajorStar::WuQu, ziwei_index as i32 - 4),
        (MajorStar::TianTong, ziwei_index as i32 - 5),
        (MajorStar::LianZhen, ziwei_index as i32 - 8),
        (MajorStar::TianFu, tianfu_index as i32),
        (MajorStar::TaiYin, tianfu_index as i32 + 1),
        (MajorStar::TanLang, tianfu_index as i32 + 2),
        (MajorStar::JuMen, tianfu_index as i32 + 3),
        (MajorStar::TianXiang, tianfu_index as i32 + 4),
        (MajorStar::TianLiang, tianfu_index as i32 + 5),
        (MajorStar::QiSha, tianfu_index as i32 + 6),
        (MajorStar::PoJun, tianfu_index as i32 + 10),
    ];

    specs
        .into_iter()
        .map(|(star, index)| StarPlacement {
            star,
            branch: CHART_BRANCHES[wrap(index)],
        })
        .collect()
}

fn auxiliary_star_placements(
    lunar_month: u32,
    hour_branch: Branch,
    year_stem: Stem,
    year_branch: Branch,
) -> Vec<NamedStarPlacement> {
    let month_offset = lunar_month.saturating_sub(1) as i32;
    let hour_offset = hour_order_index(hour_branch) as i32;
    let (tian_kui, tian_yue) = kui_yue_branches(year_stem);
    let lu_cun = lu_cun_branch(year_stem);
    let tian_ma = tian_ma_branch(year_branch);
    let hong_luan = hong_luan_branch(year_branch);

    vec![
        NamedStarPlacement {
            star: StarRef::new("lu_cun", "록존", "祿存"),
            branch: lu_cun,
        },
        named_placement(
            "zuo_fu",
            "좌보",
            "左輔",
            Branch::Chen.chart_index() as i32 + month_offset,
        ),
        named_placement(
            "you_bi",
            "우필",
            "右弼",
            Branch::Xu.chart_index() as i32 - month_offset,
        ),
        named_placement(
            "wen_qu",
            "문곡",
            "文曲",
            Branch::Chen.chart_index() as i32 + hour_offset,
        ),
        named_placement(
            "wen_chang",
            "문창",
            "文昌",
            Branch::Xu.chart_index() as i32 - hour_offset,
        ),
        NamedStarPlacement {
            star: StarRef::new("tian_kui", "천괴", "天魁"),
            branch: tian_kui,
        },
        NamedStarPlacement {
            star: StarRef::new("tian_yue", "천월", "天鉞"),
            branch: tian_yue,
        },
        NamedStarPlacement {
            star: StarRef::new("tian_ma", "천마", "天馬"),
            branch: tian_ma,
        },
        NamedStarPlacement {
            star: StarRef::new("hong_luan", "홍란", "紅鸞"),
            branch: hong_luan,
        },
        NamedStarPlacement {
            star: StarRef::new("tian_xi", "천희", "天喜"),
            branch: CHART_BRANCHES[(hong_luan.chart_index() + 6) % 12],
        },
    ]
}

fn malefic_star_placements(
    year_stem: Stem,
    year_branch: Branch,
    hour_branch: Branch,
) -> Vec<NamedStarPlacement> {
    let lu_cun = lu_cun_branch(year_stem);
    let hour_offset = hour_order_index(hour_branch) as i32;
    let (huo_start, ling_start) = huo_ling_start_branches(year_branch);

    vec![
        named_placement("qing_yang", "경양", "擎羊", lu_cun.chart_index() as i32 + 1),
        named_placement("tuo_luo", "타라", "陀羅", lu_cun.chart_index() as i32 - 1),
        named_placement(
            "huo_xing",
            "화성",
            "火星",
            huo_start.chart_index() as i32 + hour_offset,
        ),
        named_placement(
            "ling_xing",
            "영성",
            "鈴星",
            ling_start.chart_index() as i32 + hour_offset,
        ),
        named_placement(
            "di_jie",
            "지겁",
            "地劫",
            Branch::Hai.chart_index() as i32 + hour_offset,
        ),
        named_placement(
            "di_kong",
            "지공",
            "地空",
            Branch::Hai.chart_index() as i32 - hour_offset,
        ),
    ]
}

fn named_placement(code: &str, ko: &str, hanja: &str, branch_index: i32) -> NamedStarPlacement {
    NamedStarPlacement {
        star: StarRef::new(code, ko, hanja),
        branch: CHART_BRANCHES[wrap(branch_index)],
    }
}

fn kui_yue_branches(year_stem: Stem) -> (Branch, Branch) {
    match year_stem {
        Stem::Jia | Stem::Wu | Stem::Geng => (Branch::Chou, Branch::Wei),
        Stem::Yi | Stem::Ji => (Branch::Zi, Branch::Shen),
        Stem::Xin => (Branch::Wu, Branch::Yin),
        Stem::Ren | Stem::Gui => (Branch::Mao, Branch::Si),
        Stem::Bing | Stem::Ding => (Branch::Hai, Branch::You),
    }
}

fn lu_cun_branch(year_stem: Stem) -> Branch {
    match year_stem {
        Stem::Jia => Branch::Yin,
        Stem::Yi => Branch::Mao,
        Stem::Bing | Stem::Wu => Branch::Si,
        Stem::Ding | Stem::Ji => Branch::Wu,
        Stem::Geng => Branch::Shen,
        Stem::Xin => Branch::You,
        Stem::Ren => Branch::Hai,
        Stem::Gui => Branch::Zi,
    }
}

fn huo_ling_start_branches(year_branch: Branch) -> (Branch, Branch) {
    match year_branch {
        Branch::Shen | Branch::Zi | Branch::Chen => (Branch::Yin, Branch::Xu),
        Branch::Yin | Branch::Wu | Branch::Xu => (Branch::Chou, Branch::Mao),
        Branch::Si | Branch::You | Branch::Chou => (Branch::Mao, Branch::Xu),
        Branch::Hai | Branch::Mao | Branch::Wei => (Branch::You, Branch::Xu),
    }
}

fn tian_ma_branch(year_branch: Branch) -> Branch {
    match year_branch {
        Branch::Shen | Branch::Zi | Branch::Chen => Branch::Yin,
        Branch::Yin | Branch::Wu | Branch::Xu => Branch::Shen,
        Branch::Si | Branch::You | Branch::Chou => Branch::Hai,
        Branch::Hai | Branch::Mao | Branch::Wei => Branch::Si,
    }
}

fn hong_luan_branch(year_branch: Branch) -> Branch {
    match year_branch {
        Branch::Zi => Branch::Mao,
        Branch::Chou => Branch::Yin,
        Branch::Yin => Branch::Chou,
        Branch::Mao => Branch::Zi,
        Branch::Chen => Branch::Hai,
        Branch::Si => Branch::Xu,
        Branch::Wu => Branch::You,
        Branch::Wei => Branch::Shen,
        Branch::Shen => Branch::Wei,
        Branch::You => Branch::Wu,
        Branch::Xu => Branch::Si,
        Branch::Hai => Branch::Chen,
    }
}

fn four_transformations(
    year_stem: Stem,
    star_placements: &[StarPlacement],
    named_star_placements: &[NamedStarPlacement],
) -> Vec<Transformation> {
    let specs = match year_stem {
        Stem::Jia => [
            (FourTransformation::HuaLu, major_ref(MajorStar::LianZhen)),
            (FourTransformation::HuaQuan, major_ref(MajorStar::PoJun)),
            (FourTransformation::HuaKe, major_ref(MajorStar::WuQu)),
            (FourTransformation::HuaJi, major_ref(MajorStar::TaiYang)),
        ],
        Stem::Yi => [
            (FourTransformation::HuaLu, major_ref(MajorStar::TianJi)),
            (FourTransformation::HuaQuan, major_ref(MajorStar::TianLiang)),
            (FourTransformation::HuaKe, major_ref(MajorStar::ZiWei)),
            (FourTransformation::HuaJi, major_ref(MajorStar::TaiYin)),
        ],
        Stem::Bing => [
            (FourTransformation::HuaLu, major_ref(MajorStar::TianTong)),
            (FourTransformation::HuaQuan, major_ref(MajorStar::TianJi)),
            (
                FourTransformation::HuaKe,
                StarRef::new("wen_chang", "문창", "文昌"),
            ),
            (FourTransformation::HuaJi, major_ref(MajorStar::LianZhen)),
        ],
        Stem::Ding => [
            (FourTransformation::HuaLu, major_ref(MajorStar::TaiYin)),
            (FourTransformation::HuaQuan, major_ref(MajorStar::TianTong)),
            (FourTransformation::HuaKe, major_ref(MajorStar::TianJi)),
            (FourTransformation::HuaJi, major_ref(MajorStar::JuMen)),
        ],
        Stem::Wu => [
            (FourTransformation::HuaLu, major_ref(MajorStar::TanLang)),
            (FourTransformation::HuaQuan, major_ref(MajorStar::TaiYin)),
            (
                FourTransformation::HuaKe,
                StarRef::new("you_bi", "우필", "右弼"),
            ),
            (FourTransformation::HuaJi, major_ref(MajorStar::TianJi)),
        ],
        Stem::Ji => [
            (FourTransformation::HuaLu, major_ref(MajorStar::WuQu)),
            (FourTransformation::HuaQuan, major_ref(MajorStar::TanLang)),
            (FourTransformation::HuaKe, major_ref(MajorStar::TianLiang)),
            (
                FourTransformation::HuaJi,
                StarRef::new("wen_qu", "문곡", "文曲"),
            ),
        ],
        Stem::Geng => [
            (FourTransformation::HuaLu, major_ref(MajorStar::TaiYang)),
            (FourTransformation::HuaQuan, major_ref(MajorStar::WuQu)),
            (FourTransformation::HuaKe, major_ref(MajorStar::TaiYin)),
            (FourTransformation::HuaJi, major_ref(MajorStar::TianTong)),
        ],
        Stem::Xin => [
            (FourTransformation::HuaLu, major_ref(MajorStar::JuMen)),
            (FourTransformation::HuaQuan, major_ref(MajorStar::TaiYang)),
            (
                FourTransformation::HuaKe,
                StarRef::new("wen_qu", "문곡", "文曲"),
            ),
            (
                FourTransformation::HuaJi,
                StarRef::new("wen_chang", "문창", "文昌"),
            ),
        ],
        Stem::Ren => [
            (FourTransformation::HuaLu, major_ref(MajorStar::TianLiang)),
            (FourTransformation::HuaQuan, major_ref(MajorStar::ZiWei)),
            (
                FourTransformation::HuaKe,
                StarRef::new("zuo_fu", "좌보", "左輔"),
            ),
            (FourTransformation::HuaJi, major_ref(MajorStar::WuQu)),
        ],
        Stem::Gui => [
            (FourTransformation::HuaLu, major_ref(MajorStar::PoJun)),
            (FourTransformation::HuaQuan, major_ref(MajorStar::JuMen)),
            (FourTransformation::HuaKe, major_ref(MajorStar::TaiYin)),
            (FourTransformation::HuaJi, major_ref(MajorStar::TanLang)),
        ],
    };

    specs
        .into_iter()
        .map(|(kind, star)| {
            let branch = star_placements
                .iter()
                .find(|placement| placement.star.code() == star.code)
                .map(|placement| placement.branch)
                .or_else(|| {
                    named_star_placements
                        .iter()
                        .find(|placement| placement.star.code == star.code)
                        .map(|placement| placement.branch)
                });
            let placement_status = if star_placements
                .iter()
                .any(|placement| placement.star.code() == star.code)
            {
                "placed_major_star"
            } else if branch.is_some() {
                "placed_auxiliary_star"
            } else {
                "unplaced_auxiliary_star_pending"
            }
            .to_string();

            Transformation {
                kind,
                star,
                branch,
                placement_status,
            }
        })
        .collect()
}

fn major_ref(star: MajorStar) -> StarRef {
    star.into()
}

fn triads() -> Vec<Triad> {
    CHART_BRANCHES
        .iter()
        .map(|branch| {
            let index = branch.chart_index();
            Triad {
                palace: *branch,
                related_palaces: vec![
                    CHART_BRANCHES[(index + 4) % 12],
                    CHART_BRANCHES[(index + 8) % 12],
                    CHART_BRANCHES[(index + 6) % 12],
                ],
            }
        })
        .collect()
}

fn star_states(star_placements: &[StarPlacement]) -> Vec<StarState> {
    star_placements
        .iter()
        .map(|placement| {
            let level = major_star_state(placement.star, placement.branch);
            StarState {
                star: placement.star.into(),
                branch: placement.branch,
                level: level.to_string(),
                label: state_label(level).to_string(),
                source_policy: ZIWEI_STAR_STATE_SOURCE_POLICY.to_string(),
            }
        })
        .collect()
}

fn major_star_state(star: MajorStar, branch: Branch) -> &'static str {
    let branch_index = branch.chart_index();
    let star_index = match star {
        MajorStar::ZiWei | MajorStar::TianFu => 0,
        MajorStar::TianJi | MajorStar::TaiYin => 1,
        MajorStar::TaiYang | MajorStar::WuQu => 2,
        MajorStar::TianTong | MajorStar::TianXiang => 3,
        MajorStar::LianZhen | MajorStar::TianLiang => 4,
        MajorStar::TanLang | MajorStar::JuMen => 5,
        MajorStar::QiSha | MajorStar::PoJun => 6,
    };
    match (branch_index + star_index) % 5 {
        0 => "miao",
        1 => "wang",
        2 => "de",
        3 => "ping",
        _ => "xian",
    }
}

fn state_label(level: &str) -> &'static str {
    match level {
        "miao" => "묘",
        "wang" => "왕",
        "de" => "득",
        "ping" => "평",
        _ => "함",
    }
}

fn triad_summaries(
    triads: &[Triad],
    major_stars: &[StarPlacement],
    auxiliary_stars: &[NamedStarPlacement],
    malefic_stars: &[NamedStarPlacement],
    transformations: &[Transformation],
) -> Vec<TriadSummary> {
    triads
        .iter()
        .map(|triad| {
            let mut branches = triad.related_palaces.clone();
            branches.push(triad.palace);
            TriadSummary {
                palace: triad.palace,
                related_palaces: triad.related_palaces.clone(),
                major_star_count: major_stars
                    .iter()
                    .filter(|placement| branches.contains(&placement.branch))
                    .count(),
                auxiliary_star_count: auxiliary_stars
                    .iter()
                    .filter(|placement| branches.contains(&placement.branch))
                    .count(),
                malefic_star_count: malefic_stars
                    .iter()
                    .filter(|placement| branches.contains(&placement.branch))
                    .count(),
                transformation_count: transformations
                    .iter()
                    .filter(|transformation| {
                        transformation
                            .branch
                            .is_some_and(|branch| branches.contains(&branch))
                    })
                    .count(),
            }
        })
        .collect()
}

fn decade_cycles(
    start_age: u8,
    life_palace: Branch,
    year_stem: Stem,
    palace_stems: &[Stem; 12],
    gender: Option<&str>,
    major_stars: &[StarPlacement],
    named_star_placements: &[NamedStarPlacement],
) -> Vec<DecadeCycle> {
    let (clockwise, direction) = decade_direction(year_stem, gender);
    let direction = if direction == "unknown_default_clockwise" {
        direction
    } else if clockwise {
        "clockwise"
    } else {
        "counterclockwise"
    };
    (0..12)
        .map(|index| {
            let palace_index = if clockwise {
                (life_palace.chart_index() + index) % 12
            } else {
                (life_palace.chart_index() + 12 - index % 12) % 12
            };
            let stem = palace_stems[palace_index];
            DecadeCycle {
                index,
                start_age: start_age + (index as u8 * 10),
                end_age: start_age + (index as u8 * 10) + 9,
                palace: CHART_BRANCHES[palace_index],
                stem,
                branch: CHART_BRANCHES[palace_index],
                transformations: four_transformations(stem, major_stars, named_star_placements),
                direction: direction.to_string(),
            }
        })
        .collect()
}

fn decade_direction(year_stem: Stem, gender: Option<&str>) -> (bool, &'static str) {
    let is_male = matches!(
        gender.unwrap_or("").to_ascii_lowercase().as_str(),
        "m" | "male" | "남" | "남성"
    );
    let is_female = matches!(
        gender.unwrap_or("").to_ascii_lowercase().as_str(),
        "f" | "female" | "여" | "여성"
    );
    let yang = stem_is_yang(year_stem);
    if is_male {
        (
            yang,
            if yang {
                "clockwise"
            } else {
                "counterclockwise"
            },
        )
    } else if is_female {
        (
            !yang,
            if !yang {
                "clockwise"
            } else {
                "counterclockwise"
            },
        )
    } else {
        (true, "unknown_default_clockwise")
    }
}

fn stem_is_yang(stem: Stem) -> bool {
    matches!(
        stem,
        Stem::Jia | Stem::Bing | Stem::Wu | Stem::Geng | Stem::Ren
    )
}

fn chart_lords(life_palace: Branch, year_branch: Branch) -> Vec<ChartLord> {
    vec![
        ChartLord {
            kind: "ming_zhu".to_string(),
            star: ming_lord(life_palace),
            basis: "life_palace_branch_table_v1".to_string(),
        },
        ChartLord {
            kind: "shen_zhu".to_string(),
            star: shen_lord(year_branch),
            basis: "year_branch_table_v1".to_string(),
        },
    ]
}

fn ming_lord(branch: Branch) -> StarRef {
    match branch {
        Branch::Zi => StarRef::new("tan_lang", "탐랑", "貪狼"),
        Branch::Chou | Branch::Hai => StarRef::new("ju_men", "거문", "巨門"),
        Branch::Yin | Branch::Xu => StarRef::new("lu_cun", "록존", "祿存"),
        Branch::Mao | Branch::You => StarRef::new("wen_qu", "문곡", "文曲"),
        Branch::Chen | Branch::Shen => StarRef::new("lian_zhen", "염정", "廉貞"),
        Branch::Si | Branch::Wei => StarRef::new("wu_qu", "무곡", "武曲"),
        Branch::Wu => StarRef::new("po_jun", "파군", "破軍"),
    }
}

fn shen_lord(branch: Branch) -> StarRef {
    match branch {
        Branch::Zi | Branch::Wu => StarRef::new("huo_xing", "화성", "火星"),
        Branch::Chou | Branch::Wei => StarRef::new("tian_xiang", "천상", "天相"),
        Branch::Yin | Branch::Shen => StarRef::new("tian_liang", "천량", "天梁"),
        Branch::Mao | Branch::You => StarRef::new("tian_tong", "천동", "天同"),
        Branch::Chen | Branch::Xu => StarRef::new("wen_chang", "문창", "文昌"),
        Branch::Si | Branch::Hai => StarRef::new("tian_ji", "천기", "天機"),
    }
}

fn annual_flow(
    year: i32,
    star_placements: &[StarPlacement],
    named_star_placements: &[NamedStarPlacement],
) -> AnnualFlow {
    let stem = stem_for_year(year);
    let branch = branch_for_year(year);
    AnnualFlow {
        year,
        stem,
        branch,
        palace: branch,
        transformations: four_transformations(stem, star_placements, named_star_placements),
        source_policy: "target_year_stem_branch_with_annual_life_palace".to_string(),
    }
}

fn domain_facts(palaces: &[Palace]) -> Vec<DomainFact> {
    palaces
        .iter()
        .map(|palace| {
            let score = (palace.major_stars.len() * 2
                + palace.auxiliary_stars.len()
                + palace.transformations.len()) as i32
                - palace.malefic_stars.len() as i32;
            DomainFact {
                domain: palace.name.code().to_string(),
                palace: palace.branch,
                label: palace.name.korean().to_string(),
                major_star_count: palace.major_stars.len(),
                auxiliary_star_count: palace.auxiliary_stars.len(),
                malefic_star_count: palace.malefic_stars.len(),
                transformation_count: palace.transformations.len(),
                signal_level: if score >= 4 {
                    "strong"
                } else if score >= 2 {
                    "active"
                } else if score <= -1 {
                    "caution"
                } else {
                    "quiet"
                }
                .to_string(),
            }
        })
        .collect()
}

fn wrap(index: i32) -> usize {
    index.rem_euclid(12) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::normalize_birth_date;

    fn sample_chart() -> ZiweiChart {
        let birth = normalize_birth_date(1990, 5, 15, Some("solar"), false).unwrap();
        calculate_chart(ChartInput {
            birth,
            birth_time: "14:30".to_string(),
            hour: 14,
            minute: 30,
            target_year: None,
            gender: None,
        })
        .unwrap()
    }

    #[test]
    fn hour_branch_uses_two_hour_blocks() {
        assert_eq!(branch_for_hour(0), Some(Branch::Zi));
        assert_eq!(branch_for_hour(23), Some(Branch::Zi));
        assert_eq!(branch_for_hour(14), Some(Branch::Wei));
        assert_eq!(branch_for_hour(22), Some(Branch::Hai));
        assert_eq!(branch_for_hour(24), None);
    }

    #[test]
    fn five_element_bureau_uses_life_palace_na_yin() {
        let bureau = five_element_bureau(Stem::Bing, Branch::Xu);

        assert_eq!(bureau.element, Element::Earth);
        assert_eq!(bureau.number, 5);
        assert_eq!(bureau.label, "토오국");
        assert_eq!(bureau.na_yin, "옥상토");
    }

    #[test]
    fn ziwei_formula_locks_known_table_examples() {
        assert_eq!(CHART_BRANCHES[ziwei_star_index(2, 1)], Branch::Chou);
        assert_eq!(CHART_BRANCHES[ziwei_star_index(3, 16)], Branch::You);
        assert_eq!(CHART_BRANCHES[ziwei_star_index(4, 22)], Branch::You);
        assert_eq!(CHART_BRANCHES[ziwei_star_index(3, 27)], Branch::Xu);
    }

    #[test]
    fn chart_contains_all_twelve_palaces_and_fourteen_major_stars() {
        let chart = sample_chart();

        assert_eq!(chart.palaces.len(), 12);
        assert_eq!(chart.major_star_placements.len(), 14);
        assert_eq!(chart.auxiliary_star_placements.len(), 10);
        assert_eq!(chart.malefic_star_placements.len(), 6);
        assert_eq!(chart.transformations.len(), 4);
        assert_eq!(chart.triads.len(), 12);
        assert_eq!(chart.star_states.len(), 14);
        assert_eq!(chart.triad_summaries.len(), 12);
        assert_eq!(chart.decade_cycles.len(), 12);
        assert_eq!(chart.domain_facts.len(), 12);
        assert_eq!(
            chart
                .palaces
                .iter()
                .filter(|palace| palace.is_life_palace)
                .count(),
            1
        );
        assert_eq!(
            chart
                .palaces
                .iter()
                .filter(|palace| palace.is_body_palace)
                .count(),
            1
        );
    }

    #[test]
    fn auxiliary_and_malefic_stars_are_placed_into_palaces() {
        let chart = sample_chart();

        assert_eq!(
            chart
                .auxiliary_star_placements
                .iter()
                .map(|placement| placement.star.code.as_str())
                .collect::<Vec<_>>(),
            vec![
                "lu_cun",
                "zuo_fu",
                "you_bi",
                "wen_qu",
                "wen_chang",
                "tian_kui",
                "tian_yue",
                "tian_ma",
                "hong_luan",
                "tian_xi",
            ]
        );
        assert_eq!(
            chart
                .malefic_star_placements
                .iter()
                .map(|placement| placement.star.code.as_str())
                .collect::<Vec<_>>(),
            vec![
                "qing_yang",
                "tuo_luo",
                "huo_xing",
                "ling_xing",
                "di_jie",
                "di_kong",
            ]
        );

        let palace_auxiliary_total: usize = chart
            .palaces
            .iter()
            .map(|palace| palace.auxiliary_stars.len())
            .sum();
        let palace_malefic_total: usize = chart
            .palaces
            .iter()
            .map(|palace| palace.malefic_stars.len())
            .sum();
        assert_eq!(
            palace_auxiliary_total,
            chart.auxiliary_star_placements.len()
        );
        assert_eq!(palace_malefic_total, chart.malefic_star_placements.len());
    }

    #[test]
    fn four_transformations_follow_lunar_year_stem_table() {
        let chart = sample_chart();

        assert_eq!(
            chart
                .transformations
                .iter()
                .map(|item| (item.kind, item.star.ko.as_str(), item.branch))
                .collect::<Vec<_>>(),
            vec![
                (FourTransformation::HuaLu, "태양", Some(Branch::Wei)),
                (FourTransformation::HuaQuan, "무곡", Some(Branch::Wu)),
                (FourTransformation::HuaKe, "태음", Some(Branch::Wei)),
                (FourTransformation::HuaJi, "천동", Some(Branch::Si)),
            ]
        );
        assert!(
            chart
                .palaces
                .iter()
                .any(|palace| palace.branch == Branch::Wei && palace.transformations.len() == 2)
        );
    }

    #[test]
    fn four_transformations_table_covers_all_stems_and_places_auxiliary_stars() {
        let fixtures = [
            (
                Stem::Jia,
                ["염정", "파군", "무곡", "태양"],
                [
                    "placed_major_star",
                    "placed_major_star",
                    "placed_major_star",
                    "placed_major_star",
                ],
            ),
            (
                Stem::Yi,
                ["천기", "천량", "자미", "태음"],
                [
                    "placed_major_star",
                    "placed_major_star",
                    "placed_major_star",
                    "placed_major_star",
                ],
            ),
            (
                Stem::Bing,
                ["천동", "천기", "문창", "염정"],
                [
                    "placed_major_star",
                    "placed_major_star",
                    "placed_auxiliary_star",
                    "placed_major_star",
                ],
            ),
            (
                Stem::Ding,
                ["태음", "천동", "천기", "거문"],
                [
                    "placed_major_star",
                    "placed_major_star",
                    "placed_major_star",
                    "placed_major_star",
                ],
            ),
            (
                Stem::Wu,
                ["탐랑", "태음", "우필", "천기"],
                [
                    "placed_major_star",
                    "placed_major_star",
                    "placed_auxiliary_star",
                    "placed_major_star",
                ],
            ),
            (
                Stem::Ji,
                ["무곡", "탐랑", "천량", "문곡"],
                [
                    "placed_major_star",
                    "placed_major_star",
                    "placed_major_star",
                    "placed_auxiliary_star",
                ],
            ),
            (
                Stem::Geng,
                ["태양", "무곡", "태음", "천동"],
                [
                    "placed_major_star",
                    "placed_major_star",
                    "placed_major_star",
                    "placed_major_star",
                ],
            ),
            (
                Stem::Xin,
                ["거문", "태양", "문곡", "문창"],
                [
                    "placed_major_star",
                    "placed_major_star",
                    "placed_auxiliary_star",
                    "placed_auxiliary_star",
                ],
            ),
            (
                Stem::Ren,
                ["천량", "자미", "좌보", "무곡"],
                [
                    "placed_major_star",
                    "placed_major_star",
                    "placed_auxiliary_star",
                    "placed_major_star",
                ],
            ),
            (
                Stem::Gui,
                ["파군", "거문", "태음", "탐랑"],
                [
                    "placed_major_star",
                    "placed_major_star",
                    "placed_major_star",
                    "placed_major_star",
                ],
            ),
        ];
        let placements = major_star_placements(0, 0);

        for (stem, stars, expected_statuses) in fixtures {
            let auxiliary = auxiliary_star_placements(5, Branch::Wei, stem, Branch::Wu);
            let transformations = four_transformations(stem, &placements, &auxiliary);
            assert_eq!(transformations.len(), 4);
            assert_eq!(
                transformations
                    .iter()
                    .map(|item| item.star.ko.as_str())
                    .collect::<Vec<_>>(),
                stars
            );
            assert_eq!(
                transformations
                    .iter()
                    .map(|item| item.branch.is_some())
                    .collect::<Vec<_>>(),
                vec![true, true, true, true]
            );
            assert_eq!(
                transformations
                    .iter()
                    .map(|item| item.placement_status.as_str())
                    .collect::<Vec<_>>(),
                expected_statuses
            );
        }
    }

    #[test]
    fn major_star_states_cover_all_major_stars() {
        let chart = sample_chart();
        let labels = ["묘", "왕", "득", "평", "함"];

        assert_eq!(chart.star_states.len(), chart.major_star_placements.len());
        assert!(
            chart
                .star_states
                .iter()
                .all(|state| labels.contains(&state.label.as_str()))
        );
        assert!(
            chart
                .star_states
                .iter()
                .all(|state| { state.source_policy == ZIWEI_STAR_STATE_SOURCE_POLICY })
        );
    }

    #[test]
    fn triads_include_three_related_branches_for_each_palace() {
        let chart = sample_chart();

        let yin = chart
            .triads
            .iter()
            .find(|triad| triad.palace == Branch::Yin)
            .expect("yin triad should exist");

        assert_eq!(
            yin.related_palaces,
            vec![Branch::Wu, Branch::Xu, Branch::Shen]
        );
    }

    #[test]
    fn triad_summaries_count_related_palace_signals() {
        let chart = sample_chart();
        let total_transformations: usize = chart
            .triad_summaries
            .iter()
            .map(|summary| summary.transformation_count)
            .sum();

        assert_eq!(chart.triad_summaries.len(), 12);
        assert!(
            chart
                .triad_summaries
                .iter()
                .all(|summary| summary.related_palaces.len() == 3)
        );
        assert_eq!(total_transformations, chart.transformations.len() * 4);
    }

    #[test]
    fn decade_cycles_start_from_bureau_age_and_life_palace() {
        let chart = sample_chart();
        let first_cycle = chart.decade_cycles.first().unwrap();
        let last_cycle = chart.decade_cycles.last().unwrap();

        assert_eq!(chart.decade_cycles.len(), 12);
        assert_eq!(first_cycle.start_age, chart.five_element_bureau.number);
        assert_eq!(first_cycle.end_age, chart.five_element_bureau.number + 9);
        assert_eq!(first_cycle.palace, chart.life_palace);
        assert_eq!(last_cycle.start_age, chart.five_element_bureau.number + 110);
        assert!(
            chart
                .decade_cycles
                .iter()
                .all(|cycle| cycle.direction == "unknown_default_clockwise")
        );
    }

    #[test]
    fn decade_cycles_follow_gender_and_year_stem_direction() {
        let birth = normalize_birth_date(1990, 5, 15, Some("solar"), false).unwrap();
        let male_chart = calculate_chart(ChartInput {
            birth: birth.clone(),
            birth_time: "14:30".to_string(),
            hour: 14,
            minute: 30,
            target_year: None,
            gender: Some("male".to_string()),
        })
        .unwrap();
        let female_chart = calculate_chart(ChartInput {
            birth,
            birth_time: "14:30".to_string(),
            hour: 14,
            minute: 30,
            target_year: None,
            gender: Some("female".to_string()),
        })
        .unwrap();

        assert_eq!(male_chart.decade_cycles[0].direction, "clockwise");
        assert_eq!(female_chart.decade_cycles[0].direction, "counterclockwise");
        assert_ne!(
            male_chart.decade_cycles[1].palace,
            female_chart.decade_cycles[1].palace
        );
        assert_eq!(male_chart.decade_cycles[0].transformations.len(), 4);
    }

    #[test]
    fn chart_lords_are_exposed_for_life_and_year_branch() {
        let chart = sample_chart();

        assert_eq!(chart.chart_lords.len(), 2);
        assert_eq!(chart.chart_lords[0].kind, "ming_zhu");
        assert_eq!(chart.chart_lords[1].kind, "shen_zhu");
        assert!(!chart.chart_lords[0].star.ko.is_empty());
        assert!(!chart.chart_lords[1].star.ko.is_empty());
    }

    #[test]
    fn annual_flow_uses_target_year_stem_branch_when_requested() {
        let birth = normalize_birth_date(1990, 5, 15, Some("solar"), false).unwrap();
        let chart = calculate_chart(ChartInput {
            birth,
            birth_time: "14:30".to_string(),
            hour: 14,
            minute: 30,
            target_year: Some(2026),
            gender: None,
        })
        .unwrap();
        let flow = chart.annual_flow.as_ref().expect("annual flow exists");

        assert_eq!(flow.year, 2026);
        assert_eq!(flow.stem, Stem::Bing);
        assert_eq!(flow.branch, Branch::Wu);
        assert_eq!(flow.palace, Branch::Wu);
        assert_eq!(flow.transformations.len(), 4);
        assert_eq!(
            flow.transformations
                .iter()
                .map(|item| item.placement_status.as_str())
                .collect::<Vec<_>>(),
            vec![
                "placed_major_star",
                "placed_major_star",
                "placed_auxiliary_star",
                "placed_major_star",
            ]
        );
        assert_eq!(
            flow.source_policy,
            "target_year_stem_branch_with_annual_life_palace"
        );
    }

    #[test]
    fn domain_facts_cover_all_palaces_for_product_interpretation() {
        let chart = sample_chart();
        let domains = chart
            .domain_facts
            .iter()
            .map(|fact| fact.domain.as_str())
            .collect::<Vec<_>>();

        assert_eq!(chart.domain_facts.len(), 12);
        assert_eq!(
            domains,
            vec![
                "career", "friends", "travel", "health", "wealth", "children", "spouse",
                "siblings", "life", "parents", "fortune", "property",
            ]
        );
        assert!(chart.domain_facts.iter().all(|fact| !fact.label.is_empty()));
        assert!(chart.domain_facts.iter().all(|fact| matches!(
            fact.signal_level.as_str(),
            "strong" | "active" | "quiet" | "caution"
        )));
    }

    #[test]
    fn chart_fixture_locks_lunar_2022_06_16_wei_hour() {
        let birth = normalize_birth_date(2022, 6, 16, Some("lunar"), false).unwrap();
        let chart = calculate_chart(ChartInput {
            birth,
            birth_time: "14:30".to_string(),
            hour: 14,
            minute: 30,
            target_year: None,
            gender: None,
        })
        .unwrap();

        assert_eq!(chart.birth.lunar_date, "2022-06-16");
        assert_eq!(chart.birth.hour_branch, Branch::Wei);
        assert_eq!(chart.life_palace, Branch::Zi);
        assert_eq!(chart.body_palace, Branch::Yin);
        assert_eq!(chart.five_element_bureau.element, Element::Wood);
        assert_eq!(chart.five_element_bureau.number, 3);
        assert_eq!(chart.ziwei_star, Branch::You);
        assert_eq!(chart.tianfu_star, Branch::Wei);
        assert_eq!(
            chart
                .major_star_placements
                .iter()
                .map(|placement| (placement.star, placement.branch))
                .collect::<Vec<_>>(),
            vec![
                (MajorStar::ZiWei, Branch::You),
                (MajorStar::TianJi, Branch::Shen),
                (MajorStar::TaiYang, Branch::Wu),
                (MajorStar::WuQu, Branch::Si),
                (MajorStar::TianTong, Branch::Chen),
                (MajorStar::LianZhen, Branch::Chou),
                (MajorStar::TianFu, Branch::Wei),
                (MajorStar::TaiYin, Branch::Shen),
                (MajorStar::TanLang, Branch::You),
                (MajorStar::JuMen, Branch::Xu),
                (MajorStar::TianXiang, Branch::Hai),
                (MajorStar::TianLiang, Branch::Zi),
                (MajorStar::QiSha, Branch::Chou),
                (MajorStar::PoJun, Branch::Si),
            ]
        );
    }

    #[test]
    fn chart_fixture_locks_iztro_documented_example() {
        let birth = normalize_birth_date(2000, 8, 16, Some("solar"), false).unwrap();
        let chart = calculate_chart(ChartInput {
            birth,
            birth_time: "04:00".to_string(),
            hour: 4,
            minute: 0,
            target_year: None,
            gender: None,
        })
        .unwrap();

        assert_eq!(chart.birth.solar_date, "2000-08-16");
        assert_eq!(chart.birth.lunar_date, "2000-07-17");
        assert_eq!(chart.birth.hour_branch, Branch::Yin);
        assert_eq!(chart.life_palace, Branch::Wu);
        assert_eq!(chart.body_palace, Branch::Xu);
        assert_eq!(chart.five_element_bureau.element, Element::Wood);
        assert_eq!(chart.five_element_bureau.number, 3);
        assert_eq!(chart.five_element_bureau.label, "목삼국");

        assert_eq!(
            chart
                .palaces
                .iter()
                .map(|palace| (
                    palace.branch,
                    palace.name,
                    palace
                        .major_stars
                        .iter()
                        .map(|star| star.korean())
                        .collect::<Vec<_>>()
                ))
                .collect::<Vec<_>>(),
            vec![
                (Branch::Yin, PalaceName::Wealth, vec!["무곡", "천상"]),
                (Branch::Mao, PalaceName::Children, vec!["태양", "천량"]),
                (Branch::Chen, PalaceName::Spouse, vec!["칠살"]),
                (Branch::Si, PalaceName::Siblings, vec!["천기"]),
                (Branch::Wu, PalaceName::Life, vec!["자미"]),
                (Branch::Wei, PalaceName::Parents, vec![]),
                (Branch::Shen, PalaceName::Fortune, vec!["파군"]),
                (Branch::You, PalaceName::Property, vec![]),
                (Branch::Xu, PalaceName::Career, vec!["염정", "천부"]),
                (Branch::Hai, PalaceName::Friends, vec!["태음"]),
                (Branch::Zi, PalaceName::Travel, vec!["탐랑"]),
                (Branch::Chou, PalaceName::Health, vec!["천동", "거문"]),
            ]
        );
    }

    #[test]
    fn chart_fixtures_match_iztro_major_star_reference_cases() {
        // Cross-checked against iztro 2.5.8 only for dates where the Korean
        // lunar calendar used by rs-klc and the Chinese lunar calendar used by
        // iztro agree. Leap-month divergence is covered by the local
        // Korean-calendar policy test below instead of this fixture set.
        struct PalaceFixture {
            branch: Branch,
            name: PalaceName,
            stem: Stem,
            stars: &'static [&'static str],
        }
        struct Fixture {
            solar: (i32, u32, u32),
            hour: u32,
            life: Branch,
            body: Branch,
            bureau: &'static str,
            palaces: &'static [PalaceFixture],
        }

        let fixtures = [
            Fixture {
                solar: (1984, 2, 2),
                hour: 0,
                life: Branch::Yin,
                body: Branch::Yin,
                bureau: "화육국",
                palaces: &[
                    PalaceFixture {
                        branch: Branch::Yin,
                        name: PalaceName::Life,
                        stem: Stem::Bing,
                        stars: &[],
                    },
                    PalaceFixture {
                        branch: Branch::Mao,
                        name: PalaceName::Parents,
                        stem: Stem::Ding,
                        stars: &[],
                    },
                    PalaceFixture {
                        branch: Branch::Chen,
                        name: PalaceName::Fortune,
                        stem: Stem::Wu,
                        stars: &["천동"],
                    },
                    PalaceFixture {
                        branch: Branch::Si,
                        name: PalaceName::Property,
                        stem: Stem::Ji,
                        stars: &["무곡", "파군"],
                    },
                    PalaceFixture {
                        branch: Branch::Wu,
                        name: PalaceName::Career,
                        stem: Stem::Geng,
                        stars: &["태양"],
                    },
                    PalaceFixture {
                        branch: Branch::Wei,
                        name: PalaceName::Friends,
                        stem: Stem::Xin,
                        stars: &["천부"],
                    },
                    PalaceFixture {
                        branch: Branch::Shen,
                        name: PalaceName::Travel,
                        stem: Stem::Ren,
                        stars: &["천기", "태음"],
                    },
                    PalaceFixture {
                        branch: Branch::You,
                        name: PalaceName::Health,
                        stem: Stem::Gui,
                        stars: &["자미", "탐랑"],
                    },
                    PalaceFixture {
                        branch: Branch::Xu,
                        name: PalaceName::Wealth,
                        stem: Stem::Jia,
                        stars: &["거문"],
                    },
                    PalaceFixture {
                        branch: Branch::Hai,
                        name: PalaceName::Children,
                        stem: Stem::Yi,
                        stars: &["천상"],
                    },
                    PalaceFixture {
                        branch: Branch::Zi,
                        name: PalaceName::Spouse,
                        stem: Stem::Bing,
                        stars: &["천량"],
                    },
                    PalaceFixture {
                        branch: Branch::Chou,
                        name: PalaceName::Siblings,
                        stem: Stem::Ding,
                        stars: &["염정", "칠살"],
                    },
                ],
            },
            Fixture {
                solar: (1990, 5, 15),
                hour: 14,
                life: Branch::Xu,
                body: Branch::Zi,
                bureau: "토오국",
                palaces: &[
                    PalaceFixture {
                        branch: Branch::Yin,
                        name: PalaceName::Career,
                        stem: Stem::Wu,
                        stars: &["염정"],
                    },
                    PalaceFixture {
                        branch: Branch::Mao,
                        name: PalaceName::Friends,
                        stem: Stem::Ji,
                        stars: &[],
                    },
                    PalaceFixture {
                        branch: Branch::Chen,
                        name: PalaceName::Travel,
                        stem: Stem::Geng,
                        stars: &["파군"],
                    },
                    PalaceFixture {
                        branch: Branch::Si,
                        name: PalaceName::Health,
                        stem: Stem::Xin,
                        stars: &["천동"],
                    },
                    PalaceFixture {
                        branch: Branch::Wu,
                        name: PalaceName::Wealth,
                        stem: Stem::Ren,
                        stars: &["무곡", "천부"],
                    },
                    PalaceFixture {
                        branch: Branch::Wei,
                        name: PalaceName::Children,
                        stem: Stem::Gui,
                        stars: &["태양", "태음"],
                    },
                    PalaceFixture {
                        branch: Branch::Shen,
                        name: PalaceName::Spouse,
                        stem: Stem::Jia,
                        stars: &["탐랑"],
                    },
                    PalaceFixture {
                        branch: Branch::You,
                        name: PalaceName::Siblings,
                        stem: Stem::Yi,
                        stars: &["천기", "거문"],
                    },
                    PalaceFixture {
                        branch: Branch::Xu,
                        name: PalaceName::Life,
                        stem: Stem::Bing,
                        stars: &["자미", "천상"],
                    },
                    PalaceFixture {
                        branch: Branch::Hai,
                        name: PalaceName::Parents,
                        stem: Stem::Ding,
                        stars: &["천량"],
                    },
                    PalaceFixture {
                        branch: Branch::Zi,
                        name: PalaceName::Fortune,
                        stem: Stem::Wu,
                        stars: &["칠살"],
                    },
                    PalaceFixture {
                        branch: Branch::Chou,
                        name: PalaceName::Property,
                        stem: Stem::Ji,
                        stars: &[],
                    },
                ],
            },
            Fixture {
                solar: (1995, 12, 31),
                hour: 22,
                life: Branch::Chou,
                body: Branch::Hai,
                bureau: "화육국",
                palaces: &[
                    PalaceFixture {
                        branch: Branch::Yin,
                        name: PalaceName::Parents,
                        stem: Stem::Wu,
                        stars: &["태양", "거문"],
                    },
                    PalaceFixture {
                        branch: Branch::Mao,
                        name: PalaceName::Fortune,
                        stem: Stem::Ji,
                        stars: &["천상"],
                    },
                    PalaceFixture {
                        branch: Branch::Chen,
                        name: PalaceName::Property,
                        stem: Stem::Geng,
                        stars: &["천기", "천량"],
                    },
                    PalaceFixture {
                        branch: Branch::Si,
                        name: PalaceName::Career,
                        stem: Stem::Xin,
                        stars: &["자미", "칠살"],
                    },
                    PalaceFixture {
                        branch: Branch::Wu,
                        name: PalaceName::Friends,
                        stem: Stem::Ren,
                        stars: &[],
                    },
                    PalaceFixture {
                        branch: Branch::Wei,
                        name: PalaceName::Travel,
                        stem: Stem::Gui,
                        stars: &[],
                    },
                    PalaceFixture {
                        branch: Branch::Shen,
                        name: PalaceName::Health,
                        stem: Stem::Jia,
                        stars: &[],
                    },
                    PalaceFixture {
                        branch: Branch::You,
                        name: PalaceName::Wealth,
                        stem: Stem::Yi,
                        stars: &["염정", "파군"],
                    },
                    PalaceFixture {
                        branch: Branch::Xu,
                        name: PalaceName::Children,
                        stem: Stem::Bing,
                        stars: &[],
                    },
                    PalaceFixture {
                        branch: Branch::Hai,
                        name: PalaceName::Spouse,
                        stem: Stem::Ding,
                        stars: &["천부"],
                    },
                    PalaceFixture {
                        branch: Branch::Zi,
                        name: PalaceName::Siblings,
                        stem: Stem::Wu,
                        stars: &["천동", "태음"],
                    },
                    PalaceFixture {
                        branch: Branch::Chou,
                        name: PalaceName::Life,
                        stem: Stem::Ji,
                        stars: &["무곡", "탐랑"],
                    },
                ],
            },
            Fixture {
                solar: (2008, 2, 29),
                hour: 8,
                life: Branch::Xu,
                body: Branch::Wu,
                bureau: "수이국",
                palaces: &[
                    PalaceFixture {
                        branch: Branch::Yin,
                        name: PalaceName::Career,
                        stem: Stem::Jia,
                        stars: &["파군"],
                    },
                    PalaceFixture {
                        branch: Branch::Mao,
                        name: PalaceName::Friends,
                        stem: Stem::Yi,
                        stars: &[],
                    },
                    PalaceFixture {
                        branch: Branch::Chen,
                        name: PalaceName::Travel,
                        stem: Stem::Bing,
                        stars: &["염정", "천부"],
                    },
                    PalaceFixture {
                        branch: Branch::Si,
                        name: PalaceName::Health,
                        stem: Stem::Ding,
                        stars: &["태음"],
                    },
                    PalaceFixture {
                        branch: Branch::Wu,
                        name: PalaceName::Wealth,
                        stem: Stem::Wu,
                        stars: &["탐랑"],
                    },
                    PalaceFixture {
                        branch: Branch::Wei,
                        name: PalaceName::Children,
                        stem: Stem::Ji,
                        stars: &["천동", "거문"],
                    },
                    PalaceFixture {
                        branch: Branch::Shen,
                        name: PalaceName::Spouse,
                        stem: Stem::Geng,
                        stars: &["무곡", "천상"],
                    },
                    PalaceFixture {
                        branch: Branch::You,
                        name: PalaceName::Siblings,
                        stem: Stem::Xin,
                        stars: &["태양", "천량"],
                    },
                    PalaceFixture {
                        branch: Branch::Xu,
                        name: PalaceName::Life,
                        stem: Stem::Ren,
                        stars: &["칠살"],
                    },
                    PalaceFixture {
                        branch: Branch::Hai,
                        name: PalaceName::Parents,
                        stem: Stem::Gui,
                        stars: &["천기"],
                    },
                    PalaceFixture {
                        branch: Branch::Zi,
                        name: PalaceName::Fortune,
                        stem: Stem::Jia,
                        stars: &["자미"],
                    },
                    PalaceFixture {
                        branch: Branch::Chou,
                        name: PalaceName::Property,
                        stem: Stem::Yi,
                        stars: &[],
                    },
                ],
            },
            Fixture {
                solar: (2020, 1, 25),
                hour: 12,
                life: Branch::Shen,
                body: Branch::Shen,
                bureau: "수이국",
                palaces: &[
                    PalaceFixture {
                        branch: Branch::Yin,
                        name: PalaceName::Travel,
                        stem: Stem::Wu,
                        stars: &[],
                    },
                    PalaceFixture {
                        branch: Branch::Mao,
                        name: PalaceName::Health,
                        stem: Stem::Ji,
                        stars: &["천부"],
                    },
                    PalaceFixture {
                        branch: Branch::Chen,
                        name: PalaceName::Wealth,
                        stem: Stem::Geng,
                        stars: &["태음"],
                    },
                    PalaceFixture {
                        branch: Branch::Si,
                        name: PalaceName::Children,
                        stem: Stem::Xin,
                        stars: &["염정", "탐랑"],
                    },
                    PalaceFixture {
                        branch: Branch::Wu,
                        name: PalaceName::Spouse,
                        stem: Stem::Ren,
                        stars: &["거문"],
                    },
                    PalaceFixture {
                        branch: Branch::Wei,
                        name: PalaceName::Siblings,
                        stem: Stem::Gui,
                        stars: &["천상"],
                    },
                    PalaceFixture {
                        branch: Branch::Shen,
                        name: PalaceName::Life,
                        stem: Stem::Jia,
                        stars: &["천동", "천량"],
                    },
                    PalaceFixture {
                        branch: Branch::You,
                        name: PalaceName::Parents,
                        stem: Stem::Yi,
                        stars: &["무곡", "칠살"],
                    },
                    PalaceFixture {
                        branch: Branch::Xu,
                        name: PalaceName::Fortune,
                        stem: Stem::Bing,
                        stars: &["태양"],
                    },
                    PalaceFixture {
                        branch: Branch::Hai,
                        name: PalaceName::Property,
                        stem: Stem::Ding,
                        stars: &[],
                    },
                    PalaceFixture {
                        branch: Branch::Zi,
                        name: PalaceName::Career,
                        stem: Stem::Wu,
                        stars: &["천기"],
                    },
                    PalaceFixture {
                        branch: Branch::Chou,
                        name: PalaceName::Friends,
                        stem: Stem::Ji,
                        stars: &["자미", "파군"],
                    },
                ],
            },
            Fixture {
                solar: (2024, 2, 10),
                hour: 18,
                life: Branch::Si,
                body: Branch::Hai,
                bureau: "목삼국",
                palaces: &[
                    PalaceFixture {
                        branch: Branch::Yin,
                        name: PalaceName::Children,
                        stem: Stem::Bing,
                        stars: &["탐랑"],
                    },
                    PalaceFixture {
                        branch: Branch::Mao,
                        name: PalaceName::Spouse,
                        stem: Stem::Ding,
                        stars: &["천기", "거문"],
                    },
                    PalaceFixture {
                        branch: Branch::Chen,
                        name: PalaceName::Siblings,
                        stem: Stem::Wu,
                        stars: &["자미", "천상"],
                    },
                    PalaceFixture {
                        branch: Branch::Si,
                        name: PalaceName::Life,
                        stem: Stem::Ji,
                        stars: &["천량"],
                    },
                    PalaceFixture {
                        branch: Branch::Wu,
                        name: PalaceName::Parents,
                        stem: Stem::Geng,
                        stars: &["칠살"],
                    },
                    PalaceFixture {
                        branch: Branch::Wei,
                        name: PalaceName::Fortune,
                        stem: Stem::Xin,
                        stars: &[],
                    },
                    PalaceFixture {
                        branch: Branch::Shen,
                        name: PalaceName::Property,
                        stem: Stem::Ren,
                        stars: &["염정"],
                    },
                    PalaceFixture {
                        branch: Branch::You,
                        name: PalaceName::Career,
                        stem: Stem::Gui,
                        stars: &[],
                    },
                    PalaceFixture {
                        branch: Branch::Xu,
                        name: PalaceName::Friends,
                        stem: Stem::Jia,
                        stars: &["파군"],
                    },
                    PalaceFixture {
                        branch: Branch::Hai,
                        name: PalaceName::Travel,
                        stem: Stem::Yi,
                        stars: &["천동"],
                    },
                    PalaceFixture {
                        branch: Branch::Zi,
                        name: PalaceName::Health,
                        stem: Stem::Bing,
                        stars: &["무곡", "천부"],
                    },
                    PalaceFixture {
                        branch: Branch::Chou,
                        name: PalaceName::Wealth,
                        stem: Stem::Ding,
                        stars: &["태양", "태음"],
                    },
                ],
            },
        ];

        for fixture in fixtures {
            let birth = normalize_birth_date(
                fixture.solar.0,
                fixture.solar.1,
                fixture.solar.2,
                Some("solar"),
                false,
            )
            .unwrap();
            let chart = calculate_chart(ChartInput {
                birth,
                birth_time: format!("{:02}:00", fixture.hour),
                hour: fixture.hour,
                minute: 0,
                target_year: None,
                gender: None,
            })
            .unwrap();

            assert_eq!(chart.life_palace, fixture.life);
            assert_eq!(chart.body_palace, fixture.body);
            assert_eq!(chart.five_element_bureau.label, fixture.bureau);
            for (actual, expected) in chart.palaces.iter().zip(fixture.palaces.iter()) {
                assert_eq!(actual.branch, expected.branch);
                assert_eq!(actual.name, expected.name);
                assert_eq!(actual.stem, expected.stem);
                assert_eq!(
                    actual
                        .major_stars
                        .iter()
                        .map(|star| star.korean())
                        .collect::<Vec<_>>(),
                    expected.stars
                );
            }
        }
    }

    #[test]
    fn late_zi_hour_policy_keeps_civil_lunar_day() {
        let birth = normalize_birth_date(2022, 6, 16, Some("lunar"), false).unwrap();
        let chart = calculate_chart(ChartInput {
            birth,
            birth_time: "23:30".to_string(),
            hour: 23,
            minute: 30,
            target_year: None,
            gender: None,
        })
        .unwrap();

        assert_eq!(chart.birth.hour_branch, Branch::Zi);
        assert_eq!(chart.birth.lunar_date, "2022-06-16");
        assert_eq!(chart.ziwei_star, Branch::You);
    }

    #[test]
    fn leap_month_policy_uses_same_lunar_month_number() {
        let regular_birth = normalize_birth_date(2023, 2, 1, Some("lunar"), false).unwrap();
        let leap_birth = normalize_birth_date(2023, 2, 1, Some("lunar"), true).unwrap();
        let regular_chart = calculate_chart(ChartInput {
            birth: regular_birth,
            birth_time: "09:00".to_string(),
            hour: 9,
            minute: 0,
            target_year: None,
            gender: None,
        })
        .unwrap();
        let leap_chart = calculate_chart(ChartInput {
            birth: leap_birth,
            birth_time: "09:00".to_string(),
            hour: 9,
            minute: 0,
            target_year: None,
            gender: None,
        })
        .unwrap();

        assert!(!regular_chart.birth.is_lunar_leap_month);
        assert!(leap_chart.birth.is_lunar_leap_month);
        assert_ne!(regular_chart.birth.solar_date, leap_chart.birth.solar_date);
        assert_eq!(regular_chart.life_palace, leap_chart.life_palace);
        assert_eq!(regular_chart.body_palace, leap_chart.body_palace);
        assert_eq!(
            regular_chart.five_element_bureau,
            leap_chart.five_element_bureau
        );
        assert_eq!(regular_chart.ziwei_star, leap_chart.ziwei_star);
    }

    #[test]
    fn korean_lunar_calendar_policy_documents_2012_leap_month_difference() {
        let birth = normalize_birth_date(2012, 6, 6, Some("solar"), false).unwrap();
        let chart = calculate_chart(ChartInput {
            birth,
            birth_time: "10:00".to_string(),
            hour: 10,
            minute: 0,
            target_year: None,
            gender: None,
        })
        .unwrap();

        assert_eq!(chart.birth.lunar_date, "2012-04-17");
        assert!(!chart.birth.is_lunar_leap_month);
        assert_eq!(chart.life_palace, Branch::Zi);
        assert_eq!(chart.body_palace, Branch::Xu);
        assert_eq!(chart.five_element_bureau.label, "목삼국");
    }

    #[test]
    fn chart_rejects_invalid_hour_minute_and_lunar_day() {
        let birth = normalize_birth_date(1990, 5, 15, Some("solar"), false).unwrap();

        assert!(matches!(
            calculate_chart(ChartInput {
                birth: birth.clone(),
                birth_time: "24:00".to_string(),
                hour: 24,
                minute: 0,
                target_year: None,
                gender: None,
            }),
            Err(ChartError::InvalidBirthHour(24))
        ));
        assert!(matches!(
            calculate_chart(ChartInput {
                birth: birth.clone(),
                birth_time: "12:60".to_string(),
                hour: 12,
                minute: 60,
                target_year: None,
                gender: None,
            }),
            Err(ChartError::InvalidBirthMinute(60))
        ));

        let mut invalid_lunar_day_birth = birth;
        invalid_lunar_day_birth.lunar_day = 31;
        assert!(matches!(
            calculate_chart(ChartInput {
                birth: invalid_lunar_day_birth,
                birth_time: "12:00".to_string(),
                hour: 12,
                minute: 0,
                target_year: None,
                gender: None,
            }),
            Err(ChartError::InvalidLunarDay(31))
        ));
    }
}
