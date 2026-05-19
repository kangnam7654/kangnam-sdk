use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Branch {
    Zi,
    Chou,
    Yin,
    Mao,
    Chen,
    Si,
    Wu,
    Wei,
    Shen,
    You,
    Xu,
    Hai,
}

impl Branch {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Zi => "zi",
            Self::Chou => "chou",
            Self::Yin => "yin",
            Self::Mao => "mao",
            Self::Chen => "chen",
            Self::Si => "si",
            Self::Wu => "wu",
            Self::Wei => "wei",
            Self::Shen => "shen",
            Self::You => "you",
            Self::Xu => "xu",
            Self::Hai => "hai",
        }
    }

    pub const fn hanja(self) -> &'static str {
        match self {
            Self::Zi => "子",
            Self::Chou => "丑",
            Self::Yin => "寅",
            Self::Mao => "卯",
            Self::Chen => "辰",
            Self::Si => "巳",
            Self::Wu => "午",
            Self::Wei => "未",
            Self::Shen => "申",
            Self::You => "酉",
            Self::Xu => "戌",
            Self::Hai => "亥",
        }
    }

    pub const fn korean(self) -> &'static str {
        match self {
            Self::Zi => "자",
            Self::Chou => "축",
            Self::Yin => "인",
            Self::Mao => "묘",
            Self::Chen => "진",
            Self::Si => "사",
            Self::Wu => "오",
            Self::Wei => "미",
            Self::Shen => "신",
            Self::You => "유",
            Self::Xu => "술",
            Self::Hai => "해",
        }
    }

    pub const fn chart_index(self) -> usize {
        match self {
            Self::Yin => 0,
            Self::Mao => 1,
            Self::Chen => 2,
            Self::Si => 3,
            Self::Wu => 4,
            Self::Wei => 5,
            Self::Shen => 6,
            Self::You => 7,
            Self::Xu => 8,
            Self::Hai => 9,
            Self::Zi => 10,
            Self::Chou => 11,
        }
    }

    pub const fn cycle_index(self) -> usize {
        match self {
            Self::Zi => 0,
            Self::Chou => 1,
            Self::Yin => 2,
            Self::Mao => 3,
            Self::Chen => 4,
            Self::Si => 5,
            Self::Wu => 6,
            Self::Wei => 7,
            Self::Shen => 8,
            Self::You => 9,
            Self::Xu => 10,
            Self::Hai => 11,
        }
    }
}

impl fmt::Display for Branch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.korean())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stem {
    Jia,
    Yi,
    Bing,
    Ding,
    Wu,
    Ji,
    Geng,
    Xin,
    Ren,
    Gui,
}

impl Stem {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Jia => "jia",
            Self::Yi => "yi",
            Self::Bing => "bing",
            Self::Ding => "ding",
            Self::Wu => "wu",
            Self::Ji => "ji",
            Self::Geng => "geng",
            Self::Xin => "xin",
            Self::Ren => "ren",
            Self::Gui => "gui",
        }
    }

    pub const fn hanja(self) -> &'static str {
        match self {
            Self::Jia => "甲",
            Self::Yi => "乙",
            Self::Bing => "丙",
            Self::Ding => "丁",
            Self::Wu => "戊",
            Self::Ji => "己",
            Self::Geng => "庚",
            Self::Xin => "辛",
            Self::Ren => "壬",
            Self::Gui => "癸",
        }
    }

    pub const fn korean(self) -> &'static str {
        match self {
            Self::Jia => "갑",
            Self::Yi => "을",
            Self::Bing => "병",
            Self::Ding => "정",
            Self::Wu => "무",
            Self::Ji => "기",
            Self::Geng => "경",
            Self::Xin => "신",
            Self::Ren => "임",
            Self::Gui => "계",
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Self::Jia => 0,
            Self::Yi => 1,
            Self::Bing => 2,
            Self::Ding => 3,
            Self::Wu => 4,
            Self::Ji => 5,
            Self::Geng => 6,
            Self::Xin => 7,
            Self::Ren => 8,
            Self::Gui => 9,
        }
    }
}

impl fmt::Display for Stem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.korean())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Element {
    Water,
    Wood,
    Metal,
    Earth,
    Fire,
}

