use serde_json::{Value, json};

pub const TAROT_INTERPRETATION_VERSION: &str = "tarot-v2.1";

#[derive(Clone, Copy)]
struct CardGuide {
    archetype: &'static str,
    upright: &'static str,
    reversed: &'static str,
    reflection: &'static str,
    keywords: &'static [&'static str],
}

#[derive(Clone, Copy)]
struct CategoryGuide {
    key: &'static str,
    label: &'static str,
    focus: &'static str,
    lens: &'static str,
    question_axis: &'static str,
}

const SOURCES: &[(&str, &str, &str)] = &[
    (
        "waite_pictorial_key",
        "A. E. Waite, The Pictorial Key to the Tarot (1911)",
        "https://sacred-texts.com/tarot/pkt/index.htm",
    ),
    (
        "met_history",
        "The Metropolitan Museum of Art, Before Fortune-Telling: The History and Structure of Tarot Cards",
        "https://www.metmuseum.org/perspectives/tarot-2",
    ),
    (
        "vam_history",
        "Victoria and Albert Museum, A history of tarot cards",
        "https://www.vam.ac.uk/articles/tarot-cards",
    ),
    (
        "kci_psychological_application",
        "최경희, 타로카드의 심리학적 적용에 대한 제언적 고찰 (2023)",
        "https://www.kci.go.kr/kciportal/landing/article.kci?arti_id=ART002931770",
    ),
    (
        "jung_journal_archetypal_tarot",
        "Jessica K. Fink, Archetypal Tarot: The Art of Seeing Through (2022)",
        "https://www.tandfonline.com/doi/abs/10.1080/19342039.2022.2053470",
    ),
];

const CATEGORIES: &[CategoryGuide] = &[
    CategoryGuide {
        key: "love",
        label: "연애",
        focus: "관계와 친밀감",
        lens: "관계에서는 감정의 진심, 경계, 상호성, 선택의 책임을 함께 보셔야 합니다.",
        question_axis: "이 관계에서 내가 지키고 싶은 진심과 경계는 무엇인가요?",
    },
    CategoryGuide {
        key: "career",
        label: "직업",
        focus: "업무와 진로",
        lens: "업무에서는 역할, 권한, 실행력, 협업 방식이 카드의 핵심 메시지가 됩니다.",
        question_axis: "지금 일에서 강화해야 할 역할과 조정해야 할 방식은 무엇인가요?",
    },
    CategoryGuide {
        key: "wealth",
        label: "재물",
        focus: "돈과 자원",
        lens: "재물에서는 수입보다 자원 관리, 위험 한도, 계약과 지출의 균형을 먼저 보셔야 합니다.",
        question_axis: "내 자원을 지키면서도 흐르게 하려면 어떤 기준이 필요할까요?",
    },
    CategoryGuide {
        key: "health",
        label: "건강",
        focus: "몸과 마음의 리듬",
        lens: "건강에서는 진단이 아니라 수면, 회복, 스트레스, 생활 리듬을 점검하는 상징으로 읽습니다.",
        question_axis: "몸과 마음이 반복해서 보내는 신호를 어떻게 돌볼 수 있을까요?",
    },
    CategoryGuide {
        key: "general",
        label: "전반",
        focus: "삶의 방향",
        lens: "전반 흐름에서는 사건 예측보다 선택의 태도, 전환점, 자기이해를 중심으로 읽습니다.",
        question_axis: "지금의 선택이 내가 되고 싶은 방향과 어떻게 연결되나요?",
    },
];

