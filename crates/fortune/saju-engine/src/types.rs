use serde::Serialize;
use std::fmt;

/// 천간 (Heavenly Stems)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Stem {
    Gap,    // 갑 甲 Wood+
    Eul,    // 을 乙 Wood-
    Byeong, // 병 丙 Fire+
    Jeong,  // 정 丁 Fire-
    Mu,     // 무 戊 Earth+
    Gi,     // 기 己 Earth-
    Gyeong, // 경 庚 Metal+
    Sin,    // 신 辛 Metal-
    Im,     // 임 壬 Water+
    Gye,    // 계 癸 Water-
}

/// 지지 (Earthly Branches)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Branch {
    Ja,   // 자 子 Rat
    Chuk, // 축 丑 Ox
    In,   // 인 寅 Tiger
    Myo,  // 묘 卯 Rabbit
    Jin,  // 진 辰 Dragon
    Sa,   // 사 巳 Snake
    O,    // 오 午 Horse
    Mi,   // 미 未 Goat
    Sin,  // 신 申 Monkey
    Yu,   // 유 酉 Rooster
    Sul,  // 술 戌 Dog
    Hae,  // 해 亥 Pig
}

/// 오행 (Five Elements)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Element {
    Wood,  // 목
    Fire,  // 화
    Earth, // 토
    Metal, // 금
    Water, // 수
}

/// 음양 (Yin-Yang)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Polarity {
    Yang, // 양
    Yin,  // 음
}

/// 십신 (Ten Gods)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum TenGod {
    Bigyeon,   // 비견 (比肩) - Companion
    Geupjae,   // 겁재 (劫財) - Rob Wealth
    Sikshin,   // 식신 (食神) - Eating God
    Sanggwan,  // 상관 (傷官) - Hurting Officer
    Pyeonjae,  // 편재 (偏財) - Indirect Wealth
    Jeongjae,  // 정재 (正財) - Direct Wealth
    Pyeongwan, // 편관 (偏官) - Indirect Officer
    Jeonggwan, // 정관 (正官) - Direct Officer
    Pyeonin,   // 편인 (偏印) - Indirect Seal
    Jeongin,   // 정인 (正印) - Direct Seal
}

/// 사주 기둥 하나 (천간 + 지지)
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Pillar {
    pub stem: Stem,
    pub branch: Branch,
}

/// 사주팔자 (네 기둥)
#[derive(Debug, Clone, Serialize)]
pub struct FourPillars {
    pub year: Pillar,
    pub month: Pillar,
    pub day: Pillar,
    pub hour: Pillar,
}

/// 오행 분포
#[derive(Debug, Clone, Serialize)]
pub struct ElementBalance {
    pub wood: u8,
    pub fire: u8,
    pub earth: u8,
    pub metal: u8,
    pub water: u8,
}

impl Stem {
    pub const ALL: [Stem; 10] = [
        Stem::Gap,
        Stem::Eul,
        Stem::Byeong,
        Stem::Jeong,
        Stem::Mu,
        Stem::Gi,
        Stem::Gyeong,
        Stem::Sin,
        Stem::Im,
        Stem::Gye,
    ];

    pub fn from_index(i: usize) -> Stem {
        Stem::ALL[i % 10]
    }

    pub fn index(self) -> usize {
        Stem::ALL.iter().position(|&s| s == self).unwrap()
    }

    pub fn element(self) -> Element {
        match self {
            Stem::Gap | Stem::Eul => Element::Wood,
            Stem::Byeong | Stem::Jeong => Element::Fire,
            Stem::Mu | Stem::Gi => Element::Earth,
            Stem::Gyeong | Stem::Sin => Element::Metal,
            Stem::Im | Stem::Gye => Element::Water,
        }
    }

    pub fn polarity(self) -> Polarity {
        match self {
            Stem::Gap | Stem::Byeong | Stem::Mu | Stem::Gyeong | Stem::Im => Polarity::Yang,
            _ => Polarity::Yin,
        }
    }

    pub fn korean(self) -> &'static str {
        match self {
            Stem::Gap => "갑",
            Stem::Eul => "을",
            Stem::Byeong => "병",
            Stem::Jeong => "정",
            Stem::Mu => "무",
            Stem::Gi => "기",
            Stem::Gyeong => "경",
            Stem::Sin => "신",
            Stem::Im => "임",
            Stem::Gye => "계",
        }
    }

    pub fn hanja(self) -> &'static str {
        match self {
            Stem::Gap => "甲",
            Stem::Eul => "乙",
            Stem::Byeong => "丙",
            Stem::Jeong => "丁",
            Stem::Mu => "戊",
            Stem::Gi => "己",
            Stem::Gyeong => "庚",
            Stem::Sin => "辛",
            Stem::Im => "壬",
            Stem::Gye => "癸",
        }
    }
}

impl Branch {
    pub const ALL: [Branch; 12] = [
        Branch::Ja,
        Branch::Chuk,
        Branch::In,
        Branch::Myo,
        Branch::Jin,
        Branch::Sa,
        Branch::O,
        Branch::Mi,
        Branch::Sin,
        Branch::Yu,
        Branch::Sul,
        Branch::Hae,
    ];

    pub fn from_index(i: usize) -> Branch {
        Branch::ALL[i % 12]
    }

