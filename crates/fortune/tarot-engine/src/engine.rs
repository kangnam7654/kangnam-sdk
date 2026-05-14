use serde_json::{Value, json};

use crate::cards;
use crate::draw;
use crate::interpreter;
use crate::types::{SpreadType, TarotReading};

pub struct TarotEngine;

/// 엔진 버전. 캐시 무효화 기준으로 사용된다.
pub const TAROT_ENGINE_VERSION: &str = "tarot-v2.3";

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

        let drawn_cards = draw::draw_cards(&spread_type, &seed_input);

        // 프리뷰: 선택 위치의 카드 1장만 요약 정보로 반환
        if reading_type == "tarot_one_preview" {
            let all_cards = draw::draw_cards_n(draw::DRAW_POOL_SIZE as usize, &seed_input);
            let pick_idx = selected_position.min(all_cards.len().saturating_sub(1));
            let drawn = &all_cards[pick_idx];
            let card = cards::get_card(drawn.card_id);
            if let Some(card) = card {
                let full_meaning = if drawn.is_reversed {
                    card.reversed_meaning
                } else {
                    card.upright_meaning
                };
                let preview_text = if full_meaning.chars().count() > 30 {
                    let truncated: String = full_meaning.chars().take(30).collect();
                    format!("{}...", truncated)
                } else {
                    full_meaning.to_string()
                };
                let direction = if drawn.is_reversed {
                    "역방향"
                } else {
                    "정방향"
                };

                let result = json!({
                    "spread_type": "one_card_preview",
                    "is_preview": true,
                    "cards": [{
                        "card_name_ko": card.name_ko,
                        "card_name_en": card.name_en,
                        "card_number": card.number,
                        "is_reversed": drawn.is_reversed,
                        "direction": direction,
                        "preview_text": preview_text,
                    }]
                });
                return (result, version);
            }
        }

        // tarot_one / tarot_daily에서 selected_position 적용 (tarot_daily는 0 고정으로 쓰는 걸 권장하지만 옵션은 허용)
        let drawn_cards =
            if matches!(reading_type, "tarot_one" | "tarot_daily") && selected_position > 0 {
                let all = draw::draw_cards_n(draw::DRAW_POOL_SIZE as usize, &seed_input);
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
                    .all(|&p| p < draw::DRAW_POOL_SIZE as usize)
                && {
                    let mut sorted = selected_positions.clone();
                    sorted.sort();
                    sorted.dedup();
                    sorted.len() == selected_positions.len()
                };

            if is_valid {
                let all = draw::draw_cards_n(draw::DRAW_POOL_SIZE as usize, &seed_input);
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

        // 해석 (사주 무관). options.category가 있으면 카테고리별 본문 사용
        // (메이저만), 없거나 매칭 안 되는 마이너는 cards.rs 일반 톤 fallback.
        // 카테고리 후보: "love" | "career" | "wealth" | "health" | "general".
        let category = input
            .get("options")
            .and_then(|o| o.get("category"))
            .and_then(|v| v.as_str());

        let mut reading = TarotReading {
            spread_type,
            cards: drawn_cards,
            interpretations: Vec::new(),
            overall_message: String::new(),
        };

        let basics = interpreter::interpret_with_category(&mut reading, category);

        // JSON 결과 구성 — interpretation 단일 키만 유지. saju/mixed/basic 분기 키 없음.
        let cards_json: Vec<Value> = reading
            .cards
            .iter()
            .enumerate()
            .map(|(i, drawn)| {
                let card = cards::get_card(drawn.card_id);
                let interpretation = basics.get(i).cloned().unwrap_or_default();

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
                        "keywords": card.keywords,
                        "meaning": if drawn.is_reversed { card.reversed_meaning } else { card.upright_meaning },
                        // 클라이언트가 is_reversed에 따라 직접 선택할 수 있도록 두 톤을 모두 노출한다
                        "meaning_upright": card.upright_meaning,
                        "meaning_reversed": card.reversed_meaning,
                        "interpretation": interpretation,
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
            "cards": cards_json,
            "overall_summary": reading.overall_message,
        });

        (result, version)
    }
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
        assert!(result["overall_summary"].is_string());

        // 사주 관련 필드 부재 검증
        assert!(result.get("saju_connection").is_none());
        let card = &result["cards"][0];
        assert!(card.get("saju_interpretation").is_none());
        assert!(card.get("mixed_interpretation").is_none());
        assert!(card.get("basic_interpretation").is_none());
        assert!(card.get("interpretation").is_some());
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
        assert!(card["preview_text"].is_string());

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
        assert!(card["keywords"].is_array());
        assert!(card["meaning"].is_string());
        assert!(card["interpretation"].is_string());
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
        assert_eq!(TAROT_ENGINE_VERSION, "tarot-v2.3");
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
        // 같은 사용자 + 같은 날 + draw_index 다르면 카드도 달라야 함.
        // 22장 풀이라 1/22 확률로 우연히 겹칠 수 있어, fixture는 미리 검증된
        // birth_date를 사용한다. 셋 중 하나라도 겹치면 fixture를 조정할 것.
        let engine = TarotEngine;
        let base = |di: u64| {
            json!({
                "birth_date": "1990-01-01",
                "birth_time": "12:00",
                "calendar_type": "solar",
                "options": { "draw_index": di },
            })
        };
        let (r0, _) = engine.generate("tarot_one", &base(0));
        let (r1, _) = engine.generate("tarot_one", &base(1));
        let (r2, _) = engine.generate("tarot_one", &base(2));
        let id0 = r0["cards"][0]["card_id"].as_u64().unwrap();
        let id1 = r1["cards"][0]["card_id"].as_u64().unwrap();
        let id2 = r2["cards"][0]["card_id"].as_u64().unwrap();
        assert_ne!(id0, id1, "draw_index=1 must differ from 0 (got {})", id0);
        assert_ne!(id0, id2, "draw_index=2 must differ from 0 (got {})", id0);
        assert_ne!(id1, id2, "draw_index=2 must differ from 1 (got {})", id1);
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