const CARD_GUIDES: [CardGuide; 22] = [
    CardGuide {
        archetype: "새 출발과 자유로운 실험",
        upright: "새 출발은 완벽한 계획보다 열린 태도에서 시작됩니다. 아직 길이 다 보이지 않아도 호기심과 신뢰가 첫 문을 엽니다.",
        reversed: "충동과 회피가 자유처럼 보일 수 있습니다. 뛰어들기 전에 최소한의 준비와 책임 범위를 확인해야 합니다.",
        reflection: "두려움 때문에 미루는 것과 준비 부족을 용기로 포장하는 것을 구분해보세요.",
        keywords: &["새 출발", "가능성", "신뢰", "주의"],
    },
    CardGuide {
        archetype: "의지와 도구를 현실로 연결하는 힘",
        upright: "이미 가진 자원과 기술을 한 방향으로 모을 때 결과가 만들어집니다. 말, 손, 계획이 같은 목적을 향해야 합니다.",
        reversed: "능력은 있지만 의도나 방법이 흐려져 있습니다. 과장, 산만함, 지름길 유혹을 걷어내고 실력으로 증명해야 합니다.",
        reflection: "지금 내 손에 이미 있는 도구는 무엇이며, 어디에 집중해야 하나요?",
        keywords: &["의지", "기술", "집중", "실현"],
    },
    CardGuide {
        archetype: "직관과 숨은 진실",
        upright: "겉으로 드러난 정보보다 침묵 속의 신호가 중요합니다. 서두르지 않을 때 내면의 앎이 선명해집니다.",
        reversed: "직관을 외면하거나 반대로 근거 없는 예감에만 기대고 있을 수 있습니다. 감정과 사실을 분리해 보세요.",
        reflection: "내가 이미 알고 있지만 말로 인정하지 않은 진실은 무엇인가요?",
        keywords: &["직관", "비밀", "침묵", "내면"],
    },
    CardGuide {
        archetype: "풍요와 창조적 돌봄",
        upright: "무언가를 키우고 살리는 힘이 커집니다. 창조, 돌봄, 감각의 회복이 실제 성과로 이어질 수 있습니다.",
        reversed: "돌봄이 과잉보호나 소진으로 바뀌기 쉽습니다. 먼저 자신을 채워야 건강하게 나눌 수 있습니다.",
        reflection: "내가 키우고 싶은 것은 무엇이며, 그것을 위해 어떤 환경이 필요한가요?",
        keywords: &["풍요", "창조", "돌봄", "성장"],
    },
    CardGuide {
        archetype: "구조와 책임 있는 권위",
        upright: "원칙과 경계가 상황을 안정시킵니다. 책임을 피하지 않고 구조를 세울수록 선택지가 넓어집니다.",
        reversed: "통제와 고집이 책임으로 위장될 수 있습니다. 질서를 세우되 사람과 상황의 유연성을 남겨야 합니다.",
        reflection: "내가 세워야 할 기준은 무엇이며, 어디서 지나치게 통제하고 있나요?",
        keywords: &["구조", "책임", "권위", "경계"],
    },
    CardGuide {
        archetype: "전통과 배움의 통로",
        upright: "검증된 지혜, 멘토, 공동체의 기준이 도움이 됩니다. 혼자만의 해석보다 배움의 체계를 활용하세요.",
        reversed: "낡은 규범을 무비판적으로 따르거나 반대로 모든 조언을 거부하고 있을 수 있습니다. 내 신념을 스스로 점검해야 합니다.",
        reflection: "지금 필요한 가르침은 무엇이며, 어떤 규칙은 다시 질문해야 하나요?",
        keywords: &["전통", "멘토", "신념", "배움"],
    },
    CardGuide {
        archetype: "선택과 결합",
        upright: "끌림만이 아니라 가치의 일치가 중요합니다. 마음이 향하는 곳과 책임질 수 있는 선택을 함께 보세요.",
        reversed: "불일치, 회피, 유혹이 선택을 흐립니다. 관계나 결정 안에서 잃어버린 자기 목소리를 회복해야 합니다.",
        reflection: "내가 선택하려는 것은 욕망인가요, 가치인가요, 둘의 조화인가요?",
        keywords: &["사랑", "선택", "가치", "조화"],
    },
    CardGuide {
        archetype: "방향을 잡은 추진력",
        upright: "서로 다른 힘을 한 방향으로 모으면 돌파가 가능합니다. 의지와 통제가 균형을 이룰 때 전진합니다.",
        reversed: "속도는 있지만 방향이 흔들립니다. 이기려는 마음이 목적을 압도하지 않도록 운전대를 다시 잡아야 합니다.",
        reflection: "지금 전진을 막는 것은 외부 장애물인가요, 내부의 분산인가요?",
        keywords: &["전진", "통제", "승리", "방향"],
    },
    CardGuide {
        archetype: "부드러운 용기와 자기조절",
        upright: "강함은 밀어붙이는 힘보다 다루는 힘에 가깝습니다. 불안과 욕구를 적으로 보지 말고 길들여야 합니다.",
        reversed: "자신감 저하나 억눌린 분노가 판단을 흔들 수 있습니다. 힘을 숨기기보다 안전하게 표현하는 법이 필요합니다.",
        reflection: "나는 무엇을 제압하려 하고, 무엇을 이해하려 해야 하나요?",
        keywords: &["용기", "인내", "자기조절", "회복"],
    },
    CardGuide {
        archetype: "고독 속의 지혜",
        upright: "잠시 물러나야 보이는 답이 있습니다. 타인의 소음보다 내면의 기준을 밝히는 시간이 필요합니다.",
        reversed: "성찰이 고립이나 회피로 변하고 있을 수 있습니다. 배운 것을 다시 세상과 연결해야 합니다.",
        reflection: "혼자 있어야 보이는 진실과, 혼자만 있어서 놓치는 도움은 무엇인가요?",
        keywords: &["성찰", "고독", "지혜", "안내"],
    },
    CardGuide {
        archetype: "순환과 전환의 타이밍",
        upright: "흐름이 바뀌는 국면입니다. 통제할 수 없는 변화를 읽고, 준비된 선택으로 기회를 잡아야 합니다.",
        reversed: "같은 패턴이 반복되고 있습니다. 운이 나쁘다고 보기 전에 내가 되풀이하는 선택을 확인하세요.",
        reflection: "반복되는 흐름 속에서 내가 바꿀 수 있는 한 가지는 무엇인가요?",
        keywords: &["순환", "전환", "기회", "패턴"],
    },
    CardGuide {
        archetype: "공정함과 책임의 저울",
        upright: "감정과 사실을 나누고 균형 있게 판단해야 합니다. 결과보다 과정의 정당성이 오래 남습니다.",
        reversed: "편향, 책임 회피, 과도한 엄격함이 균형을 무너뜨릴 수 있습니다. 판단 기준을 다시 투명하게 세우세요.",
        reflection: "내가 공정하다고 믿는 기준은 모두에게 같은 방식으로 적용되고 있나요?",
        keywords: &["공정", "진실", "책임", "균형"],
    },
    CardGuide {
        archetype: "멈춤과 관점 전환",
        upright: "지금의 정지는 실패가 아니라 시야를 바꾸는 시간일 수 있습니다. 내려놓을 때 새 해석이 생깁니다.",
        reversed: "불필요한 희생이나 지연이 길어지고 있습니다. 기다림이 지혜인지 회피인지 구분해야 합니다.",
        reflection: "붙들수록 꼬이는 것은 무엇이며, 내려놓으면 보일 관점은 무엇인가요?",
        keywords: &["멈춤", "전환", "희생", "통찰"],
    },
    CardGuide {
        archetype: "끝맺음과 변형",
        upright: "어떤 단계가 끝나야 다음 단계가 시작됩니다. 상실보다 변형의 과정을 의식적으로 받아들이는 카드입니다.",
        reversed: "끝난 것을 붙들수록 새 흐름이 막힙니다. 변화 저항의 이유를 인정하고 작별의 절차를 밟아야 합니다.",
        reflection: "이미 끝났지만 아직 놓지 못한 것은 무엇인가요?",
        keywords: &["변형", "끝맺음", "해방", "전환"],
    },
    CardGuide {
        archetype: "절제와 조율",
        upright: "서로 다른 요소를 섞어 균형점을 찾는 시기입니다. 급한 결론보다 지속 가능한 리듬이 중요합니다.",
        reversed: "과함과 부족함이 번갈아 나타날 수 있습니다. 극단을 줄이고 회복 가능한 속도로 조정해야 합니다.",
        reflection: "지금 섞어야 할 것과 덜어내야 할 것은 각각 무엇인가요?",
        keywords: &["조율", "절제", "균형", "회복"],
    },
    CardGuide {
        archetype: "속박과 욕망의 그림자",
        upright: "나를 묶는 것은 외부 조건만이 아니라 반복되는 욕망과 두려움일 수 있습니다. 사슬의 구조를 정확히 보세요.",
        reversed: "속박을 알아차리고 풀어내는 단계입니다. 단번에 끊기보다 유혹의 패턴을 약화시키는 선택이 필요합니다.",
        reflection: "내가 벗어날 수 없다고 믿는 것은 정말 사실인가요?",
        keywords: &["속박", "유혹", "그림자", "해방"],
    },
    CardGuide {
        archetype: "붕괴와 진실의 노출",
        upright: "버티던 구조가 흔들리며 감춰진 진실이 드러납니다. 충격은 크지만 허술한 기반을 다시 세울 기회가 됩니다.",
        reversed: "큰 붕괴를 피하려 작은 균열을 외면하고 있을 수 있습니다. 지금 점검하면 손상을 줄일 수 있습니다.",
        reflection: "이미 균열이 보이는 구조는 무엇이며, 무엇부터 점검해야 하나요?",
        keywords: &["붕괴", "각성", "진실", "재건"],
    },
    CardGuide {
        archetype: "희망과 회복의 별빛",
        upright: "긴장 뒤에 회복의 감각이 돌아옵니다. 멀리 있는 이상을 바라보되 작은 치유의 루틴을 놓치지 마세요.",
        reversed: "희망이 약해져 방향을 잃기 쉽습니다. 큰 믿음을 억지로 만들기보다 가까운 가능성부터 다시 켜야 합니다.",
        reflection: "내가 회복하고 싶은 믿음은 무엇이며, 오늘 할 수 있는 작은 돌봄은 무엇인가요?",
        keywords: &["희망", "치유", "영감", "회복"],
    },
    CardGuide {
        archetype: "무의식과 불확실성",
        upright: "불안, 환상, 직감이 뒤섞인 구간입니다. 선명한 결론보다 무엇이 두려움을 키우는지 살펴야 합니다.",
        reversed: "혼란이 조금씩 걷히는 과정입니다. 그래도 모든 것이 명확해졌다고 단정하지 말고 검증을 이어가세요.",
        reflection: "내 두려움이 사실을 말하고 있나요, 상상을 키우고 있나요?",
        keywords: &["무의식", "불안", "직감", "검증"],
    },
    CardGuide {
        archetype: "기쁨과 명료한 생명력",
        upright: "숨기지 않아도 되는 밝음이 찾아옵니다. 성과, 인정, 즐거움을 건강하게 누릴 수 있는 카드입니다.",
        reversed: "빛이 약해진 것이 아니라 과한 낙관이나 피로가 시야를 흐릴 수 있습니다. 기쁨을 회복 가능한 방식으로 다루세요.",
        reflection: "나는 어떤 성취를 더 솔직하게 인정하고 즐겨도 될까요?",
        keywords: &["기쁨", "명료함", "성공", "활력"],
    },
    CardGuide {
        archetype: "각성과 부름에 대한 응답",
        upright: "지난 경험을 평가하고 다음 단계로 응답할 때입니다. 후회보다 배움을 정리할수록 재출발이 쉬워집니다.",
        reversed: "자기비판이나 미루기가 결단을 늦춥니다. 완벽한 확신을 기다리기보다 책임질 수 있는 답을 선택하세요.",
        reflection: "내가 더 이상 미룰 수 없는 부름은 무엇인가요?",
        keywords: &["각성", "평가", "응답", "재출발"],
    },
    CardGuide {
        archetype: "완성과 통합",
        upright: "여정의 조각들이 하나로 묶입니다. 마무리와 축하, 다음 순환으로 넘어갈 준비가 함께 옵니다.",
        reversed: "완성 직전의 지연이나 미해결 과제가 남아 있습니다. 끝맺음의 기준을 정하고 마지막 정리를 해야 합니다.",
        reflection: "끝냈다고 말하기 위해 마지막으로 통합해야 할 것은 무엇인가요?",
        keywords: &["완성", "통합", "성취", "이동"],
    },
];