    pub fn index(self) -> usize {
        Branch::ALL.iter().position(|&b| b == self).unwrap()
    }

    pub fn element(self) -> Element {
        match self {
            Branch::In | Branch::Myo => Element::Wood,
            Branch::Sa | Branch::O => Element::Fire,
            Branch::Jin | Branch::Mi | Branch::Sul | Branch::Chuk => Element::Earth,
            Branch::Sin | Branch::Yu => Element::Metal,
            Branch::Hae | Branch::Ja => Element::Water,
        }
    }

    pub fn korean(self) -> &'static str {
        match self {
            Branch::Ja => "자",
            Branch::Chuk => "축",
            Branch::In => "인",
            Branch::Myo => "묘",
            Branch::Jin => "진",
            Branch::Sa => "사",
            Branch::O => "오",
            Branch::Mi => "미",
            Branch::Sin => "신",
            Branch::Yu => "유",
            Branch::Sul => "술",
            Branch::Hae => "해",
        }
    }

    pub fn hanja(self) -> &'static str {
        match self {
            Branch::Ja => "子",
            Branch::Chuk => "丑",
            Branch::In => "寅",
            Branch::Myo => "卯",
            Branch::Jin => "辰",
            Branch::Sa => "巳",
            Branch::O => "午",
            Branch::Mi => "未",
            Branch::Sin => "申",
            Branch::Yu => "酉",
            Branch::Sul => "戌",
            Branch::Hae => "亥",
        }
    }

    pub fn animal(self) -> &'static str {
        match self {
            Branch::Ja => "쥐",
            Branch::Chuk => "소",
            Branch::In => "호랑이",
            Branch::Myo => "토끼",
            Branch::Jin => "용",
            Branch::Sa => "뱀",
            Branch::O => "말",
            Branch::Mi => "양",
            Branch::Sin => "원숭이",
            Branch::Yu => "닭",
            Branch::Sul => "개",
            Branch::Hae => "돼지",
        }
    }
}

impl Element {
    pub fn korean(self) -> &'static str {
        match self {
            Element::Wood => "목(木)",
            Element::Fire => "화(火)",
            Element::Earth => "토(土)",
            Element::Metal => "금(金)",
            Element::Water => "수(水)",
        }
    }
}

impl Polarity {
    pub fn korean(self) -> &'static str {
        match self {
            Polarity::Yang => "양(陽)",
            Polarity::Yin => "음(陰)",
        }
    }
}

impl TenGod {
    pub fn korean(self) -> &'static str {
        match self {
            TenGod::Bigyeon => "비견(比肩)",
            TenGod::Geupjae => "겁재(劫財)",
            TenGod::Sikshin => "식신(食神)",
            TenGod::Sanggwan => "상관(傷官)",
            TenGod::Pyeonjae => "편재(偏財)",
            TenGod::Jeongjae => "정재(正財)",
            TenGod::Pyeongwan => "편관(偏官)",
            TenGod::Jeonggwan => "정관(正官)",
            TenGod::Pyeonin => "편인(偏印)",
            TenGod::Jeongin => "정인(正印)",
        }
    }
}

impl fmt::Display for Pillar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{} ({}{})",
            self.stem.korean(),
            self.branch.korean(),
            self.stem.hanja(),
            self.branch.hanja()
        )
    }
}

impl Pillar {
    pub fn new(stem: Stem, branch: Branch) -> Self {
        Pillar { stem, branch }
    }
}

impl ElementBalance {
    pub fn from_pillars(pillars: &FourPillars) -> Self {
        Self::from_pillars_with_hour(pillars, true)
    }

    pub fn from_pillars_with_hour(pillars: &FourPillars, include_hour: bool) -> Self {
        let mut balance = ElementBalance {
            wood: 0,
            fire: 0,
            earth: 0,
            metal: 0,
            water: 0,
        };
        let mut stems: Vec<Stem> = vec![pillars.year.stem, pillars.month.stem, pillars.day.stem];
        let mut branches: Vec<Branch> = vec![
            pillars.year.branch,
            pillars.month.branch,
            pillars.day.branch,
        ];
        if include_hour {
            stems.push(pillars.hour.stem);
            branches.push(pillars.hour.branch);
        }

        for s in &stems {
            balance.add_element(s.element(), 1);
        }
        for b in &branches {
            balance.add_element(b.element(), 1);
        }
        balance
    }

    fn add_element(&mut self, elem: Element, count: u8) {
        match elem {
            Element::Wood => self.wood += count,
            Element::Fire => self.fire += count,
            Element::Earth => self.earth += count,
            Element::Metal => self.metal += count,
            Element::Water => self.water += count,
        }
    }

    pub fn dominant(&self) -> Element {
        let pairs = [
            (self.wood, Element::Wood),
            (self.fire, Element::Fire),
            (self.earth, Element::Earth),
            (self.metal, Element::Metal),
            (self.water, Element::Water),
        ];
        pairs.iter().max_by_key(|p| p.0).unwrap().1
    }

    pub fn weakest(&self) -> Element {
        let pairs = [
            (self.wood, Element::Wood),
            (self.fire, Element::Fire),
            (self.earth, Element::Earth),
            (self.metal, Element::Metal),
            (self.water, Element::Water),
        ];
        pairs.iter().min_by_key(|p| p.0).unwrap().1
    }
}
