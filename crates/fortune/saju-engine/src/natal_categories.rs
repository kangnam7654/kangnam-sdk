//! Natal-category readings derived from the core saju result.
//!
//! These are legacy compatibility composers for existing SDK reading types.
//! New application backends should build user-facing category narratives from
//! structured saju facts rather than parsing these strings.

use serde_json::{Value, json};

#[derive(Debug, Default, Clone, Copy)]
struct TenGodCounts {
    rival: u32,
    output: u32,
    wealth: u32,
    officer: u32,
    seal: u32,
}

impl TenGodCounts {
    fn from(saju: &Value) -> Self {
        let mut c = TenGodCounts::default();
        if let Some(arr) = saju.get("ten_gods").and_then(|v| v.as_array()) {
            for entry in arr {
                let label = ten_god_label(entry);
                match label.as_str() {
                    label if label.starts_with("비견") || label.starts_with("겁재") => {
                        c.rival += 1
                    }
                    label if label.starts_with("식신") || label.starts_with("상관") => {
                        c.output += 1
                    }
                    label if label.starts_with("편재") || label.starts_with("정재") => {
                        c.wealth += 1
                    }
                    label if label.starts_with("편관") || label.starts_with("정관") => {
                        c.officer += 1
                    }
                    label if label.starts_with("편인") || label.starts_with("정인") => {
                        c.seal += 1
                    }
                    _ => {}
                }
            }
        }
        c
    }
}