pub fn is_current_tarot_version(version: &str) -> bool {
    version == TAROT_INTERPRETATION_VERSION
}

pub fn enrich_tarot_result(result: &mut Value, category: Option<&str>) -> String {
    let category = category_guide(category);

    if let Some(cards) = result.get_mut("cards").and_then(|v| v.as_array_mut()) {
        for card in cards {
            enrich_card(card, category);
        }
    }

    let summary = build_overall_summary(result, category);
    if let Some(obj) = result.as_object_mut() {
        obj.insert("engine_version".into(), json!(TAROT_INTERPRETATION_VERSION));
        obj.insert("category".into(), json!(category.key));
        obj.insert("category_label".into(), json!(category.label));
        obj.insert("category_focus".into(), json!(category.focus));
        obj.insert("overall_summary".into(), json!(summary));
        obj.insert(
            "interpretation_framework".into(),
            json!({
                "version": TAROT_INTERPRETATION_VERSION,
                "method": "Rider-Waite-Smith 계열 정/역방향 의미 + 스프레드 위치 + 질문 카테고리 + 상징심리 자기성찰 프레임",
                "sources": SOURCES.iter().map(|(id, title, url)| {
                    json!({ "id": id, "title": title, "url": url })
                }).collect::<Vec<_>>()
            }),
        );
    }

    TAROT_INTERPRETATION_VERSION.to_string()
}

