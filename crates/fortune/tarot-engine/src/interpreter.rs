use super::cards;
use super::category_meanings;
use super::{DrawnCard, TarotReading};

/// 카드 해석 텍스트 생성 (사주 무관, 카드 단독 의미만 전달).
///
/// `reading.interpretations`에 각 카드별 basic 해석을 채우고,
/// `reading.overall_message`에 종합 메시지를 채운다.
/// 반환값은 카드별 basic 해석 문자열 리스트.
///
/// 하위 호환: category 미지정. cards.rs의 일반 톤 의미 사용.
pub fn interpret(reading: &mut TarotReading) -> Vec<String> {
    interpret_with_category(reading, None)
}

/// 카테고리 기반 해석. `category`가 메이저 카드의 카테고리 lookup에 매칭되면
/// 그 본문을, 매칭 안 되면 cards.rs의 일반 톤으로 fallback.
///
/// `category` 후보: "love" | "career" | "wealth" | "health" | "general".
/// 마이너 카드(22~77)는 카테고리와 무관하게 항상 일반 톤 fallback.
pub fn interpret_with_category(
    reading: &mut TarotReading,
    category: Option<&str>,
) -> Vec<String> {
    let mut basics: Vec<String> = Vec::with_capacity(reading.cards.len());

    for drawn in &reading.cards {
        let card = match cards::get_card(drawn.card_id) {
            Some(c) => c,
            None => {
                basics.push("카드 정보를 불러올 수 없습니다.".to_string());
                continue;
            }
        };

        // 카테고리별 lookup 우선, fallback은 카드 자체의 일반 톤.
        let base_meaning: &str = category
            .and_then(|cat| {
                category_meanings::major_category_meaning(card.id, cat, drawn.is_reversed)
            })
            .unwrap_or(if drawn.is_reversed {
                card.reversed_meaning
            } else {
                card.upright_meaning
            });

        basics.push(build_interpretation(
            &drawn.position_name,
            card.name_ko,
            drawn.is_reversed,
            base_meaning,
        ));
    }

    reading.overall_message = build_overall_message(&reading.cards);
    reading.interpretations = basics.clone();

    basics
}

/// 카드 단독 의미 해석 문자열. 사주 용어 일절 포함하지 않는다.
fn build_interpretation(
    position_name: &str,
    card_name: &str,
    is_reversed: bool,
    base_meaning: &str,
) -> String {
    let orientation = if is_reversed {
        "역방향"
    } else {
        "정방향"
    };
    format!(
        "[{}] {} ({})\n{}",
        position_name, card_name, orientation, base_meaning
    )
}

