//! 행운 아이템(色·數·方位) 산출 — 일간 기반 주조(主調)와 균형 기반 보충(補).
//!
//! 두 세트를 함께 제공하여 소비자 서비스가 계산값을 바탕으로 자체 해석을
//! 붙일 수 있게 한다.
//!
//! - **주조(primary)**: 일간(`day.stem`)의 오행 → 자아의 핵심을 드러내는 색·수·방위.
//! - **보충(supplementary)**: 사주 8글자 중 가장 부족한 오행 → 균형 회복의 행운.
//!
//! 정통 명리학의 용신(用神, [[Decisions/ADR ...]])이 들어오면 보충 자리가 그것으로
//! 자연스럽게 교체된다. v0.2.0 시점에서는 `ElementBalance::weakest()` 의 단순
//! 빈도 기반 결손 오행이 그 역할을 임시로 맡는다.
//!
//! ## 매핑 (전통 하도/河圖 기준)
//!
//! | 오행 | 색상 | 숫자 | 방위 |
//! |---|---|---|---|
//! | 목(木) | 청 | 3, 8 | 동(東) |
//! | 화(火) | 적 | 2, 7 | 남(南) |
//! | 토(土) | 황 | 5, 10 | 중앙 |
//! | 금(金) | 백 | 4, 9 | 서(西) |
//! | 수(水) | 흑 | 1, 6 | 북(北) |
//!
//! 색상은 한국어 의미 키워드만 반환 — 구체적 hex 코드는 소비자(lunawave web)
//! 의 디자인 시스템에 위임 (saju-engine을 다른 컨슈머에서도 재사용 가능하게).

use crate::types::{Element, ElementBalance, FourPillars};
use serde::Serialize;

/// 한 오행에 대응되는 색·수·방위 묶음.
#[derive(Debug, Clone, Serialize)]
pub struct LuckyTriple {
    /// 이 묶음의 기준 오행.
    pub element: Element,
    /// 한국어 색상 라벨 (예: "청색").
    pub color: &'static str,
    /// 행운 숫자 2개 (하도 기준).
    pub numbers: [u8; 2],
    /// 한국어 방위 라벨 (예: "동(東)").
    pub direction: &'static str,
}

impl LuckyTriple {
    fn for_element(element: Element) -> Self {
        let (color, numbers, direction) = match element {
            Element::Wood => ("청색", [3, 8], "동(東)"),
            Element::Fire => ("적색", [2, 7], "남(南)"),
            Element::Earth => ("황색", [5, 10], "중앙"),
            Element::Metal => ("백색", [4, 9], "서(西)"),
            Element::Water => ("흑색", [1, 6], "북(北)"),
        };
        LuckyTriple {
            element,
            color,
            numbers,
            direction,
        }
    }
}

/// 일간 + 오행 균형으로 도출한 행운 아이템 계산값. 사용자-facing prose는 포함하지 않는다.
#[derive(Debug, Clone, Serialize)]
pub struct LuckyCoreItems {
    /// 일간 오행 기반 — 자아·본질 색.
    pub primary: LuckyTriple,
    /// 가장 부족한 오행 기반 — 균형 회복 색. 진짜 용신은 v0.1.x.
    pub supplementary: LuckyTriple,
}

/// 일간 + 오행 균형으로 행운 아이템 계산값만 도출.
pub fn calculate(pillars: &FourPillars, has_birth_time: bool) -> LuckyCoreItems {
    let primary_elem = pillars.day.stem.element();
    let balance = ElementBalance::from_pillars_with_hour(pillars, has_birth_time);
    let supplementary_elem = balance.weakest();

    let primary = LuckyTriple::for_element(primary_elem);
    let supplementary = LuckyTriple::for_element(supplementary_elem);

    LuckyCoreItems {
        primary,
        supplementary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Branch, Pillar, Stem};

    fn pillars(
        year: (Stem, Branch),
        month: (Stem, Branch),
        day: (Stem, Branch),
        hour: (Stem, Branch),
    ) -> FourPillars {
        FourPillars {
            year: Pillar::new(year.0, year.1),
            month: Pillar::new(month.0, month.1),
            day: Pillar::new(day.0, day.1),
            hour: Pillar::new(hour.0, hour.1),
        }
    }

    /// 5오행 매핑 anchor 잠금 — 하도 기준 색·수·방위.
    #[test]
    fn five_element_mapping_anchor() {
        let cases = [
            (Element::Wood, "청색", [3, 8], "동(東)"),
            (Element::Fire, "적색", [2, 7], "남(南)"),
            (Element::Earth, "황색", [5, 10], "중앙"),
            (Element::Metal, "백색", [4, 9], "서(西)"),
            (Element::Water, "흑색", [1, 6], "북(北)"),
        ];
        for (elem, color, numbers, direction) in cases {
            let t = LuckyTriple::for_element(elem);
            assert_eq!(t.color, color, "{:?} color", elem);
            assert_eq!(t.numbers, numbers, "{:?} numbers", elem);
            assert_eq!(t.direction, direction, "{:?} direction", elem);
        }
    }

    /// 갑 일간(목) → primary가 청색·동.
    #[test]
    fn gap_day_master_primary_is_wood() {
        let p = pillars(
            (Stem::Im, Branch::Sin),
            (Stem::Gye, Branch::Yu),
            (Stem::Gap, Branch::Ja),
            (Stem::Eul, Branch::Chuk),
        );
        let r = calculate(&p, true);
        assert_eq!(r.primary.element, Element::Wood);
        assert_eq!(r.primary.color, "청색");
        assert_eq!(r.primary.direction, "동(東)");
    }

    /// 갑 일간 + 사주에 화 0개 → supplementary가 화.
    #[test]
    fn supplementary_picks_missing_element() {
        // 의도적으로 화(火) 0개 사주: 갑(목) 임(수) 계(수) 을(목) — 천간 모두 목/수.
        // 지지 자(수) 신(금) 인(목) 축(토). 화 카운트 0.
        let p = pillars(
            (Stem::Im, Branch::Ja),
            (Stem::Gye, Branch::Sin),
            (Stem::Gap, Branch::In),
            (Stem::Eul, Branch::Chuk),
        );
        let r = calculate(&p, true);
        assert_eq!(r.primary.element, Element::Wood);
        assert_eq!(r.supplementary.element, Element::Fire);
        assert_eq!(r.supplementary.color, "적색");
    }

    /// has_birth_time = false 에서도 oranje. weakest 계산이 시주 제외 가능.
    #[test]
    fn no_hour_still_returns_valid_lucky() {
        let p = pillars(
            (Stem::Im, Branch::Sin),
            (Stem::Gye, Branch::Yu),
            (Stem::Gap, Branch::Ja),
            (Stem::Eul, Branch::Chuk),
        );
        let r = calculate(&p, false);
        assert_eq!(r.primary.element, Element::Wood);
        // supplementary는 결정적 — element는 어느 것이든 5오행 중 하나여야.
        assert!(matches!(
            r.supplementary.element,
            Element::Wood | Element::Fire | Element::Earth | Element::Metal | Element::Water
        ));
    }
}