fn enrich_card(card: &mut Value, category: CategoryGuide) {
    let Some(obj) = card.as_object_mut() else {
        return;
    };
    let Some(card_id) = read_card_id(obj) else {
        return;
    };
    let Some(guide) = CARD_GUIDES.get(card_id) else {
        return;
    };

    let is_reversed = obj
        .get("is_reversed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let name_ko =
        read_string(obj, &["name_ko", "card_name_ko"]).unwrap_or_else(|| "이 카드".into());
    let name_en = read_string(obj, &["name_en", "card_name_en"]);
    let position_label = read_string(obj, &["position_label"])
        .or_else(|| read_string(obj, &["position_desc"]))
        .unwrap_or_else(|| "선택한 카드".into());
    let orientation = if is_reversed {
        "역방향"
    } else {
        "정방향"
    };
    let core = if is_reversed {
        guide.reversed
    } else {
        guide.upright
    };

    let interpretation = format!(
        "[{}] {} ({}) — {}\n{} {} {}\n질문: {} {}",
        position_label,
        name_ko,
        orientation,
        guide.archetype,
        position_context(&position_label, category),
        core,
        category_lens(category, guide, is_reversed),
        category.question_axis,
        guide.reflection,
    );

    let preview_text = truncate_chars(
        &format!("{}: {}", guide.archetype, first_sentence(core)),
        80,
    );

    obj.insert("card_id".into(), json!(card_id));
    obj.insert("number".into(), json!(card_id));
    obj.insert("name_ko".into(), json!(name_ko));
    if let Some(name_en) = name_en {
        obj.insert("name_en".into(), json!(name_en));
    }
    obj.insert("direction".into(), json!(orientation));
    obj.insert("source_archetype".into(), json!(guide.archetype));
    obj.insert("source_keywords".into(), json!(guide.keywords));
    obj.insert("category".into(), json!(category.key));
    obj.insert("category_label".into(), json!(category.label));
    obj.insert("meaning".into(), json!(core));
    obj.insert("interpretation".into(), json!(interpretation));
    obj.insert("preview_text".into(), json!(preview_text));

    let mut keywords = obj
        .get("keywords")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(ToOwned::to_owned))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for keyword in guide.keywords {
        if !keywords.iter().any(|k| k == keyword) {
            keywords.push((*keyword).to_string());
        }
    }
    obj.insert("keywords".into(), json!(keywords));
}