impl Element {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Water => "water",
            Self::Wood => "wood",
            Self::Metal => "metal",
            Self::Earth => "earth",
            Self::Fire => "fire",
        }
    }

    pub const fn korean(self) -> &'static str {
        match self {
            Self::Water => "수",
            Self::Wood => "목",
            Self::Metal => "금",
            Self::Earth => "토",
            Self::Fire => "화",
        }
    }

    pub const fn bureau_number(self) -> u8 {
        match self {
            Self::Water => 2,
            Self::Wood => 3,
            Self::Metal => 4,
            Self::Earth => 5,
            Self::Fire => 6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PalaceName {
    Life,
    Siblings,
    Spouse,
    Children,
    Wealth,
    Health,
    Travel,
    Friends,
    Career,
    Property,
    Fortune,
    Parents,
}

impl PalaceName {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Life => "life",
            Self::Siblings => "siblings",
            Self::Spouse => "spouse",
            Self::Children => "children",
            Self::Wealth => "wealth",
            Self::Health => "health",
            Self::Travel => "travel",
            Self::Friends => "friends",
            Self::Career => "career",
            Self::Property => "property",
            Self::Fortune => "fortune",
            Self::Parents => "parents",
        }
    }

    pub const fn korean(self) -> &'static str {
        match self {
            Self::Life => "명궁",
            Self::Siblings => "형제궁",
            Self::Spouse => "부부궁",
            Self::Children => "자녀궁",
            Self::Wealth => "재백궁",
            Self::Health => "질액궁",
            Self::Travel => "천이궁",
            Self::Friends => "노복궁",
            Self::Career => "관록궁",
            Self::Property => "전택궁",
            Self::Fortune => "복덕궁",
            Self::Parents => "부모궁",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MajorStar {
    ZiWei,
    TianJi,
    TaiYang,
    WuQu,
    TianTong,
    LianZhen,
    TianFu,
    TaiYin,
    TanLang,
    JuMen,
    TianXiang,
    TianLiang,
    QiSha,
    PoJun,
}

impl MajorStar {
    pub const fn code(self) -> &'static str {
        match self {
            Self::ZiWei => "zi_wei",
            Self::TianJi => "tian_ji",
            Self::TaiYang => "tai_yang",
            Self::WuQu => "wu_qu",
            Self::TianTong => "tian_tong",
            Self::LianZhen => "lian_zhen",
            Self::TianFu => "tian_fu",
            Self::TaiYin => "tai_yin",
            Self::TanLang => "tan_lang",
            Self::JuMen => "ju_men",
            Self::TianXiang => "tian_xiang",
            Self::TianLiang => "tian_liang",
            Self::QiSha => "qi_sha",
            Self::PoJun => "po_jun",
        }
    }

    pub const fn hanja(self) -> &'static str {
        match self {
            Self::ZiWei => "紫微",
            Self::TianJi => "天機",
            Self::TaiYang => "太陽",
            Self::WuQu => "武曲",
            Self::TianTong => "天同",
            Self::LianZhen => "廉貞",
            Self::TianFu => "天府",
            Self::TaiYin => "太陰",
            Self::TanLang => "貪狼",
            Self::JuMen => "巨門",
            Self::TianXiang => "天相",
            Self::TianLiang => "天梁",
            Self::QiSha => "七殺",
            Self::PoJun => "破軍",
        }
    }

    pub const fn korean(self) -> &'static str {
        match self {
            Self::ZiWei => "자미",
            Self::TianJi => "천기",
            Self::TaiYang => "태양",
            Self::WuQu => "무곡",
            Self::TianTong => "천동",
            Self::LianZhen => "염정",
            Self::TianFu => "천부",
            Self::TaiYin => "태음",
            Self::TanLang => "탐랑",
            Self::JuMen => "거문",
            Self::TianXiang => "천상",
            Self::TianLiang => "천량",
            Self::QiSha => "칠살",
            Self::PoJun => "파군",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarPlacement {
    pub star: MajorStar,
    pub branch: Branch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Palace {
    pub branch: Branch,
    pub stem: Stem,
    pub name: PalaceName,
    pub major_stars: Vec<MajorStar>,
    pub is_life_palace: bool,
    pub is_body_palace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FiveElementBureau {
    pub element: Element,
    pub number: u8,
    pub label: String,
    pub na_yin: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BirthData {
    pub original_date: String,
    pub solar_date: String,
    pub lunar_date: String,
    pub birth_time: String,
    pub hour_branch: Branch,
    pub calendar_type: String,
    pub is_lunar_leap_month: bool,
    pub was_lunar_converted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZiweiChart {
    pub chart_type: String,
    pub schema_version: String,
    pub birth: BirthData,
    pub life_palace: Branch,
    pub body_palace: Branch,
    pub five_element_bureau: FiveElementBureau,
    pub ziwei_star: Branch,
    pub tianfu_star: Branch,
    pub palaces: Vec<Palace>,
    pub major_star_placements: Vec<StarPlacement>,
}
