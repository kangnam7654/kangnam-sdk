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

    /// 천간 상징 — 한 줄 자연물 비유 (예: 갑목 = "큰 소나무").
    /// UI에서 일간 카드의 헤드라인으로 사용.
    pub fn symbol(self) -> &'static str {
        match self {
            Stem::Gap => "큰 소나무",
            Stem::Eul => "넝쿨과 화초",
            Stem::Byeong => "한낮의 태양",
            Stem::Jeong => "촛불과 별빛",
            Stem::Mu => "거대한 산",
            Stem::Gi => "비옥한 논밭",
            Stem::Gyeong => "다듬지 않은 무쇠",
            Stem::Sin => "정련된 보석",
            Stem::Im => "깊은 바다",
            Stem::Gye => "이슬과 옹달샘",
        }
    }

    /// 천간 심리 키워드 3종 — 일간 카드의 보조 카피.
    /// 첫 번째는 핵심 성향, 두 번째는 행동 양식, 세 번째는 대인 인상.
    pub fn psyche_keywords(self) -> [&'static str; 3] {
        match self {
            Stem::Gap => ["곧은 의지", "창의적 추진", "리더의 직진"],
            Stem::Eul => ["유연한 적응", "끈질긴 생명력", "부드러운 침투"],
            Stem::Byeong => ["명랑한 열정", "강한 존재감", "만물을 비추는 빛"],
            Stem::Jeong => ["집중된 따뜻함", "은은한 헌신", "어둠 속 희망"],
            Stem::Mu => ["우직한 포용", "흔들리지 않는 중재", "신뢰의 무게"],
            Stem::Gi => ["섬세한 배려", "생명을 기르는 자양", "실용적 지혜"],
            Stem::Gyeong => ["결단의 단호함", "우직한 파괴력", "가공되지 않은 순수"],
            Stem::Sin => ["세련된 완벽주의", "날카로운 안목", "정밀한 예민함"],
            Stem::Im => ["깊은 지혜", "모든 것을 수용", "유연한 융통성"],
            Stem::Gye => ["겸손한 섬세함", "스며드는 영향력", "생명력의 원천"],
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

    /// 지지 상징 — 계절·시간대 자연 비유.
    /// 12지지 순환의 어느 지점인지를 한 줄로 드러낸다.
    pub fn symbol(self) -> &'static str {
        match self {
            Branch::Ja => "한밤의 시작",
            Branch::Chuk => "새벽 직전의 갈무리",
            Branch::In => "봄의 첫 박력",
            Branch::Myo => "한낮 봄의 절정",
            Branch::Jin => "봄을 품은 변화의 그릇",
            Branch::Sa => "여름의 영민한 태동",
            Branch::O => "한낮의 정점",
            Branch::Mi => "늦여름의 안식",
            Branch::Sin => "가을의 영리한 시작",
            Branch::Yu => "가을의 결정",
            Branch::Sul => "가을의 충직한 갈무리",
            Branch::Hae => "겨울의 너그러운 시작",
        }
    }

    /// 지지 심리 키워드 3종.
    /// 천간이 정신·관념의 지향이라면, 지지는 환경과 행동에 가까운 톤.
    pub fn psyche_keywords(self) -> [&'static str; 3] {
        match self {
            Branch::Ja => ["총명한 직관", "비밀스러운 깊이", "조용한 시작"],
            Branch::Chuk => ["묵묵한 인내", "결실의 저장", "은근한 끈기"],
            Branch::In => ["박력있는 출발", "새로운 도약", "거침없는 의욕"],
            Branch::Myo => ["부드러운 성장", "섬세한 감수성", "예민한 감각"],
            Branch::Jin => ["변화를 품은 잠재력", "야심의 그릇", "상상하는 힘"],
            Branch::Sa => ["영민한 통찰", "은밀한 매혹", "정교한 사고"],
            Branch::O => ["자유로운 활력", "빛나는 사교성", "솔직한 표현"],
            Branch::Mi => ["따뜻한 정", "부드러운 헌신", "감성의 깊이"],
            Branch::Sin => ["영리한 기지", "다재다능한 적응", "빠른 판단"],
            Branch::Yu => ["빈틈없는 정확함", "단호한 명료함", "예리한 비판"],
            Branch::Sul => ["충직한 의리", "지키는 자의 무게", "신중한 헌신"],
            Branch::Hae => ["풍부한 수용", "너그러운 깊이", "포용하는 지혜"],
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

#[cfg(test)]
mod copy_tests {
    use super::*;

    /// 모든 천간이 (a) 비어있지 않은 symbol과 (b) 정확히 3개의 비어있지 않은 키워드를 갖는다.
    /// 카피 누락이 컴파일 시점에 잡히도록 enum match로 구현되어 있지만,
    /// 빈 문자열을 막기 위한 런타임 가드.
    #[test]
    fn all_stems_have_symbol_and_three_keywords() {
        for &stem in Stem::ALL.iter() {
            let symbol = stem.symbol();
            assert!(!symbol.is_empty(), "{:?} symbol is empty", stem);

            let kws = stem.psyche_keywords();
            assert_eq!(kws.len(), 3);
            for (i, kw) in kws.iter().enumerate() {
                assert!(!kw.is_empty(), "{:?} keyword[{}] is empty", stem, i);
            }
        }
    }

    /// 모든 지지가 (a) 비어있지 않은 symbol과 (b) 정확히 3개의 비어있지 않은 키워드를 갖는다.
    #[test]
    fn all_branches_have_symbol_and_three_keywords() {
        for &branch in Branch::ALL.iter() {
            let symbol = branch.symbol();
            assert!(!symbol.is_empty(), "{:?} symbol is empty", branch);

            let kws = branch.psyche_keywords();
            assert_eq!(kws.len(), 3);
            for (i, kw) in kws.iter().enumerate() {
                assert!(!kw.is_empty(), "{:?} keyword[{}] is empty", branch, i);
            }
        }
    }

    /// 핵심 fixture: 갑목 = "큰 소나무" / 곧은 의지 — 보고서 §3 톤 잠금.
    /// 다른 갑목 비유로 갈아끼우려면 이 테스트를 의도적으로 깨야 한다.
    #[test]
    fn gap_stem_anchor_copy() {
        assert_eq!(Stem::Gap.symbol(), "큰 소나무");
        assert_eq!(Stem::Gap.psyche_keywords()[0], "곧은 의지");
    }

    /// 핵심 fixture: 인지 = "봄의 첫 박력" — 12지지 순환 톤 잠금.
    #[test]
    fn in_branch_anchor_copy() {
        assert_eq!(Branch::In.symbol(), "봄의 첫 박력");
        assert_eq!(Branch::In.psyche_keywords()[0], "박력있는 출발");
    }
}