fn category_guide(category: Option<&str>) -> CategoryGuide {
    let key = category.unwrap_or("general");
    CATEGORIES
        .iter()
        .copied()
        .find(|c| c.key == key)
        .unwrap_or_else(|| {
            CATEGORIES
                .iter()
                .copied()
                .find(|c| c.key == "general")
                .unwrap()
        })
}

fn read_card_id(obj: &serde_json::Map<String, Value>) -> Option<usize> {
    ["card_id", "number", "card_number"]
        .iter()
        .find_map(|key| obj.get(*key).and_then(|v| v.as_i64()))
        .and_then(|id| usize::try_from(id).ok())
        .filter(|id| *id < CARD_GUIDES.len())
}

fn read_string(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| obj.get(*key).and_then(|v| v.as_str()))
        .map(ToOwned::to_owned)
}

fn position_context(position_label: &str, category: CategoryGuide) -> String {
    let normalized = position_label.trim();
    if normalized.is_empty() || normalized == "선택한 카드" {
        return format!(
            "이 카드는 {} 질문에서 지금 가장 먼저 살펴야 할 상징을 보여줍니다.",
            category.label
        );
    }

    format!(
        "{} 자리는 {}의 관점에서 이 카드가 작동하는 위치를 보여줍니다.",
        normalized, category.focus
    )
}

