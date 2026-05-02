use super::cards;
use super::{DrawnCard, TarotReading};

/// 카드 해석 텍스트 생성 (사주 무관, 카드 단독 의미만 전달).
///
/// `reading.interpretations`에 각 카드별 basic 해석을 채우고,
/// `reading.overall_message`에 종합 메시지를 채운다.
/// 반환값은 카드별 basic 해석 문자열 리스트.
pub fn interpret(reading: &mut TarotReading) -> Vec<String> {
    let mut basics: Vec<String> = Vec::with_capacity(reading.cards.len());

    for drawn in &reading.cards {
        let card = match cards::get_card(drawn.card_id) {
            Some(c) => c,
            None => {
                basics.push("카드 정보를 불러올 수 없습니다.".to_string());
                continue;
            }
        };

        let base_meaning = if drawn.is_reversed {
            card.reversed_meaning
        } else {
            card.upright_meaning
        };

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
}