fn ten_god_label(entry: &Value) -> String {
    entry
        .get("god")
        .or_else(|| entry.get("label"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn day_master_label(saju: &Value) -> String {
    saju.get("day_master")
        .and_then(|dm| dm.get("stem"))
        .and_then(|v| v.as_str())
        .unwrap_or("일간")
        .to_string()
}

fn has_shinsal(saju: &Value, needle: &str) -> bool {
    saju.get("shinsal")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter().any(|s| {
                ["kind_korean", "label", "name"].iter().any(|key| {
                    s.get(*key)
                        .and_then(|v| v.as_str())
                        .is_some_and(|v| v.contains(needle))
                })
            })
        })
        .unwrap_or(false)
}

fn shape(
    headline: &str,
    summary: String,
    sections: Vec<(&'static str, String)>,
    version: &'static str,
) -> Value {
    json!({
        "headline": headline,
        "summary": summary,
        "sections": sections.into_iter()
            .map(|(title, body)| json!({ "title": title, "body": body }))
            .collect::<Vec<_>>(),
        "engine_version": version,
    })
}

pub fn compose(reading_type: &str, saju: &Value) -> Option<Value> {
    match reading_type {
        "saju_wealth" => Some(compose_wealth(saju)),
        "saju_love" => Some(compose_love(saju)),
        "saju_marriage" => Some(compose_marriage(saju)),
        "saju_career" => Some(compose_career(saju)),
        "saju_health" => Some(compose_health(saju)),
        "saju_study" => Some(compose_study(saju)),
        "saju_children" => Some(compose_children(saju)),
        "saju_travel" => Some(compose_travel(saju)),
        "saju_relations" => Some(compose_relations(saju)),
        _ => None,
    }
}

fn compose_wealth(saju: &Value) -> Value {
    let c = TenGodCounts::from(saju);
    let dm = day_master_label(saju);
    let headline = match (c.wealth, c.output) {
        (0, _) => "재성이 드러나지 않은 사주, 보이는 돈보다 만드는 흐름",
        (1..=2, 0) => "재성은 있으나 만드는 통로가 약한 자수성가형",
        (1..=2, _) => "재성과 식상이 함께 굴러가는 균형형 재물 흐름",
        (3..=4, _) => "재성이 두터운 사주, 감당 가능한 구조가 관건",
        _ => "재성이 매우 강한 사주, 욕심과 그릇의 균형이 핵심",
    };
    shape(
        headline,
        format!("재성 {} · 식상 {} · 비겁 {} · 일간 {} 기준 재물 패턴.", c.wealth, c.output, c.rival, dm),
        vec![
            ("돈을 끌어오는 방식", if c.wealth == 0 {
                "재성 자체보다 신용·전문성·결과물에 돈이 따라오는 구조입니다. 단기 수익보다 오래 쌓이는 직책, 자격, 기술을 먼저 키우는 편이 안정적입니다."
            } else if c.output == 0 {
                "재성은 있으나 새로 만드는 통로가 약합니다. 한 번 들어온 자리를 오래 지키고, 익숙한 시장에서 몫을 넓히는 방식이 유리합니다."
            } else {
                "재성과 식상이 같이 움직입니다. 만들고 팔고 설득하는 흐름이 자연스러워 사업·영업·프리랜스형 수입에 강점이 있습니다."
            }.to_string()),
            ("조심해야 할 함정", if c.rival >= 3 {
                "비겁이 강해 동업·공동투자·보증에서 돈이 새기 쉽습니다. 친한 관계일수록 명의와 책임을 분리해야 합니다."
            } else if c.wealth >= 4 {
                "재성이 과하면 수익 기회가 많아 보여도 욕심이 판단을 흐릴 수 있습니다. 고위험 집중 투자보다 분산과 현금흐름 관리가 우선입니다."
            } else {
                "큰 함정은 자만입니다. 좋은 시기에 무리하지 않고, 나쁜 시기에 구조를 바꾸지 않는 꾸준함이 자산을 지킵니다."
            }.to_string()),
            ("지금 우선해야 할 것", if c.output == 0 {
                "새 시도보다 현재 자리의 깊이를 키우세요. 이미 들어온 돈의 흐름을 정리하고 반복 가능한 수입 구조를 만드는 것이 먼저입니다."
            } else {
                "결과물이 시장에 닿는 채널을 늘리세요. 부업, 콘텐츠, 제안, 영업처럼 식상이 재성을 밀어주는 행동이 필요합니다."
            }.to_string()),
            ("좋은 시기와 나쁜 시기", "재성 운에는 자산 기회가 강하게 오지만, 일간이 약한 시기에는 큰돈을 감당하기 어렵습니다. 큰 결정은 대운 흐름과 함께 확인하는 편이 안전합니다.".to_string()),
        ],
        "saju-wealth-v0.1.0",
    )
}

fn compose_love(saju: &Value) -> Value {
    let c = TenGodCounts::from(saju);
    let dm = day_master_label(saju);
    let dohwa = has_shinsal(saju, "도화");
    let headline = match (c.output, c.officer, dohwa) {
        (_, _, true) => "도화 흐름이 있어 사람을 끌어당기는 매력이 강한 사주",
        (0, 0, _) => "감정 표현과 관계 동력이 천천히 익는 연애 패턴",
        (3.., _, _) => "표현이 강해 마음을 적극적으로 드러내는 연애 흐름",
        (_, 3.., _) => "관성이 두터워 책임감을 무겁게 받는 연애 사주",
        _ => "표현과 책임이 균형 잡힌 안정형 연애 사주",
    };
    shape(
        headline,
        format!("식상 {} · 관성 {} · 일간 {} 기준 본명 연애 패턴.", c.output, c.officer, dm),
        vec![
            ("끌리는 사람의 유형", if c.officer >= c.output {
                "책임감 있고 안정적인 사람에게 끌립니다. 관계가 깊어질수록 말보다 약속과 태도를 더 중요하게 봅니다."
            } else {
                "감정 표현이 풍부하고 자기 세계가 분명한 사람에게 끌립니다. 대화와 분위기가 인연을 여는 핵심입니다."
            }.to_string()),
            ("연애에서 자주 만나는 함정", if dohwa {
                "관계가 쉽게 시작되는 만큼 정리되지 않은 인연도 따라올 수 있습니다. 끌림보다 경계와 선택 기준이 중요합니다."
            } else if c.output >= 3 {
                "표현이 앞서 상대가 부담을 느낄 수 있습니다. 마음을 숨기라는 뜻이 아니라 속도를 조절해야 합니다."
            } else {
                "감정 표현이 늦어 인연이 지나갈 수 있습니다. 마음이 있으면 작은 말이라도 그 자리에서 남기는 편이 좋습니다."
            }.to_string()),
            ("지금 우선해야 할 태도", "상대가 나를 어떻게 보느냐보다 내가 관계 안에서 어떤 리듬을 반복하는지 보는 것이 먼저입니다.".to_string()),
            ("좋은 시기와 나쁜 시기", "관성·식상 운에는 인연이 빠르게 들어오고, 비겁 운에는 경쟁·삼각 구도가 생기기 쉽습니다.".to_string()),
        ],
        "saju-love-v0.1.0",
    )
}

fn compose_marriage(saju: &Value) -> Value {
    let c = TenGodCounts::from(saju);
    let dm = day_master_label(saju);
    let headline = match (c.officer, c.output, c.rival) {
        (0, 0, _) => "결혼 동기를 스스로 만들어가야 하는 부부 사주",
        (3.., _, _) => "관성이 두터워 결혼 후 책임이 중심축이 되는 사주",
        (_, 3.., _) => "표현이 강해 부부 대화가 핵심이 되는 사주",
        (_, _, 3..) => "비겁이 강해 결혼 결정에 본인 의지가 중요한 사주",
        _ => "관성과 식상이 균형을 이룬 평탄한 결혼 흐름",
    };
    shape(
        headline,
        format!("관성 {} · 식상 {} · 비겁 {} · 일간 {} 기준 부부 패턴.", c.officer, c.output, c.rival, dm),
        vec![
            ("맞는 배우자상", if c.officer >= 3 {
                "감정보다 책임과 약속을 지키는 사람과 안정적입니다."
            } else if c.output >= 3 {
                "표현을 받아주고 자기 색도 분명한 상대가 잘 맞습니다."
            } else {
                "본인의 페이스에 맞춰 호흡을 맞출 수 있는 균형형 상대가 좋습니다."
            }.to_string()),
            ("결혼 시 주의할 패턴", if c.rival >= 3 {
                "본인 의지가 강해 주변 조언을 차단하기 쉽습니다. 결정은 스스로 하되 소통은 닫지 않아야 합니다."
            } else if c.officer >= 3 && c.output == 0 {
                "책임은 강하지만 표현이 적어질 수 있습니다. 마음을 말로 옮기는 연습이 필요합니다."
            } else {
                "큰 함정은 없지만 시기 변화에 따라 같은 사람도 다르게 보일 수 있습니다."
            }.to_string()),
            ("부부 운영의 우선순위", "강한 부분이 닮은 사람보다 약한 부분을 보완해주는 사람과 길게 안정됩니다.".to_string()),
            ("결혼 시기와 흐름", "관성 운에는 결혼 결정이 빨라지고, 충이 강한 해에는 큰 결정을 늦추는 편이 안전합니다.".to_string()),
        ],
        "saju-marriage-v0.1.0",
    )
}

fn compose_career(saju: &Value) -> Value {
    let c = TenGodCounts::from(saju);
    let dm = day_master_label(saju);
    let headline = match (c.officer, c.output, c.seal) {
        (3.., _, _) => "관성이 두터워 조직과 직책에 어울리는 직업 사주",
        (_, 3.., _) => "식상이 강해 만들어 파는 자영·창작형 직업 사주",
        (_, _, 3..) => "인성이 강해 학문·자격·전문성으로 자리를 만드는 사주",
        (0, 0, 0) => "환경에 따라 적성이 달라지는 균형형 직업 사주",
        _ => "관성·인성·식상이 균형 잡힌 다재다능형 직업 사주",
    };
    shape(
        headline,
        format!(
            "관성 {} · 식상 {} · 인성 {} · 일간 {} 기준 직업 패턴.",
            c.officer, c.output, c.seal, dm
        ),
        vec![
            (
                "어울리는 직업 형태",
                if c.officer >= c.output && c.officer >= c.seal && c.officer > 0 {
                    "조직·직책·책임이 분명한 환경에서 강점이 살아납니다."
                } else if c.output >= c.officer && c.output > 0 {
                    "결과물을 직접 만들고 시장에 내놓는 일에서 기회가 큽니다."
                } else if c.seal > 0 {
                    "공부와 자격, 깊은 전문성이 자산이 되는 직업이 맞습니다."
                } else {
                    "한 형태에 고정하기보다 경험을 넓히며 적성을 찾는 편이 좋습니다."
                }
                .to_string(),
            ),
            (
                "조직 안에서의 강점과 약점",
                if c.rival >= 3 {
                    "협업보다 본인 책임 범위가 분명한 자리에서 효율이 높습니다."
                } else if c.officer >= 3 {
                    "체계 안에서 성과를 내지만 자율성이 큰 환경에서는 방향을 잃기 쉽습니다."
                } else {
                    "팀 안의 균형추 역할을 잘하지만 본인 색을 의식적으로 드러낼 필요가 있습니다."
                }
                .to_string(),
            ),
            (
                "이직·창업 결정 시 체크포인트",
                "강한 십신을 더 세게 만드는 환경보다 약한 부분을 보완하는 환경이 오래 갑니다."
                    .to_string(),
            ),
            (
                "좋은 시기와 나쁜 시기",
                "관성 운에는 승진·이직, 식상 운에는 창업·부업 흐름이 강해집니다.".to_string(),
            ),
        ],
        "saju-career-v0.1.0",
    )
}

fn compose_health(saju: &Value) -> Value {
    let c = TenGodCounts::from(saju);
    let dm = day_master_label(saju);
    let strong = c.seal + c.rival;
    let weak = c.output + c.wealth + c.officer;
    let headline = if strong > weak + 2 {
        "일간이 강한 사주, 버티는 힘은 있으나 무리의 누적을 조심"
    } else if weak > strong + 2 {
        "일간이 약한 사주, 회복 시간을 충분히 확보해야 하는 체질"
    } else {
        "일간이 중화된 사주, 꾸준한 관리가 핵심인 체질"
    };
    shape(
        headline,
        format!(
            "신강 신호 {} · 신약 신호 {} · 일간 {} 기준 체질 패턴.",
            strong, weak, dm
        ),
        vec![
            (
                "타고난 체질의 큰 그림",
                if strong > weak + 2 {
                    "큰 부담을 견디는 힘은 있으나 피로 신호를 무시하기 쉽습니다."
                } else if weak > strong + 2 {
                    "한꺼번에 부담을 받으면 회복이 늦습니다. 일정 사이 회복 구간이 필요합니다."
                } else {
                    "큰 기복보다는 생활 리듬이 컨디션을 좌우합니다."
                }
                .to_string(),
            ),
            (
                "관리해야 할 약한 영역",
                if c.officer >= 3 {
                    "압박과 스트레스가 몸으로 빨리 옵니다. 수면과 소화 리듬을 살피세요."
                } else if c.output >= 3 {
                    "표현·소비 에너지가 많아 신경 피로가 누적되기 쉽습니다."
                } else {
                    "특정 약점보다 전반적인 컨디션 변화를 자주 점검하는 편이 좋습니다."
                }
                .to_string(),
            ),
            (
                "지금 우선해야 할 습관",
                "강한 기운을 계속 밀어붙이기보다 반대 리듬으로 균형을 맞추는 습관이 필요합니다."
                    .to_string(),
            ),
            (
                "주의 시기와 회복 시기",
                "충·형이 강한 해에는 무리한 일정과 장거리 이동을 줄이는 편이 안전합니다."
                    .to_string(),
            ),
        ],
        "saju-health-v0.1.0",
    )
}

fn compose_study(saju: &Value) -> Value {
    let c = TenGodCounts::from(saju);
    let dm = day_master_label(saju);
    let headline = match (c.seal, c.output) {
        (0, 0) => "인성과 식상이 약해 경험으로 익히는 학습 사주",
        (3.., _) => "인성이 두터워 학문·자격·전문 영역에 강한 사주",
        (_, 3..) => "식상이 강해 배운 것을 표현하며 익히는 사주",
        (1..=2, 1..=2) => "입력과 출력이 균형 잡힌 학습 사주",
        _ => "꾸준함이 가장 큰 무기인 안정형 학업 흐름",
    };
    shape(
        headline,
        format!(
            "인성 {} · 식상 {} · 일간 {} 기준 학습 패턴.",
            c.seal, c.output, dm
        ),
        vec![
            (
                "잘 맞는 학습 방식",
                if c.seal >= c.output && c.seal > 0 {
                    "혼자 깊게 읽고 정리하는 학습이 잘 맞습니다."
                } else if c.output > 0 {
                    "배운 것을 말이나 글로 바로 풀어낼 때 기억에 오래 남습니다."
                } else {
                    "짧은 단위의 반복 경험으로 익히는 방식이 효과적입니다."
                }
                .to_string(),
            ),
            (
                "공부에서 만나는 함정",
                if c.seal >= 4 {
                    "입력만 쌓고 결과물이 늦어질 수 있습니다. 외부 제출 기한이 필요합니다."
                } else if c.output >= 4 {
                    "빠르게 이해한 듯 보여도 깊이가 부족할 수 있습니다. 한 분야를 오래 파야 합니다."
                } else {
                    "시기와 환경의 영향을 받으므로 혼자 의지만으로 끌고 가려 하지 마세요."
                }
                .to_string(),
            ),
            (
                "지금 우선해야 할 학습 전략",
                "강한 쪽으로 시작하고 약한 쪽은 스터디·코칭·마감 같은 외부 장치로 보완하세요."
                    .to_string(),
            ),
            (
                "시험·합격 흐름",
                "인성 운에는 자격·합격, 식상 운에는 발표·성과 노출이 잘 맞습니다.".to_string(),
            ),
        ],
        "saju-study-v0.1.0",
    )
}

fn compose_children(saju: &Value) -> Value {
    let c = TenGodCounts::from(saju);
    let dm = day_master_label(saju);
    let child = c.output + c.officer;
    let headline = match child {
        0 => "자녀 인연이 천천히 익는 사주",
        1..=2 => "자녀 인연이 안정적인 사주",
        3..=5 => "자녀와의 인연이 두터운 사주",
        _ => "자녀 인연이 강해 인생의 중심축이 되는 사주",
    };
    shape(
        headline,
        format!("식상 {} · 관성 {} · 일간 {} 기준 자녀 인연 패턴.", c.output, c.officer, dm),
        vec![
            ("자녀와의 관계 결", if c.output >= 3 {
                "표현이 많고 적극적으로 다가가는 부모 흐름입니다. 거리 조절이 중요합니다."
            } else if c.officer >= 3 {
                "책임과 규율을 강하게 세우는 흐름입니다. 자율성을 같이 열어줘야 합니다."
            } else {
                "안정적인 관계를 만들 수 있으나 자녀의 결을 별도로 보는 태도가 필요합니다."
            }.to_string()),
            ("양육에서 주의할 패턴", if c.rival >= 3 {
                "자녀를 본인 분신처럼 동일시하지 않는 거리감이 필요합니다."
            } else if c.seal >= 3 {
                "학업·자격 기준을 강하게 요구하기 쉽습니다. 자녀의 속도를 우선해야 합니다."
            } else {
                "한 시기의 갈등을 평생의 결로 단정하지 않는 것이 중요합니다."
            }.to_string()),
            ("본인이 우선해야 할 자세", "자녀운은 부모 한쪽 사주만으로 확정하지 않습니다. 자녀의 기질과 함께 보아야 합니다.".to_string()),
            ("임신·출산 시기 흐름", "식상 운에는 자녀 인연이 강해지고, 충이 강한 해에는 큰 결정을 신중히 보는 편이 좋습니다.".to_string()),
        ],
        "saju-children-v0.1.0",
    )
}

fn compose_travel(saju: &Value) -> Value {
    let c = TenGodCounts::from(saju);
    let dm = day_master_label(saju);
    let yeokma = has_shinsal(saju, "역마");
    let headline = if yeokma {
        "역마 흐름이 있어 한 곳에 머물지 않는 사주"
    } else if c.rival >= 3 {
        "비겁이 강해 본인 의지로 자리를 옮기는 사주"
    } else if c.seal >= 3 {
        "인성이 두터워 한 자리에 깊이 뿌리내리는 사주"
    } else {
        "이동은 평균적이며 결정적 이동은 시기에 좌우되는 사주"
    };
    shape(
        headline,
        format!("역마 {} · 비겁 {} · 인성 {} · 일간 {} 기준 이동 패턴.", if yeokma { "있음" } else { "없음" }, c.rival, c.seal, dm),
        vec![
            ("본인의 이동 결", if yeokma {
                "환경을 바꾸는 리듬이 필요합니다. 이동 자체가 답답함을 푸는 통로가 됩니다."
            } else if c.seal >= 3 {
                "한 자리에 오래 머물수록 깊이가 생깁니다. 잦은 이동은 오히려 힘을 분산합니다."
            } else {
                "직장·관계·가족 흐름에 따라 이동이 결정되는 균형형입니다."
            }.to_string()),
            ("이동·이사 체크포인트", "이동이 약한 기운을 보완하는지, 강한 기운을 더 과하게 만드는지 확인해야 합니다.".to_string()),
            ("해외·장거리 이동", if yeokma {
                "해외·장거리 이동에서 본인의 그릇이 더 크게 열릴 수 있습니다."
            } else {
                "장거리 이동은 큰 변화를 강제로 끌어오므로 시기와 목적을 분명히 해야 합니다."
            }.to_string()),
            ("이동 시기 흐름", "역마 운에는 이사·이직·이민 흐름이 자연스럽고, 충이 강한 해에는 안전 점검이 필요합니다.".to_string()),
        ],
        "saju-travel-v0.1.0",
    )
}

fn compose_relations(saju: &Value) -> Value {
    let c = TenGodCounts::from(saju);
    let dm = day_master_label(saju);
    let headline = match (c.rival, c.output) {
        (0, 0) => "깊은 소수 인연에 맞는 조용한 대인관계 사주",
        (3.., _) => "비겁이 강해 동료·친구·라이벌이 중심에 있는 사주",
        (_, 3..) => "식상이 강해 표현으로 사람을 끌어모으는 사주",
        _ => "깊이와 넓이가 모두 적당한 균형형 인간관계",
    };
    shape(
        headline,
        format!("비겁 {} · 식상 {} · 일간 {} 기준 대인관계 패턴.", c.rival, c.output, dm),
        vec![
            ("사람을 만나는 방식", if c.rival >= 3 {
                "그룹 안에서 자기 색이 또렷합니다. 동료와의 비교가 동력이 되기도 합니다."
            } else if c.output >= 3 {
                "말과 표현으로 분위기를 만듭니다. 대신 에너지 소모도 큽니다."
            } else if c.rival == 0 && c.output == 0 {
                "넓은 네트워킹보다 깊은 소수 관계에서 안정됩니다."
            } else {
                "환경에 따라 어울리는 그룹이 달라지는 유연성이 강점입니다."
            }.to_string()),
            ("관계에서 만나는 함정", if c.rival >= 4 {
                "친한 사이일수록 돈·계약·책임을 분리해야 합니다."
            } else if c.output >= 4 {
                "말이 앞서 신뢰를 잃기 쉽습니다. 약속한 것만 말하는 절제가 필요합니다."
            } else {
                "한 시기의 갈등을 평생의 결로 단정하지 마세요."
            }.to_string()),
            ("지금 우선해야 할 전략", "나와 같은 사람만 곁에 두면 같은 갈등이 반복됩니다. 약한 기운을 보완하는 사람을 의식적으로 두세요.".to_string()),
            ("대인관계 시기", "비겁 운에는 새 사람이 강하게 들어오고 기존 관계가 정리되는 변화가 잦습니다.".to_string()),
        ],
        "saju-relations-v0.1.0",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saju_with_gods(gods: &[&str]) -> Value {
        json!({
            "day_master": { "stem": "갑" },
            "ten_gods": gods.iter().map(|god| json!({ "god": god })).collect::<Vec<_>>(),
            "shinsal": [{ "kind_korean": "도화살" }, { "kind_korean": "역마살" }]
        })
    }

    #[test]
    fn counts_sdk_god_field_with_hanja_suffix() {
        let result = compose_wealth(&saju_with_gods(&[
            "편재(偏財)",
            "정재(正財)",
            "식신(食神)",
            "비견(比肩)",
        ]));
        assert!(result["summary"].as_str().unwrap_or("").contains("재성 2"));
        assert!(result["summary"].as_str().unwrap_or("").contains("식상 1"));
        assert!(result["summary"].as_str().unwrap_or("").contains("비겁 1"));
    }

    #[test]
    fn all_category_readings_return_four_sections() {
        let saju = saju_with_gods(&["정관(正官)", "식신(食神)", "정인(正印)"]);
        for reading_type in [
            "saju_wealth",
            "saju_love",
            "saju_marriage",
            "saju_career",
            "saju_health",
            "saju_study",
            "saju_children",
            "saju_travel",
            "saju_relations",
        ] {
            let result = compose(reading_type, &saju).expect("known category");
            assert!(result["headline"].as_str().is_some_and(|v| !v.is_empty()));
            assert_eq!(result["sections"].as_array().map(Vec::len), Some(4));
        }
    }
}