fn category_lens(category: CategoryGuide, guide: &CardGuide, is_reversed: bool) -> String {
    let mode = if is_reversed {
        "그림자 면을 다룰 때"
    } else {
        "강점을 살릴 때"
    };
    format!(
        "{} {}에는 '{}'라는 원형을 {}의 언어로 번역해 보세요.",
        category.lens, mode, guide.archetype, category.focus
    )
}

fn build_overall_summary(result: &Value, category: CategoryGuide) -> String {
    let cards = result
        .get("cards")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    match cards.len() {
        0 => format!(
            "{} 리딩은 카드가 충분하지 않아 결과를 요약하기 어렵습니다.",
            category.label
        ),
        1 => {
            let (name, reversed, archetype) = card_snapshot(&cards[0]);
            let orientation = if reversed { "역방향" } else { "정방향" };
            format!(
                "이번 {} 리딩의 중심은 {}({})입니다. 핵심 주제는 {}이며, 예언처럼 단정하기보다 지금의 선택과 태도를 점검하는 상징으로 보시면 좋습니다.",
                category.label, name, orientation, archetype
            )
        }
        3 => {
            let (past, _, past_arc) = card_snapshot(&cards[0]);
            let (present, _, present_arc) = card_snapshot(&cards[1]);
            let (future, _, future_arc) = card_snapshot(&cards[2]);
            format!(
                "{} 질문은 과거의 {}({})에서 현재의 {}({})로 이어지고, 앞으로는 {}({})의 과제를 향합니다. 세 카드는 사건을 단정하기보다 흐름 속에서 선택할 태도를 보여줍니다.",
                category.label, past, past_arc, present, present_arc, future, future_arc
            )
        }
        10 => {
            let (current, _, current_arc) = card_snapshot(&cards[0]);
            let (challenge, _, challenge_arc) = card_snapshot(&cards[1]);
            let (outcome, _, outcome_arc) = card_snapshot(&cards[9]);
            format!(
                "켈틱크로스에서 현재는 {}({})가, 도전은 {}({})가, 결과는 {}({})가 잡고 있습니다. {} 문제는 단일 답보다 여러 층위가 얽힌 흐름으로 보고, 마지막 결과 카드는 지금 조정할 선택의 방향으로 읽어야 합니다.",
                current,
                current_arc,
                challenge,
                challenge_arc,
                outcome,
                outcome_arc,
                category.label
            )
        }
        _ => format!(
            "{} 리딩은 여러 카드가 서로 보완하는 구조입니다. 반복되는 원형과 역방향 카드를 중심으로 지금 조정할 태도를 읽어보세요.",
            category.label
        ),
    }
}

