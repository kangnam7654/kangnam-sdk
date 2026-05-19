use crate::calendar::NormalizedBirthDate;
use crate::types::{
    BirthData, Branch, Element, FiveElementBureau, MajorStar, Palace, PalaceName, StarPlacement,
    Stem, ZiweiChart,
};
use std::fmt;

pub const ZIWEI_SCHEMA_VERSION: &str = "ziwei_chart_v1";

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

    let palaces = CHART_BRANCHES
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
                is_life_palace: idx == life_index,
                is_body_palace: idx == body_index,
            }
        })
        .collect();

    Ok(ZiweiChart {
        chart_type: "ziwei".to_string(),
        schema_version: ZIWEI_SCHEMA_VERSION.to_string(),
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

fn wrap(index: i32) -> usize {
    index.rem_euclid(12) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::normalize_birth_date;

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
        let birth = normalize_birth_date(1990, 5, 15, Some("solar"), false).unwrap();
        let chart = calculate_chart(ChartInput {
            birth,
            birth_time: "14:30".to_string(),
            hour: 14,
            minute: 30,
        })
        .unwrap();

        assert_eq!(chart.palaces.len(), 12);
        assert_eq!(chart.major_star_placements.len(), 14);
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
    fn chart_fixture_locks_lunar_2022_06_16_wei_hour() {
        let birth = normalize_birth_date(2022, 6, 16, Some("lunar"), false).unwrap();
        let chart = calculate_chart(ChartInput {
            birth,
            birth_time: "14:30".to_string(),
            hour: 14,
            minute: 30,
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
        })
        .unwrap();
        let leap_chart = calculate_chart(ChartInput {
            birth: leap_birth,
            birth_time: "09:00".to_string(),
            hour: 9,
            minute: 0,
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
            }),
            Err(ChartError::InvalidBirthHour(24))
        ));
        assert!(matches!(
            calculate_chart(ChartInput {
                birth: birth.clone(),
                birth_time: "12:60".to_string(),
                hour: 12,
                minute: 60,
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
            }),
            Err(ChartError::InvalidLunarDay(31))
        ));
    }
}
