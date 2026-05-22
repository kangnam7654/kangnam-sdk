use serde_json::{Value, json};

use crate::cards;
use crate::category_meanings;
use crate::draw;
use crate::profile;
use crate::types::{ArcanaType, DrawnCard, SpreadType, Suit, TarotCard, TarotElement};
use sha2::{Digest, Sha256};

pub struct TarotEngine;

/// 엔진 버전. 캐시 무효화 기준으로 사용된다.
pub const TAROT_ENGINE_VERSION: &str = "tarot-v2.4";

/// Public reading type keys supported by the tarot engine.
pub const TAROT_READING_TYPES: [&str; 5] = [
    "tarot_daily",
    "tarot_one",
    "tarot_one_preview",
    "tarot_three",
    "tarot_celtic",
];

/// Returns whether `reading_type` is a supported tarot reading type key.
pub fn is_valid_reading_type(reading_type: &str) -> bool {
    TAROT_READING_TYPES.contains(&reading_type)
}

impl TarotEngine {
    /// Generate a tarot reading for the given reading_type and input.
    ///
    /// Returns `(result_json, engine_version)`. `reading_type` must be one of
    /// `"tarot_daily"`, `"tarot_one"`, `"tarot_one_preview"`, `"tarot_three"`,
    /// or `"tarot_celtic"`; unknown values produce an error JSON object in
    /// the first tuple element (version is still returned).
    pub fn generate(&self, reading_type: &str, input: &Value) -> (Value, String) {
        let version = TAROT_ENGINE_VERSION.to_string();

        let spread_type = match SpreadType::from_reading_type(reading_type) {
            Some(s) => s,
            None => {
                return (
                    json!({"error": format!("알 수 없는 타로 스프레드: {}", reading_type)}),
                    version,
                );
            }
        };

        let birth_date = input
            .get("birth_date")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let birth_time = input
            .get("birth_time")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let calendar_type = input
            .get("calendar_type")
            .and_then(|v| v.as_str())
            .unwrap_or("solar");

        let kst = chrono::FixedOffset::east_opt(9 * 3600).unwrap();
        let today_kst = chrono::Utc::now()
            .with_timezone(&kst)
            .format("%Y-%m-%d")
            .to_string();
        let drawn_at = chrono::Utc::now().with_timezone(&kst).to_rfc3339();

        // seed 정규화: preview와 풀 리딩이 같은 카드를 뽑도록 함.
        // tarot_daily와 tarot_one은 서로 다른 카드를 유도하기 위해 seed에 reading_type 포함.
        let seed_spread = if reading_type == "tarot_one_preview" {
            "tarot_one"
        } else {
            reading_type
        };
        // 재뽑기 nonce. 0(미지정)이면 기존 시드 그대로 — 하위 호환.
        let draw_index = input
            .get("options")
            .and_then(|o| o.get("draw_index"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let deck_scope = input
            .get("options")
            .and_then(|o| o.get("deck_scope"))
            .and_then(|v| v.as_str())
            .unwrap_or("major_arcana");
        let draw_pool_size = if deck_scope == "full_78" {
            draw::FULL_DECK_SIZE
        } else {
            draw::DRAW_POOL_SIZE
        };
        let draw_pool = if draw_pool_size == draw::FULL_DECK_SIZE {
            "full_78"
        } else {
            "major_arcana_22"
        };
        let seed_input = if draw_index == 0 {
            format!(
                "{}|{}|{}|{}|{}",
                birth_date, birth_time, calendar_type, seed_spread, today_kst
            )
        } else {
            format!(
                "{}|{}|{}|{}|{}|d{}",
                birth_date, birth_time, calendar_type, seed_spread, today_kst, draw_index
            )
        };

        let selected_position = input
            .get("options")
            .and_then(|o| o.get("selected_position"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let drawn_cards = draw::draw_cards_from_pool(&spread_type, &seed_input, draw_pool_size);

        // 프리뷰: 선택 위치의 카드 1장만 요약 정보로 반환
        if reading_type == "tarot_one_preview" {
            let all_cards =
                draw::draw_cards_n_from_pool(draw_pool_size as usize, &seed_input, draw_pool_size);
            let pick_idx = selected_position.min(all_cards.len().saturating_sub(1));
            let drawn = &all_cards[pick_idx];
            let card = cards::get_card(drawn.card_id);
            if let Some(card) = card {
                let direction = if drawn.is_reversed {
                    "역방향"
                } else {
                    "정방향"
                };

                let result = json!({
                    "spread_type": "one_card_preview",
                    "is_preview": true,
                    "engine_version": version,
                    "deck_contract": deck_contract_json(draw_pool, draw_pool_size),
                    "draw_contract": draw_contract_json(&seed_input, &today_kst, draw_index, draw_pool, draw_pool_size, &[], calendar_type),
                    "cards": [{
                        "card_name_ko": card.name_ko,
                        "card_name_en": card.name_en,
                        "card_number": card.number,
                        "is_reversed": drawn.is_reversed,
                        "direction": direction,
                    }]
                });
                return (result, version);
            }
        }

        // tarot_one / tarot_daily에서 selected_position 적용 (tarot_daily는 0 고정으로 쓰는 걸 권장하지만 옵션은 허용)
        let drawn_cards = if matches!(reading_type, "tarot_one" | "tarot_daily")
            && selected_position > 0
        {
            let all =
                draw::draw_cards_n_from_pool(draw_pool_size as usize, &seed_input, draw_pool_size);
            let pick_idx = selected_position.min(all.len().saturating_sub(1));
            vec![all[pick_idx].clone()]
        } else {
            drawn_cards
        };

        // 멀티카드: options.selected_positions 배열 허용
        let drawn_cards = if matches!(spread_type, SpreadType::ThreeCard | SpreadType::CelticCross)
        {
            let selected_positions: Vec<usize> = input
                .get("options")
                .and_then(|o| o.get("selected_positions"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_u64().map(|u| u as usize))
                        .collect()
                })
                .unwrap_or_default();

            let is_valid = selected_positions.len() == spread_type.card_count()
                && selected_positions
                    .iter()
                    .all(|&p| p < draw_pool_size as usize)
                && {
                    let mut sorted = selected_positions.clone();
                    sorted.sort();
                    sorted.dedup();
                    sorted.len() == selected_positions.len()
                };

            if is_valid {
                let all = draw::draw_cards_n_from_pool(
                    draw_pool_size as usize,
                    &seed_input,
                    draw_pool_size,
                );
                let position_names = spread_type.position_names();
                selected_positions
                    .iter()
                    .enumerate()
                    .map(|(i, &pos)| {
                        let mut c = all[pos].clone();
                        c.position = i as u8;
                        c.position_name = position_names
                            .get(i)
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| c.position_name.clone());
                        c
                    })
                    .collect()
            } else {
                drawn_cards
            }
        } else {
            drawn_cards
        };

        let raw_category = input
            .get("options")
            .and_then(|o| o.get("category"))
            .and_then(|v| v.as_str());
        let category = raw_category.filter(|value| category_meanings::is_valid_category(value));

        let cards_json: Vec<Value> = drawn_cards
            .iter()
            .enumerate()
            .map(|(i, drawn)| {
                let card = cards::get_card(drawn.card_id);

                if let Some(card) = card {
                    json!({
                        "position": drawn.position,
                        "position_label": drawn.position_name,
                        "position_desc": spread_type.position_names().get(i).unwrap_or(&""),
                        "card_id": card.id,
                        "name_ko": card.name_ko,
                        "name_en": card.name_en,
                        "arcana": format!("{:?}", card.arcana),
                        "suit": card.suit.map(|s| format!("{:?}", s)),
                        "number": card.number,
                        "is_reversed": drawn.is_reversed,
                        "element": tarot_element_code(card.element),
                        "ohang": card.ohang.korean(),
                    })
                } else {
                    json!({
                        "position": drawn.position,
                        "error": "카드 데이터 없음",
                    })
                }
            })
            .collect();

        let spread_name = if reading_type == "tarot_daily" {
            "오늘의 타로"
        } else {
            spread_type.name_ko()
        };

        let result = json!({
            "spread_type": reading_type,
            "spread_name": spread_name,
            "engine_version": version,
            "drawn_at": drawn_at,
            "valid_for": valid_for_json(reading_type, &today_kst),
            "deck_contract": deck_contract_json(draw_pool, draw_pool_size),
            "spread_contract": spread_contract_json(spread_type),
            "draw_contract": draw_contract_json(
                &seed_input,
                &today_kst,
                draw_index,
                draw_pool,
                draw_pool_size,
                &drawn_cards.iter().map(|card| card.position as usize).collect::<Vec<_>>(),
                calendar_type
            ),
            "cards": cards_json,
            "card_relationships": card_relationships_json(&drawn_cards),
            "reading_facts": reading_facts_json(&drawn_cards, category, raw_category),
        });

        (result, version)
    }
}

fn deck_contract_json(draw_pool: &str, draw_pool_size: u8) -> Value {
    json!({
        "deck_id": "rider_waite_smith_78_ko_v1",
        "source_profile": profile::deck_source_profile_json(),
        "deck_size": cards::all_cards().len(),
        "draw_pool": draw_pool,
        "draw_pool_size": draw_pool_size,
        "major_count": 22,
        "minor_count": 56,
        "supports_reversed": true,
    })
}

fn spread_contract_json(spread_type: SpreadType) -> Value {
    json!({
        "spread": spread_type.name_ko(),
        "card_count": spread_type.card_count(),
        "positions": spread_type.position_names().iter().enumerate().map(|(index, label)| {
            json!({"index": index, "label": label})
        }).collect::<Vec<_>>(),
    })
}

fn draw_contract_json(
    seed_input: &str,
    seed_date: &str,
    draw_index: u64,
    draw_pool: &str,
    draw_pool_size: u8,
    selected_positions: &[usize],
    calendar_type: &str,
) -> Value {
    json!({
        "algorithm": "sha256_seed_std_rng_shuffle_v1",
        "seed_hash": sha256_hex(seed_input),
        "seed_date": seed_date,
        "timezone": "Asia/Seoul",
        "draw_index": draw_index,
        "draw_pool": draw_pool,
        "draw_pool_size": draw_pool_size,
        "selected_positions": selected_positions,
        "calendar_type": calendar_type,
    })
}

fn valid_for_json(reading_type: &str, today_kst: &str) -> Value {
    json!({
        "timezone": "Asia/Seoul",
        "seed_date": today_kst,
        "scope": if reading_type == "tarot_daily" { "daily" } else { "single_draw" },
    })
}

fn tarot_element_code(element: TarotElement) -> &'static str {
    match element {
        TarotElement::Fire => "fire",
        TarotElement::Water => "water",
        TarotElement::Air => "air",
        TarotElement::Earth => "earth",
    }
}

fn card_relationships_json(drawn_cards: &[DrawnCard]) -> Value {
    let resolved = drawn_cards
        .iter()
        .filter_map(|drawn| cards::get_card(drawn.card_id).map(|card| (drawn, card)))
        .collect::<Vec<_>>();
    let reversal_count = resolved
        .iter()
        .filter(|(drawn, _)| drawn.is_reversed)
        .count();
    let dominant_element = dominant_tarot_element(&resolved);
    let pairs = resolved
        .windows(2)
        .map(|pair| {
            let (left_drawn, left) = pair[0];
            let (right_drawn, right) = pair[1];
            json!({
                "from_position": left_drawn.position,
                "to_position": right_drawn.position,
                "from_card": left.name_ko,
                "to_card": right.name_ko,
                "element_relation": element_relation_label(left.element, right.element),
                "number_delta": i16::from(right.number) - i16::from(left.number),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "reversal_count": reversal_count,
        "upright_count": resolved.len().saturating_sub(reversal_count),
        "dominant_element": dominant_element,
        "pairs": pairs,
    })
}

fn reading_facts_json(
    drawn_cards: &[DrawnCard],
    category: Option<&str>,
    raw_category: Option<&str>,
) -> Value {
    let resolved = drawn_cards
        .iter()
        .filter_map(|drawn| cards::get_card(drawn.card_id).map(|card| (drawn, card)))
        .collect::<Vec<_>>();
    let major_count = resolved
        .iter()
        .filter(|(_, card)| card.arcana == ArcanaType::Major)
        .count();
    let minor_count = resolved.len().saturating_sub(major_count);
    json!({
        "category": category.unwrap_or("general"),
        "category_status": if raw_category.is_some() && category.is_none() { "unsupported_fallback_general" } else { "accepted" },
        "major_count": major_count,
        "minor_count": minor_count,
        "suit_balance": suit_balance_json(&resolved),
        "repeated_numbers": repeated_numbers_json(&resolved),
    })
}

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn dominant_tarot_element(cards: &[(&DrawnCard, &TarotCard)]) -> &'static str {
    let mut counts = [0usize; 4];
    for (_, card) in cards {
        counts[match card.element {
            TarotElement::Fire => 0,
            TarotElement::Water => 1,
            TarotElement::Air => 2,
            TarotElement::Earth => 3,
        }] += 1;
    }
    let index = counts
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| *count)
        .map(|(index, _)| index)
        .unwrap_or(0);
    ["fire", "water", "air", "earth"][index]
}

fn element_relation_label(left: TarotElement, right: TarotElement) -> &'static str {
    if left == right {
        "same_element"
    } else {
        match (left, right) {
            (TarotElement::Fire, TarotElement::Air)
            | (TarotElement::Air, TarotElement::Fire)
            | (TarotElement::Water, TarotElement::Earth)
            | (TarotElement::Earth, TarotElement::Water) => "supportive",
            (TarotElement::Fire, TarotElement::Water)
            | (TarotElement::Water, TarotElement::Fire)
            | (TarotElement::Air, TarotElement::Earth)
            | (TarotElement::Earth, TarotElement::Air) => "challenging",
            _ => "neutral",
        }
    }
}