fn card_snapshot(card: &Value) -> (String, bool, String) {
    let obj = card.as_object();
    let name = obj
        .and_then(|o| read_string(o, &["name_ko", "card_name_ko"]))
        .unwrap_or_else(|| "카드".into());
    let reversed = obj
        .and_then(|o| o.get("is_reversed"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let archetype = obj
        .and_then(|o| read_string(o, &["source_archetype"]))
        .or_else(|| {
            obj.and_then(read_card_id)
                .and_then(|id| CARD_GUIDES.get(id))
                .map(|g| g.archetype.to_string())
        })
        .unwrap_or_else(|| "상징".into());

    (name, reversed, archetype)
}

fn first_sentence(text: &str) -> &str {
    text.split('.').next().unwrap_or(text).trim()
}

fn truncate_chars(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }

    let mut truncated = text
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_card(id: i64, name: &str, reversed: bool, position: i64) -> Value {
        json!({
            "position": position,
            "position_label": format!("자리 {}", position + 1),
            "position_desc": "테스트 자리",
            "card_id": id,
            "name_ko": name,
            "name_en": "Test",
            "arcana": "Major",
            "number": id,
            "is_reversed": reversed,
            "keywords": ["기존"],
            "meaning": "기존 의미",
            "interpretation": "기존 해석"
        })
    }

    #[test]
    fn daily_love_card_gets_category_and_source_layers() {
        let mut result = json!({
            "spread_type": "tarot_daily",
            "spread_name": "오늘의 타로",
            "cards": [sample_card(4, "황제", false, 0)],
            "overall_summary": "기존 요약"
        });

        let version = enrich_tarot_result(&mut result, Some("love"));

        assert_eq!(version, TAROT_INTERPRETATION_VERSION);
        assert!(is_current_tarot_version(&version));
        assert_eq!(result["engine_version"], TAROT_INTERPRETATION_VERSION);
        assert_eq!(result["category"], "love");
        assert_eq!(result["category_label"], "연애");

        let card = &result["cards"][0];
        let interpretation = card["interpretation"].as_str().unwrap();
        assert!(interpretation.contains("황제"));
        assert!(interpretation.contains("관계"));
        assert!(interpretation.contains("질문:"));
        assert!(card["source_keywords"].as_array().unwrap().len() >= 3);
        assert!(
            result["interpretation_framework"]["sources"]
                .as_array()
                .unwrap()
                .len()
                >= 4
        );

        for banned in ["사주", "오행", "일간", "상생", "상극"] {
            assert!(
                !interpretation.contains(banned),
                "{banned} must not appear in tarot text"
            );
        }
    }

    #[test]
    fn reversed_card_uses_shadow_language_without_certainty() {
        let mut result = json!({
            "spread_type": "tarot_one",
            "spread_name": "원카드",
            "cards": [sample_card(16, "탑", true, 0)],
        });

        enrich_tarot_result(&mut result, Some("career"));

        let interpretation = result["cards"][0]["interpretation"].as_str().unwrap();
        assert!(interpretation.contains("역방향"));
        assert!(interpretation.contains("업무"));
        assert!(interpretation.contains("점검"));
        assert!(!interpretation.contains("반드시"));
        assert!(!interpretation.contains("확정"));
    }

    #[test]
    fn preview_shape_gets_professional_preview_text() {
        let mut result = json!({
            "spread_type": "one_card_preview",
            "is_preview": true,
            "cards": [{
                "card_name_ko": "바보",
                "card_name_en": "The Fool",
                "card_number": 0,
                "is_reversed": false,
                "direction": "정방향",
                "preview_text": "기존 프리뷰"
            }]
        });

        enrich_tarot_result(&mut result, Some("general"));

        let card = &result["cards"][0];
        assert_eq!(card["card_id"], 0);
        assert_eq!(card["name_ko"], "바보");
        let preview = card["preview_text"].as_str().unwrap();
        assert!(preview.contains("새 출발"));
        assert!(preview.chars().count() <= 80);
        assert!(card["interpretation"].as_str().unwrap().contains("질문:"));
    }

    #[test]
    fn celtic_summary_uses_current_challenge_and_outcome() {
        let cards: Vec<Value> = (0..10)
            .map(|i| sample_card(i, &format!("카드 {i}"), false, i))
            .collect();
        let mut result = json!({
            "spread_type": "tarot_celtic",
            "spread_name": "켈틱크로스",
            "cards": cards,
        });

        enrich_tarot_result(&mut result, Some("wealth"));

        let summary = result["overall_summary"].as_str().unwrap();
        assert!(summary.contains("현재"));
        assert!(summary.contains("도전"));
        assert!(summary.contains("결과"));
        assert!(summary.contains("재물"));
    }
}
