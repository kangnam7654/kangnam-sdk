/// 메이저 아르카나 정방향/역방향 두 톤 카피 결정성 잠금 테스트
///
/// 각 카드의 upright_meaning과 reversed_meaning이:
/// 1. 비어 있지 않고
/// 2. 서로 다른 텍스트를 가지며
/// 3. 각각 2문장 이상(마침표 기준)인지 검증한다.
///
/// 고정 시드를 사용한 카드 해석 fixture 5건(정·역 각각)으로
/// 카피 변경 시 테스트가 실패하도록 잠근다.
#[cfg(test)]
mod tests {
    use crate::cards::get_card;
    use crate::engine::TarotEngine;
    use serde_json::json;

    // ── 헬퍼 ────────────────────────────────────────────────────────────────

    /// 문장 수 추정: 마침표/물음표/느낌표 뒤에 공백이 있거나 문자열 끝인 패턴
    fn sentence_count(text: &str) -> usize {
        text.chars()
            .filter(|&c| c == '.' || c == '?' || c == '!')
            .count()
            .max(1)
    }

    // ── 1. 22장 전체 기본 품질 검증 ─────────────────────────────────────────

    #[test]
    fn all_major_arcana_meanings_are_non_empty() {
        for id in 0u8..22 {
            let card = get_card(id).expect("메이저 아르카나 id는 0~21");
            assert!(
                !card.upright_meaning.is_empty(),
                "카드 {} ({}) upright_meaning 비어있음",
                id,
                card.name_ko
            );
            assert!(
                !card.reversed_meaning.is_empty(),
                "카드 {} ({}) reversed_meaning 비어있음",
                id,
                card.name_ko
            );
        }
    }

    #[test]
    fn all_major_arcana_upright_differs_from_reversed() {
        for id in 0u8..22 {
            let card = get_card(id).expect("메이저 아르카나 id는 0~21");
            assert_ne!(
                card.upright_meaning, card.reversed_meaning,
                "카드 {} ({}) 정방향과 역방향이 동일함 — 두 톤 분리 필요",
                id, card.name_ko
            );
        }
    }

    #[test]
    fn all_major_arcana_meanings_have_at_least_two_sentences() {
        for id in 0u8..22 {
            let card = get_card(id).expect("메이저 아르카나 id는 0~21");
            assert!(
                sentence_count(card.upright_meaning) >= 2,
                "카드 {} ({}) upright_meaning 문장이 너무 짧음: {}",
                id,
                card.name_ko,
                card.upright_meaning
            );
            assert!(
                sentence_count(card.reversed_meaning) >= 2,
                "카드 {} ({}) reversed_meaning 문장이 너무 짧음: {}",
                id,
                card.name_ko,
                card.reversed_meaning
            );
        }
    }

    // ── 2. 5장 fixture — 카피 결정성 잠금 ──────────────────────────────────

    /// (카드 id, 정방향 시작 20자, 역방향 시작 20자)
    /// 카피가 바뀌면 이 테스트가 실패한다.
    const FIXTURES: [(u8, &str, &str); 5] = [
        // 0. 바보
        (0, "벼랑 끝에 선 것 같아도", "발이 앞서고 머리가"),
        // 6. 연인
        (6, "인생에서 몇 번 오지 않는", "관계 안에서 자신의 목"),
        // 13. 죽음
        (13, "지금 끝나는 것이 있어야", "이미 끝났어야 할 것을"),
        // 17. 별
        (17, "폭풍이 지나간 밤하늘에", "희망이 닳아 믿음이 흔"),
        // 21. 세계
        (21, "긴 여정이 아름다운 완성", "결승선이 코앞인데 아직"),
    ];

    #[test]
    fn fixture_upright_copy_lock() {
        for (id, upright_prefix, _) in &FIXTURES {
            let card = get_card(*id).expect("fixture id 유효해야 함");
            assert!(
                card.upright_meaning.contains(upright_prefix),
                "카드 {} ({}) 정방향 카피 변경 감지. 예상 prefix: '{}'\n실제: {}",
                id,
                card.name_ko,
                upright_prefix,
                card.upright_meaning
            );
        }
    }

    #[test]
    fn fixture_reversed_copy_lock() {
        for (id, _, reversed_prefix) in &FIXTURES {
            let card = get_card(*id).expect("fixture id 유효해야 함");
            assert!(
                card.reversed_meaning.contains(reversed_prefix),
                "카드 {} ({}) 역방향 카피 변경 감지. 예상 prefix: '{}'\n실제: {}",
                id,
                card.name_ko,
                reversed_prefix,
                card.reversed_meaning
            );
        }
    }

    // ── 3. engine JSON 응답은 카드 정체성/방향 계산값만 노출 ─

    #[test]
    fn engine_response_excludes_meaning_fields() {
        let engine = TarotEngine;
        let input = json!({
            "birth_date": "1990-06-15",
            "birth_time": "14:30",
            "calendar_type": "solar",
        });

        // 원카드와 쓰리카드 모두 검증
        for rt in ["tarot_one", "tarot_three", "tarot_celtic"] {
            let (result, _) = engine.generate(rt, &input);
            let cards = result["cards"].as_array().expect("cards 배열 필요");
            for card in cards {
                assert!(card.get("meaning").is_none(), "{rt}: meaning 필드 금지");
                assert!(
                    card.get("meaning_upright").is_none(),
                    "{rt}: meaning_upright 필드 금지"
                );
                assert!(
                    card.get("meaning_reversed").is_none(),
                    "{rt}: meaning_reversed 필드 금지"
                );
            }
        }
    }

    #[test]
    fn engine_reversal_keeps_orientation_fact_only() {
        let engine = TarotEngine;
        let input = json!({
            "birth_date": "1990-06-15",
            "birth_time": "14:30",
            "calendar_type": "solar",
        });

        let (result, _) = engine.generate("tarot_three", &input);
        let cards = result["cards"].as_array().unwrap();
        for card in cards {
            assert!(card["is_reversed"].is_boolean());
            assert!(card.get("meaning").is_none());
            assert!(card.get("interpretation").is_none());
        }
    }
}
