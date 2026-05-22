use serde::Serialize;
use std::fmt;

/// 아르카나 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ArcanaType {
    Major,
    Minor,
}

/// 마이너 아르카나 수트
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Suit {
    Wands,     // 완드 (봉)
    Cups,      // 컵 (잔)
    Swords,    // 소드 (검)
    Pentacles, // 펜타클 (동전)
}

/// 타로 원소
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TarotElement {
    Fire,  // 불
    Water, // 물
    Air,   // 바람
    Earth, // 땅
}

/// 오행 매핑
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Ohang {
    Wood,  // 목(木) - Air
    Fire,  // 화(火) - Fire
    Earth, // 토(土) - 중앙/전환
    Metal, // 금(金) - Earth
    Water, // 수(水) - Water
}

impl Ohang {
    /// 오행의 한국어 표기
    pub fn korean(self) -> &'static str {
        match self {
            Ohang::Wood => "목(木)",
            Ohang::Fire => "화(火)",
            Ohang::Earth => "토(土)",
            Ohang::Metal => "금(金)",
            Ohang::Water => "수(水)",
        }
    }
}

/// 타로 카드 한 장의 정의
#[derive(Clone, Serialize)]
pub struct TarotCard {
    /// 카드 고유 번호 (0~77)
    pub id: u8,
    /// 한국어 이름
    pub name_ko: &'static str,
    /// 영어 이름
    pub name_en: &'static str,
    /// 아르카나 종류
    pub arcana: ArcanaType,
    /// 수트 (마이너 아르카나만 해당)
    pub suit: Option<Suit>,
    /// 카드 번호 (메이저: 0~21, 마이너: 1~14)
    pub number: u8,
    /// 핵심 키워드 목록
    #[serde(skip)]
    #[allow(dead_code)]
    pub(crate) keywords: &'static [&'static str],
    /// 정방향 의미
    #[serde(skip)]
    #[allow(dead_code)]
    pub(crate) upright_meaning: &'static str,
    /// 역방향 의미
    #[serde(skip)]
    #[allow(dead_code)]
    pub(crate) reversed_meaning: &'static str,
    /// 서양 4원소
    pub element: TarotElement,
    /// 대응하는 오행
    pub ohang: Ohang,
}

impl fmt::Debug for TarotCard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TarotCard")
            .field("id", &self.id)
            .field("name_ko", &self.name_ko)
            .field("name_en", &self.name_en)
            .field("arcana", &self.arcana)
            .field("suit", &self.suit)
            .field("number", &self.number)
            .field("element", &self.element)
            .field("ohang", &self.ohang)
            .finish()
    }
}

/// 스프레드 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SpreadType {
    OneCard,
    ThreeCard,
    CelticCross,
}

impl SpreadType {
    /// reading_type 문자열에서 SpreadType으로 변환
    pub fn from_reading_type(reading_type: &str) -> Option<SpreadType> {
        match reading_type {
            "tarot_daily" | "tarot_one" | "tarot_one_preview" => Some(SpreadType::OneCard),
            "tarot_three" => Some(SpreadType::ThreeCard),
            "tarot_celtic" => Some(SpreadType::CelticCross),
            _ => None,
        }
    }

    /// 스프레드에 필요한 카드 수
    pub fn card_count(self) -> usize {
        match self {
            SpreadType::OneCard => 1,
            SpreadType::ThreeCard => 3,
            SpreadType::CelticCross => 10,
        }
    }

    /// 한국어 이름
    pub fn name_ko(self) -> &'static str {
        match self {
            SpreadType::OneCard => "원카드",
            SpreadType::ThreeCard => "쓰리카드",
            SpreadType::CelticCross => "켈틱크로스",
        }
    }

    /// 각 위치의 이름 목록
    pub fn position_names(self) -> &'static [&'static str] {
        match self {
            SpreadType::OneCard => &["오늘의 메시지"],
            SpreadType::ThreeCard => &["과거", "현재", "미래"],
            SpreadType::CelticCross => &[
                "현재 상황",
                "장애물",
                "목표",
                "기반",
                "과거",
                "미래",
                "자신",
                "환경",
                "희망/두려움",
                "최종 결과",
            ],
        }
    }
}

/// 뽑힌 카드 한 장
#[derive(Debug, Clone, Serialize)]
pub struct DrawnCard {
    /// 카드 ID (TarotCard 참조)
    pub card_id: u8,
    /// 위치 인덱스
    pub position: u8,
    /// 위치 이름
    pub position_name: String,
    /// 역방향 여부
    pub is_reversed: bool,
}

/// 타로 리딩 결과
#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub(crate) struct TarotReading {
    /// 스프레드 종류
    pub(crate) spread_type: SpreadType,
    /// 뽑힌 카드들
    pub(crate) cards: Vec<DrawnCard>,
    /// 카드별 해석 텍스트
    pub(crate) interpretations: Vec<String>,
    /// 종합 메시지
    pub(crate) overall_message: String,
}