/// 종합 메시지 생성 (카드 흐름만 기반, 사주 무관).
fn build_overall_message(cards: &[DrawnCard]) -> String {
    if cards.len() == 1 {
        if let Some(card) = cards::get_card(cards[0].card_id) {
            let orientation = if cards[0].is_reversed {
                "역방향"
            } else {
                "정방향"
            };
            return format!(
                "오늘 당신에게 전하는 메시지는 {}({})입니다. 카드가 전하는 조언을 마음에 새겨보세요.",
                card.name_ko, orientation
            );
        }
    }

    if cards.len() == 3 {
        let past = cards::get_card(cards[0].card_id)
            .map(|c| c.name_ko)
            .unwrap_or("?");
        let present = cards::get_card(cards[1].card_id)
            .map(|c| c.name_ko)
            .unwrap_or("?");
        let future = cards::get_card(cards[2].card_id)
            .map(|c| c.name_ko)
            .unwrap_or("?");
        return format!(
            "과거의 '{}'에서 현재의 '{}'로 이어지고, 미래에는 '{}'의 기운이 찾아올 흐름입니다. 카드가 전하는 메시지를 마음에 새기세요.",
            past, present, future
        );
    }

    if cards.len() == 10 {
        let current = cards::get_card(cards[0].card_id)
            .map(|c| c.name_ko)
            .unwrap_or("?");
        let obstacle = cards::get_card(cards[1].card_id)
            .map(|c| c.name_ko)
            .unwrap_or("?");
        let outcome = cards::get_card(cards[9].card_id)
            .map(|c| c.name_ko)
            .unwrap_or("?");

        let upright_count = cards.iter().filter(|c| !c.is_reversed).count();
        let tone = if upright_count >= 7 {
            "전반적으로 긍정적인 기운이 강합니다."
        } else if upright_count >= 4 {
            "긍정과 도전이 공존하는 시기입니다."
        } else {
            "도전적인 시기이지만 극복할 힘이 있습니다."
        };

        return format!(
            "현재 '{}'의 상황에서 '{}'이(가) 도전으로 작용하고 있습니다. 최종 결과는 '{}'을(를) 가리키고 있습니다. {} 카드가 전하는 조언을 참고하여 지혜롭게 대처하세요.",
            current, obstacle, outcome, tone
        );
    }

    "타로 카드가 당신에게 전하는 메시지를 깊이 새겨보세요.".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::draw_cards;
    use crate::types::SpreadType;

    fn make_reading(spread: SpreadType, seed: &str) -> TarotReading {
        TarotReading {
            cards: draw_cards(&spread, seed),
            spread_type: spread,
            interpretations: Vec::new(),
            overall_message: String::new(),
        }
    }

    #[test]
    fn test_interpret_basic_only() {
        let mut reading = make_reading(SpreadType::ThreeCard, "basic_test");

        let basics = interpret(&mut reading);

        assert_eq!(basics.len(), 3);
        for b in &basics {
            assert!(!b.is_empty());
        }
        assert!(!reading.overall_message.is_empty());
    }

    #[test]
    fn test_interpret_has_no_saju_terms() {
        // 다양한 시드에 대해 해석 텍스트에 사주 용어가 일절 등장하지 않아야 한다.
        let banned = ["기운", "생(生)", "극(克)", "일간", "오행", "상생", "상극"];
        for seed in ["purity_a", "purity_b", "purity_c", "purity_d"] {
            let mut reading = make_reading(SpreadType::OneCard, seed);
            let basics = interpret(&mut reading);
            for text in basics
                .iter()
                .chain(std::iter::once(&reading.overall_message))
            {
                for term in &banned {
                    assert!(
                        !text.contains(term),
                        "사주 용어 '{}' 등장 금지: {}",
                        term,
                        text
                    );
                }
            }
        }
    }

    #[test]
    fn test_interpret_overall_varies_by_spread() {
        let mut one = make_reading(SpreadType::OneCard, "spread_variety");
        interpret(&mut one);
        assert!(one.overall_message.contains("오늘"));

        let mut three = make_reading(SpreadType::ThreeCard, "spread_variety");
        interpret(&mut three);
        assert!(three.overall_message.contains("과거"));

        let mut celtic = make_reading(SpreadType::CelticCross, "spread_variety");
        interpret(&mut celtic);
        assert!(celtic.overall_message.contains("현재"));
    }

    /// 카테고리별 lookup이 메이저 카드에 적용되는지 회귀 lock.
    /// 같은 시드로 두 카테고리 호출 시 카드는 같지만 본문은 카테고리별로 달라야.
    #[test]
    fn category_changes_meaning_for_major_cards() {
        // 메이저 카드만 뽑힐 때까지 시드 시도. 보통 OneCard는 22/78 확률로 메이저.
        for seed in [
            "cat_test_a",
            "cat_test_b",
            "cat_test_c",
            "cat_test_d",
            "cat_test_e",
        ] {
            let mut a = make_reading(SpreadType::OneCard, seed);
            let mut b = make_reading(SpreadType::OneCard, seed);
            // 메이저(0~21)일 때만 검증
            if a.cards[0].card_id > 21 {
                continue;
            }
            let basic_a = interpret_with_category(&mut a, Some("love"));
            let basic_b = interpret_with_category(&mut b, Some("career"));
            assert_eq!(a.cards[0].card_id, b.cards[0].card_id, "same seed, same card");
            assert_ne!(
                basic_a[0], basic_b[0],
                "same major card with different categories must differ — got identical: {}",
                basic_a[0]
            );
            return; // 한 번 성공하면 충분
        }
        panic!("no major card drawn across 5 seeds — adjust seeds");
    }

    /// category=None은 cards.rs의 일반 톤 fallback. 마이너도 항상 fallback.
    #[test]
    fn category_none_uses_default_meaning() {
        let mut r1 = make_reading(SpreadType::OneCard, "fallback_test");
        let mut r2 = make_reading(SpreadType::OneCard, "fallback_test");
        let basic_default = interpret(&mut r1);
        let basic_none = interpret_with_category(&mut r2, None);
        assert_eq!(basic_default, basic_none, "category=None == default interpret");
    }

    /// 알 수 없는 카테고리 — fallback to default 일반 톤.
    #[test]
    fn unknown_category_falls_back_to_default() {
        let mut r1 = make_reading(SpreadType::OneCard, "unknown_cat");
        let mut r2 = make_reading(SpreadType::OneCard, "unknown_cat");
        let default_basic = interpret(&mut r1);
        let unknown_basic = interpret_with_category(&mut r2, Some("nonsense"));
        assert_eq!(default_basic, unknown_basic);
    }
}