fn suit_balance_json(cards: &[(&DrawnCard, &TarotCard)]) -> Value {
    let mut wands = 0usize;
    let mut cups = 0usize;
    let mut swords = 0usize;
    let mut pentacles = 0usize;
    for (_, card) in cards {
        match card.suit {
            Some(Suit::Wands) => wands += 1,
            Some(Suit::Cups) => cups += 1,
            Some(Suit::Swords) => swords += 1,
            Some(Suit::Pentacles) => pentacles += 1,
            None => {}
        }
    }
    json!({
        "wands": wands,
        "cups": cups,
        "swords": swords,
        "pentacles": pentacles,
    })
}

fn repeated_numbers_json(cards: &[(&DrawnCard, &TarotCard)]) -> Vec<Value> {
    let mut results = Vec::new();
    for number in 0..=21 {
        let matches = cards
            .iter()
            .filter(|(_, card)| card.number == number)
            .map(|(_, card)| card.name_ko)
            .collect::<Vec<_>>();
        if matches.len() > 1 {
            results.push(json!({"number": number, "cards": matches}));
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tarot_engine_one_card() {
        let engine = TarotEngine;
        let input = json!({
            "birth_date": "1990-06-15",
            "birth_time": "14:30",
            "calendar_type": "solar",
            "gender": "male",
        });

        let (result, version) = engine.generate("tarot_one", &input);

        assert_eq!(version, TAROT_ENGINE_VERSION);
        assert_eq!(result["spread_type"], "tarot_one");
        assert_eq!(result["spread_name"], "원카드");
        assert!(result["cards"].is_array());
        assert_eq!(result["cards"].as_array().unwrap().len(), 1);
        assert!(result.get("overall_summary").is_none());

        // 사주 관련 필드 부재 검증
        assert!(result.get("saju_connection").is_none());
        let card = &result["cards"][0];
        assert!(card.get("saju_interpretation").is_none());
        assert!(card.get("mixed_interpretation").is_none());
        assert!(card.get("basic_interpretation").is_none());
        assert!(card.get("interpretation").is_none());
    }

    #[test]
    fn reading_type_api_matches_supported_spreads() {
        assert_eq!(
            TAROT_READING_TYPES,
            [
                "tarot_daily",
                "tarot_one",
                "tarot_one_preview",
                "tarot_three",
                "tarot_celtic",
            ]
        );

        for reading_type in TAROT_READING_TYPES {
            assert!(is_valid_reading_type(reading_type));
            assert!(
                SpreadType::from_reading_type(reading_type).is_some(),
                "{reading_type} must map to a spread"
            );
        }

        assert!(!is_valid_reading_type("tarot_saju_fusion"));
        assert!(!is_valid_reading_type(""));
    }

    #[test]
    fn test_tarot_engine_three_card() {
        let engine = TarotEngine;
        let input = json!({
            "birth_date": "1985-03-22",
            "birth_time": "",
            "calendar_type": "solar",
        });

        let (result, version) = engine.generate("tarot_three", &input);

        assert_eq!(version, TAROT_ENGINE_VERSION);
        assert_eq!(result["cards"].as_array().unwrap().len(), 3);
        assert!(result.get("saju_connection").is_none());
    }

    #[test]
    fn test_tarot_engine_celtic_cross() {
        let engine = TarotEngine;
        let input = json!({
            "birth_date": "2000-12-01",
            "birth_time": "08:00",
            "calendar_type": "solar",
        });

        let (result, _) = engine.generate("tarot_celtic", &input);
        assert_eq!(result["cards"].as_array().unwrap().len(), 10);
        assert!(result.get("saju_connection").is_none());
    }

    #[test]
    fn test_tarot_engine_no_birth_data() {
        let engine = TarotEngine;
        let input = json!({
            "birth_date": "",
            "calendar_type": "solar",
        });

        let (result, _) = engine.generate("tarot_one", &input);
        assert_eq!(result["cards"].as_array().unwrap().len(), 1);
        assert!(result.get("saju_connection").is_none());
    }

    #[test]
    fn test_tarot_engine_invalid_type() {
        let engine = TarotEngine;
        let input = json!({});

        let (result, _) = engine.generate("tarot_unknown", &input);
        assert!(result["error"].is_string());
    }

    #[test]
    fn test_tarot_daily_free() {
        let engine = TarotEngine;
        let input = json!({
            "birth_date": "1990-06-15",
            "birth_time": "14:30",
            "calendar_type": "solar",
        });

        let (result, version) = engine.generate("tarot_daily", &input);

        assert_eq!(version, TAROT_ENGINE_VERSION);
        assert_eq!(result["spread_type"], "tarot_daily");
        assert_eq!(result["spread_name"], "오늘의 타로");
        assert_eq!(result["cards"].as_array().unwrap().len(), 1);
        // 사주 필드 부재
        assert!(result.get("saju_connection").is_none());
    }

    #[test]
    fn test_tarot_daily_and_one_produce_different_cards() {
        let engine = TarotEngine;
        let input = json!({
            "birth_date": "1990-06-15",
            "birth_time": "14:30",
            "calendar_type": "solar",
        });

        let (daily, _) = engine.generate("tarot_daily", &input);
        let (one, _) = engine.generate("tarot_one", &input);

        // seed에 reading_type이 포함되므로 다른 카드를 뽑을 가능성이 높다
        // (간헐적 충돌은 허용하지만 여기서는 card_id가 다름을 기대)
        let daily_id = daily["cards"][0]["card_id"].as_u64().unwrap();
        let one_id = one["cards"][0]["card_id"].as_u64().unwrap();
        assert_ne!(
            daily_id, one_id,
            "tarot_daily와 tarot_one은 서로 다른 카드를 뽑아야 한다 (seed 구성상). 실패 시 seed 분리 검토 필요."
        );
    }

    #[test]
    fn test_tarot_one_preview() {
        let engine = TarotEngine;
        let input = json!({
            "birth_date": "1990-06-15",
            "birth_time": "14:30",
            "calendar_type": "solar",
            "gender": "male",
        });

        let (result, version) = engine.generate("tarot_one_preview", &input);

        assert_eq!(version, TAROT_ENGINE_VERSION);
        assert_eq!(result["spread_type"], "one_card_preview");
        assert_eq!(result["is_preview"], true);
        assert_eq!(result["cards"].as_array().unwrap().len(), 1);

        let card = &result["cards"][0];
        assert!(card["card_name_ko"].is_string());
        assert!(card["card_name_en"].is_string());
        assert!(card["card_number"].is_number());
        assert!(card["is_reversed"].is_boolean());
        assert!(card["direction"].is_string());
        assert!(card.get("preview_text").is_none());

        assert!(result.get("saju_connection").is_none());
        assert!(result.get("overall_summary").is_none());
    }

    #[test]
    fn test_tarot_three_selected_positions() {
        let engine = TarotEngine;
        let input = json!({
            "birth_date": "1990-06-15",
            "birth_time": "14:30",
            "calendar_type": "solar",
            "options": { "selected_positions": [5, 10, 15] },
        });

        let (result, _) = engine.generate("tarot_three", &input);
        let cards = result["cards"].as_array().unwrap();
        assert_eq!(cards.len(), 3);
        assert_eq!(cards[0]["position_label"], "과거");
        assert_eq!(cards[1]["position_label"], "현재");
        assert_eq!(cards[2]["position_label"], "미래");

        let (result2, _) = engine.generate("tarot_three", &input);
        let cards2 = result2["cards"].as_array().unwrap();
        assert_eq!(cards[0]["card_id"], cards2[0]["card_id"]);
        assert_eq!(cards[1]["card_id"], cards2[1]["card_id"]);
        assert_eq!(cards[2]["card_id"], cards2[2]["card_id"]);
    }

    #[test]
    fn test_card_json_structure() {
        let engine = TarotEngine;
        let input = json!({
            "birth_date": "1995-07-20",
            "birth_time": "22:00",
            "calendar_type": "solar",
        });

        let (result, _) = engine.generate("tarot_one", &input);
        let card = &result["cards"][0];

        assert!(card["card_id"].is_number());
        assert!(card["name_ko"].is_string());
        assert!(card["name_en"].is_string());
        assert!(card["arcana"].is_string());
        assert!(card["number"].is_number());
        assert!(card["is_reversed"].is_boolean());
        assert!(card.get("keywords").is_none());
        assert!(card["element"].is_string());
        assert!(card["ohang"].is_string());
        assert!(card.get("meaning").is_none());
        assert!(card.get("interpretation").is_none());
        assert_eq!(result["deck_contract"]["deck_size"], 78);
        assert_eq!(result["deck_contract"]["draw_pool"], "major_arcana_22");
        assert_eq!(
            result["deck_contract"]["source_profile"]["id"],
            profile::TAROT_PROFILE_ID
        );
        assert_eq!(
            result["deck_contract"]["source_profile"]["compatibility_target"],
            profile::TAROT_COMPATIBILITY_TARGET
        );
        assert!(result["draw_contract"]["seed_hash"].is_string());
        assert!(result["draw_contract"].get("seed_input").is_none());
        assert!(result["card_relationships"]["reversal_count"].is_number());
        assert!(result["reading_facts"]["major_count"].is_number());
    }

    #[test]
    fn test_rws_open_data_card_identity_contract() {
        let cards = cards::all_cards();

        assert_eq!(cards.len(), 78);
        assert_eq!(cards[0].name_en, "The Fool");
        assert_eq!(cards[0].number, 0);
        assert_eq!(cards[21].name_en, "The World");
        assert_eq!(cards[21].number, 21);
        assert_eq!(cards[22].name_en, "Ace of Wands");
        assert_eq!(cards[22].suit, Some(Suit::Wands));
        assert_eq!(cards[35].name_en, "King of Wands");
        assert_eq!(cards[36].name_en, "Ace of Cups");
        assert_eq!(cards[49].name_en, "King of Cups");
        assert_eq!(cards[50].name_en, "Ace of Swords");
        assert_eq!(cards[63].name_en, "King of Swords");
        assert_eq!(cards[64].name_en, "Ace of Pentacles");
        assert_eq!(cards[77].name_en, "King of Pentacles");
    }

    #[test]
    fn test_all_drawn_cards_are_major_arcana() {
        let engine = TarotEngine;
        let input = json!({
            "birth_date": "1980-01-01",
            "birth_time": "06:00",
            "calendar_type": "solar",
        });

        for rt in ["tarot_daily", "tarot_one", "tarot_three", "tarot_celtic"] {
            let (result, _) = engine.generate(rt, &input);
            for card in result["cards"].as_array().unwrap() {
                let id = card["card_id"].as_u64().unwrap();
                assert!(id < 22, "{}: card_id {} is Minor Arcana", rt, id);
                assert_eq!(card["arcana"], "Major", "{}: arcana must be Major", rt);
            }
        }
    }

    #[test]
    fn test_engine_version_bumped() {
        assert_eq!(TAROT_ENGINE_VERSION, "tarot-v2.4");
    }

    #[test]
    fn test_full_deck_scope_can_draw_minor_arcana_with_contract() {
        let engine = TarotEngine;
        let input = json!({
            "birth_date": "1990-01-01",
            "birth_time": "12:00",
            "calendar_type": "solar",
            "options": { "deck_scope": "full_78" },
        });
        let (result, _) = engine.generate("tarot_celtic", &input);

        assert_eq!(result["deck_contract"]["draw_pool"], "full_78");
        assert_eq!(result["deck_contract"]["draw_pool_size"], 78);
        assert_eq!(result["cards"].as_array().unwrap().len(), 10);
        assert!(
            result["reading_facts"]["minor_count"]
                .as_u64()
                .is_some_and(|count| count > 0),
            "fixed full_78 fixture must prove minor arcana can be drawn"
        );
        assert!(result["cards"].as_array().unwrap().iter().all(|card| {
            card["card_id"].as_u64().is_some_and(|id| id < 78)
                && card["element"].as_str().is_some()
                && card["ohang"].as_str().is_some()
        }));
    }

    #[test]
    fn test_unknown_category_is_marked_as_general_fallback() {
        let engine = TarotEngine;
        let input = json!({
            "birth_date": "1990-01-01",
            "birth_time": "12:00",
            "calendar_type": "solar",
            "options": { "category": "unknown" },
        });
        let (result, _) = engine.generate("tarot_three", &input);

        assert_eq!(
            result["reading_facts"]["category_status"],
            "unsupported_fallback_general"
        );
        assert_eq!(result["reading_facts"]["category"], "general");
    }

    #[test]
    fn test_draw_index_zero_matches_no_draw_index() {
        // 하위 호환: draw_index=0 또는 미지정 → 동일 카드
        let engine = TarotEngine;
        let input_no_di = json!({
            "birth_date": "1990-01-01",
            "birth_time": "12:00",
            "calendar_type": "solar",
        });
        let input_di_zero = json!({
            "birth_date": "1990-01-01",
            "birth_time": "12:00",
            "calendar_type": "solar",
            "options": { "draw_index": 0 },
        });
        let (a, _) = engine.generate("tarot_one", &input_no_di);
        let (b, _) = engine.generate("tarot_one", &input_di_zero);
        assert_eq!(
            a["cards"][0]["card_id"], b["cards"][0]["card_id"],
            "draw_index=0 must equal omitted draw_index"
        );
        assert_eq!(
            a["cards"][0]["is_reversed"], b["cards"][0]["is_reversed"],
            "draw_index=0 must equal omitted draw_index (reversal)"
        );
    }

    #[test]
    fn test_draw_index_changes_card() {
        // 같은 사용자 + 같은 날이라도 draw_index가 결과 후보군에 영향을 줘야 한다.
        // 22장 풀이라 인접 draw_index끼리는 우연히 같은 카드가 나올 수 있으므로,
        // 여러 draw_index를 훑어 최소 하나 이상의 다른 결과가 생기는지 검증한다.
        let engine = TarotEngine;
        let base = |di: u64| {
            json!({
                "birth_date": "1990-01-01",
                "birth_time": "12:00",
                "calendar_type": "solar",
                "options": { "draw_index": di },
            })
        };
        let variants = (0..8)
            .map(|di| {
                let (result, _) = engine.generate("tarot_one", &base(di));
                (
                    result["cards"][0]["card_id"].as_u64().unwrap(),
                    result["cards"][0]["is_reversed"].as_bool().unwrap(),
                )
            })
            .collect::<std::collections::BTreeSet<_>>();

        assert!(
            variants.len() > 1,
            "draw_index should produce at least two distinct card/orientation results"
        );
    }

    #[test]
    fn test_draw_index_deterministic() {
        // 같은 (birth_date, today, draw_index) → 같은 카드
        let engine = TarotEngine;
        let make = || {
            json!({
                "birth_date": "1990-01-01",
                "birth_time": "12:00",
                "calendar_type": "solar",
                "options": { "draw_index": 7 },
            })
        };
        let (a, _) = engine.generate("tarot_one", &make());
        let (b, _) = engine.generate("tarot_one", &make());
        assert_eq!(a["cards"][0]["card_id"], b["cards"][0]["card_id"]);
        assert_eq!(a["cards"][0]["is_reversed"], b["cards"][0]["is_reversed"]);
    }

    #[test]
    fn test_draw_index_isolated_per_user() {
        // 다른 birth_date + 같은 draw_index → 그 사용자만의 다른 카드
        let engine = TarotEngine;
        let user_a = json!({
            "birth_date": "1990-01-01",
            "birth_time": "12:00",
            "calendar_type": "solar",
            "options": { "draw_index": 1 },
        });
        let user_b = json!({
            "birth_date": "1991-06-15",
            "birth_time": "12:00",
            "calendar_type": "solar",
            "options": { "draw_index": 1 },
        });
        let (a, _) = engine.generate("tarot_one", &user_a);
        let (b, _) = engine.generate("tarot_one", &user_b);
        assert_ne!(
            a["cards"][0]["card_id"], b["cards"][0]["card_id"],
            "different birth_date with same draw_index should produce different cards"
        );
    }
}
