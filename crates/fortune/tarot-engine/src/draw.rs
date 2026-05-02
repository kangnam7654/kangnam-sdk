use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use sha2::{Digest, Sha256};

use super::{DrawnCard, SpreadType};

/// 드로우 카드 풀: Major Arcana 22장(card_id 0~21)만 사용.
/// 이미지 에셋이 Major Arcana만 제공되므로 Minor Arcana는 드로우하지 않는다.
const DRAW_POOL_SIZE: u8 = 22;

/// N장 드로우 (직접 고르기에서 22장 셔플용)
pub fn draw_cards_n(n: usize, seed_input: &str) -> Vec<DrawnCard> {
    let seed = {
        let mut hasher = Sha256::new();
        hasher.update(seed_input.as_bytes());
        let hash = hasher.finalize();
        u64::from_le_bytes(hash[..8].try_into().unwrap())
    };

    let mut rng = StdRng::seed_from_u64(seed);
    let mut indices: Vec<u8> = (0..DRAW_POOL_SIZE).collect();
    indices.shuffle(&mut rng);

    let count = n.min(indices.len());
    indices[..count]
        .iter()
        .enumerate()
        .map(|(i, &card_id)| {
            let is_reversed: bool = rng.gen_bool(0.5);
            DrawnCard {
                card_id,
                position: i as u8,
                position_name: format!("위치 {}", i + 1),
                is_reversed,
            }
        })
        .collect()
}

/// 시드 기반 카드 드로우
///
/// `seed_input`을 SHA256으로 해싱하여 결정적 난수 시드를 만든다.
/// 같은 입력이면 항상 같은 카드가 나온다.
pub fn draw_cards(spread: &SpreadType, seed_input: &str) -> Vec<DrawnCard> {
    let seed = {
        let mut hasher = Sha256::new();
        hasher.update(seed_input.as_bytes());
        let hash = hasher.finalize();
        u64::from_le_bytes(hash[..8].try_into().unwrap())
    };

    let mut rng = StdRng::seed_from_u64(seed);
    let card_count = spread.card_count();
    let position_names = spread.position_names();

    let mut indices: Vec<u8> = (0..DRAW_POOL_SIZE).collect();
    indices.shuffle(&mut rng);

    let selected = &indices[..card_count];

    selected
        .iter()
        .enumerate()
        .map(|(i, &card_id)| {
            let is_reversed: bool = rng.gen_bool(0.5);
            DrawnCard {
                card_id,
                position: i as u8,
                position_name: position_names[i].to_string(),
                is_reversed,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_draw() {
        let seed = "test_user|1990-01-01|tarot_three|2026-03-19";
        let cards1 = draw_cards(&SpreadType::ThreeCard, seed);
        let cards2 = draw_cards(&SpreadType::ThreeCard, seed);

        assert_eq!(cards1.len(), 3);
        assert_eq!(cards2.len(), 3);
        for (c1, c2) in cards1.iter().zip(cards2.iter()) {
            assert_eq!(c1.card_id, c2.card_id);
            assert_eq!(c1.is_reversed, c2.is_reversed);
        }
    }

    #[test]
    fn test_no_duplicate_cards() {
        let seed = "test_user|1990-06-15|tarot_celtic|2026-03-19";
        let cards = draw_cards(&SpreadType::CelticCross, seed);

        assert_eq!(cards.len(), 10);
        let ids: Vec<u8> = cards.iter().map(|c| c.card_id).collect();
        let mut unique_ids = ids.clone();
        unique_ids.sort();
        unique_ids.dedup();
        assert_eq!(ids.len(), unique_ids.len(), "Duplicate card IDs found");
    }

    #[test]
    fn test_card_ids_in_range() {
        let seed = "test|2026-01-01";
        let cards = draw_cards(&SpreadType::CelticCross, seed);
        for card in &cards {
            assert!(
                card.card_id < DRAW_POOL_SIZE,
                "Card ID {} out of Major Arcana range",
                card.card_id
            );
        }
    }

    #[test]
    fn test_draw_only_major_arcana() {
        // 다양한 시드에 대해 모든 카드가 Major Arcana(0~21)여야 함
        let seeds = [
            "seed_a|2026-04-13",
            "seed_b|1988-07-22",
            "seed_c|2001-12-31",
            "tarot_three|user_x",
            "tarot_celtic|user_y",
        ];
        for seed in seeds {
            let celtic = draw_cards(&SpreadType::CelticCross, seed);
            for card in &celtic {
                assert!(
                    card.card_id < DRAW_POOL_SIZE,
                    "Minor Arcana drawn: {}",
                    card.card_id
                );
            }
            let all = draw_cards_n(DRAW_POOL_SIZE as usize, seed);
            for card in &all {
                assert!(card.card_id < DRAW_POOL_SIZE);
            }
        }
    }

    #[test]
    fn test_one_card_spread() {
        let cards = draw_cards(&SpreadType::OneCard, "one_card_test");
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].position, 0);
        assert_eq!(cards[0].position_name, "오늘의 메시지");
    }

    #[test]
    fn test_three_card_position_names() {
        let cards = draw_cards(&SpreadType::ThreeCard, "three_card_test");
        assert_eq!(cards[0].position_name, "과거");
        assert_eq!(cards[1].position_name, "현재");
        assert_eq!(cards[2].position_name, "미래");
    }
}
