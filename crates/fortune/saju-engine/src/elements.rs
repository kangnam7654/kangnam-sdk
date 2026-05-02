use super::types::Element;

/// 상생 관계: A가 B를 생(生)하는가?
pub fn generates(a: Element, b: Element) -> bool {
    matches!(
        (a, b),
        (Element::Wood, Element::Fire)
            | (Element::Fire, Element::Earth)
            | (Element::Earth, Element::Metal)
            | (Element::Metal, Element::Water)
            | (Element::Water, Element::Wood)
    )
}

/// 상극 관계: A가 B를 극(克)하는가?
pub fn controls(a: Element, b: Element) -> bool {
    matches!(
        (a, b),
        (Element::Wood, Element::Earth)
            | (Element::Earth, Element::Water)
            | (Element::Water, Element::Fire)
            | (Element::Fire, Element::Metal)
            | (Element::Metal, Element::Wood)
    )
}

/// 두 오행의 관계
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementRelation {
    Same,       // 같은 오행 (비화)
    Generates,  // 내가 생해줌 (설기)
    Generated,  // 나를 생해줌 (인성)
    Controls,   // 내가 극함 (재성)
    Controlled, // 나를 극함 (관성)
}

/// day_master 기준으로 target과의 관계
pub fn relation(day_master: Element, target: Element) -> ElementRelation {
    if day_master == target {
        ElementRelation::Same
    } else if generates(day_master, target) {
        ElementRelation::Generates
    } else if generates(target, day_master) {
        ElementRelation::Generated
    } else if controls(day_master, target) {
        ElementRelation::Controls
    } else {
        ElementRelation::Controlled
    }
}

impl ElementRelation {
    pub fn korean(self) -> &'static str {
        match self {
            ElementRelation::Same => "비화(比和)",
            ElementRelation::Generates => "설기(洩氣)",
            ElementRelation::Generated => "인성(印星)",
            ElementRelation::Controls => "재성(財星)",
            ElementRelation::Controlled => "관성(官星)",
        }
    }

    /// 오늘의 운세 점수 가중치 (0-100 기준)
    pub fn daily_score_base(self) -> i32 {
        match self {
            ElementRelation::Generated => 85,  // 나를 생해줌 → 좋은 날
            ElementRelation::Same => 75,       // 비화 → 무난한 날
            ElementRelation::Generates => 65,  // 설기 → 에너지 소모
            ElementRelation::Controls => 70,   // 재성 → 재물운 있으나 노력 필요
            ElementRelation::Controlled => 55, // 관성 → 도전적인 날
        }
    }
}
