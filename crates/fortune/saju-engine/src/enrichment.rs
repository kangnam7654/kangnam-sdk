use chrono::{Datelike, FixedOffset, Utc};
use serde_json::{Value, json};
use std::collections::HashSet;

pub const ENRICHMENT_VERSION: &str = "saju_extended_v1";

#[derive(Clone, Copy)]
struct BranchInfo {
    ko: &'static str,
    element: &'static str,
    season: &'static str,
}

#[derive(Clone, Copy)]
struct HiddenStem {
    ko: &'static str,
    element: &'static str,
    role: &'static str,
}

#[derive(Clone, Copy)]
struct RelationRule {
    kind: &'static str,
    label: &'static str,
    branches: &'static [&'static str],
    description: &'static str,
}

pub fn enrich_saju_result(result: &mut Value) -> bool {
    if result
        .get("four_pillars_detail")
        .and_then(|v| v.as_object())
        .is_none()
    {
        return false;
    }

    let Some(obj) = result.as_object_mut() else {
        return false;
    };
    let snapshot = Value::Object(obj.clone());
    let branches = pillar_branches(&snapshot);
    let branch_values: Vec<&str> = branches.iter().map(|(_, b)| *b).collect();
    let now = Utc::now().with_timezone(&FixedOffset::east_opt(9 * 3600).expect("valid KST offset"));

    obj.insert(
        "dalgyeol_enrichment_version".into(),
        json!(ENRICHMENT_VERSION),
    );
    obj.insert(
        "hidden_stems".into(),
        json!(hidden_stem_sections(&branches)),
    );
    obj.insert(
        "branch_relations".into(),
        json!(branch_relations(&branches)),
    );
    obj.insert("naeum".into(), json!(naeum_sections(&snapshot)));
    obj.insert(
        "yin_yang_balance".into(),
        json!(yin_yang_balance(&branches, &snapshot)),
    );
    obj.insert(
        "ten_gods_summary".into(),
        json!(ten_gods_summary(&snapshot)),
    );
    obj.insert("wolryeong".into(), json!(wolryeong(&branches)));
    obj.insert("seasonal_energy".into(), json!(seasonal_energy(&branches)));
    obj.insert(
        "strength_profile".into(),
        json!(strength_profile(&snapshot)),
    );
    obj.insert(
        "annual_fortune".into(),
        json!(annual_fortune(now.year(), &branch_values)),
    );
    obj.insert(
        "monthly_fortunes".into(),
        json!(monthly_fortunes(now.year(), &branch_values)),
    );
    obj.insert(
        "life_timeline".into(),
        json!(life_timeline(now.year(), &branch_values)),
    );
    obj.insert(
        "domain_fortunes".into(),
        json!(domain_fortunes(now.year(), &branch_values)),
    );
    obj.insert("ai_prompts".into(), json!(ai_prompts()));
    true
}

pub fn is_current_enriched_saju(result: &Value) -> bool {
    result
        .get("dalgyeol_enrichment_version")
        .and_then(|v| v.as_str())
        == Some(ENRICHMENT_VERSION)
}

fn pillar_branches(result: &Value) -> Vec<(&'static str, &str)> {
    let Some(detail) = result
        .get("four_pillars_detail")
        .and_then(|v| v.as_object())
    else {
        return Vec::new();
    };
    ["year", "month", "day", "hour"]
        .into_iter()
        .filter_map(|pillar| {
            detail
                .get(pillar)
                .and_then(|p| p.get("branch"))
                .and_then(|b| b.get("korean").or_else(|| b.get("name")))
                .and_then(|v| v.as_str())
                .and_then(first_branch_char)
                .map(|branch| (pillar, branch))
        })
        .collect()
}

fn first_branch_char(value: &str) -> Option<&str> {
    BRANCHES.iter().find(|b| value.contains(b.ko)).map(|b| b.ko)
}

fn first_stem_char(value: &str) -> Option<&str> {
    STEM_CYCLE.iter().find(|s| value.contains(*s)).copied()
}

fn korean_stem_branch_pair(value: &str) -> Option<&str> {
    NAEUM_TABLE
        .iter()
        .map(|(pair, _, _, _)| *pair)
        .find(|pair| value.contains(pair))
}

fn hidden_stem_sections(branches: &[(&'static str, &str)]) -> Vec<Value> {
    branches
        .iter()
        .filter_map(|(pillar, branch)| {
            let stems = hidden_stems_for(branch)?;
            Some(json!({
                "pillar": pillar,
                "pillar_label": pillar_label(pillar),
                "branch": branch,
                "stems": stems.iter().map(|s| json!({
                    "korean": s.ko,
                    "element": s.element,
                    "role": s.role,
                })).collect::<Vec<_>>(),
                "interpretation": format!("{}지 {} 안에는 {} 기운이 숨어 있어 겉으로 보이는 성향 뒤의 실제 동기를 봅니다.",
                    pillar_label(pillar),
                    branch,
                    stems.iter().map(|s| s.ko).collect::<Vec<_>>().join("·")
                )
            }))
        })
        .collect()
}

fn branch_relations(branches: &[(&'static str, &str)]) -> Vec<Value> {
    let branch_set: HashSet<&str> = branches.iter().map(|(_, branch)| *branch).collect();
    RELATIONS
        .iter()
        .filter(|rule| rule.branches.iter().all(|b| branch_set.contains(b)))
        .map(|rule| {
            let positions = branches
                .iter()
                .filter(|(_, branch)| rule.branches.contains(branch))
                .map(|(pillar, branch)| json!({ "pillar": pillar, "label": pillar_label(pillar), "branch": branch }))
                .collect::<Vec<_>>();
            json!({
                "type": rule.kind,
                "label": rule.label,
                "branches": rule.branches,
                "positions": positions,
                "description": rule.description,
            })
        })
        .collect()
}

fn naeum_sections(result: &Value) -> Vec<Value> {
    let Some(pillars) = result.get("four_pillars").and_then(|v| v.as_object()) else {
        return Vec::new();
    };
    ["year", "month", "day", "hour"]
        .into_iter()
        .filter_map(|pillar| {
            let text = pillars.get(pillar)?.as_str()?;
            let pair = korean_stem_branch_pair(text)?;
            let (name, element, meaning) = naeum_for(pair)?;
            Some(json!({
                "pillar": pillar,
                "pillar_label": pillar_label(pillar),
                "stem_branch": pair,
                "name": name,
                "element": element,
                "meaning": meaning,
            }))
        })
        .collect()
}

fn yin_yang_balance(branches: &[(&'static str, &str)], result: &Value) -> Value {
    let mut yang = 0;
    let mut yin = 0;
    if let Some(detail) = result
        .get("four_pillars_detail")
        .and_then(|v| v.as_object())
    {
        for pillar in ["year", "month", "day", "hour"] {
            if let Some(stem) = detail
                .get(pillar)
                .and_then(|p| p.get("stem"))
                .and_then(|s| s.get("korean"))
                .and_then(|v| v.as_str())
                .and_then(first_stem_char)
            {
                if is_yang_stem(stem) {
                    yang += 1;
                } else {
                    yin += 1;
                }
            }
        }
    }
    for (_, branch) in branches {
        if is_yang_branch(branch) {
            yang += 1;
        } else {
            yin += 1;
        }
    }
    let tendency = if yang > yin {
        "양 기운 우세"
    } else if yin > yang {
        "음 기운 우세"
    } else {
        "음양 균형"
    };
    json!({
        "yang": yang,
        "yin": yin,
        "tendency": tendency,
        "summary": format!("음양 분포는 양 {}개, 음 {}개로 {} 흐름입니다.", yang, yin, tendency),
        "advice": if yang > yin {
            "속도와 추진력이 강해지기 쉬우니 멈춰서 확인하는 루틴이 도움이 됩니다."
        } else if yin > yang {
            "관찰과 축적은 좋지만 결정이 늦어지지 않게 작은 실행 단위를 정하세요."
        } else {
            "밀고 나가는 힘과 받아들이는 힘이 비교적 균형을 이룹니다."
        },
    })
}

fn ten_gods_summary(result: &Value) -> Value {
    let gods = result
        .get("ten_gods")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let counts: Vec<(&str, usize)> = TEN_GOD_LABELS
        .iter()
        .map(|label| {
            let count = gods
                .iter()
                .filter(|entry| entry.get("god").and_then(|v| v.as_str()) == Some(*label))
                .count();
            (*label, count)
        })
        .collect();
    let prominent = counts
        .iter()
        .filter(|(_, count)| *count > 0)
        .map(|(label, count)| json!({ "god": label, "count": count }))
        .collect::<Vec<_>>();
    let missing = counts
        .iter()
        .filter(|(_, count)| *count == 0)
        .map(|(label, _)| json!({ "god": label, "meaning": ten_god_plain_meaning(label) }))
        .collect::<Vec<_>>();
    let strongest = counts
        .iter()
        .max_by_key(|(_, count)| *count)
        .filter(|(_, count)| *count > 0)
        .map(|(label, count)| {
            json!({
                "god": label,
                "count": count,
                "meaning": ten_god_plain_meaning(label),
            })
        });
    json!({
        "prominent": prominent,
        "missing": missing,
        "strongest": strongest,
        "summary": "십신은 성향을 역할 언어로 바꿔 보는 영역입니다. 많은 십신은 익숙한 방식, 비어 있는 십신은 의식적으로 보완할 과제로 봅니다.",
    })
}

fn seasonal_energy(branches: &[(&'static str, &str)]) -> Value {
    let month_branch = branches
        .iter()
        .find(|(pillar, _)| *pillar == "month")
        .map(|(_, branch)| *branch)
        .unwrap_or("미");
    let info = branch_info(month_branch).unwrap_or(BRANCHES[7]);
    json!({
        "month_branch": month_branch,
        "season": info.season,
        "dominant_element": info.element,
        "summary": format!("월지는 사주의 계절감입니다. {}월지는 {} 기운이 기본 배경이라 판단과 행동의 리듬이 여기서 강하게 드러납니다.", month_branch, info.element),
        "advice": seasonal_advice(info.element),
    })
}

fn wolryeong(branches: &[(&'static str, &str)]) -> Value {
    let month_branch = branches
        .iter()
        .find(|(pillar, _)| *pillar == "month")
        .map(|(_, branch)| *branch)
        .unwrap_or("미");
    let info = branch_info(month_branch).unwrap_or(BRANCHES[7]);
    json!({
        "branch": month_branch,
        "branch_label": format!("{}월지", month_branch),
        "season": info.season,
        "dominant_element": info.element,
        "meaning": "월령은 태어난 달의 계절 기운입니다. 사주 전체의 온도와 세기를 판단할 때 가장 먼저 보는 배경값입니다.",
        "interpretation": format!("{}월령은 {} 계절감이 강해, 같은 일간이라도 판단과 행동의 기본 리듬이 {} 쪽으로 기울기 쉽습니다.", month_branch, info.season, info.element),
        "confidence_note": "월령은 중요한 근거지만 단독 결론은 아닙니다. 일간, 오행 분포, 지장간, 대운 흐름과 함께 보아야 합니다.",
    })
}

fn strength_profile(result: &Value) -> Value {
    let day_element = result
        .get("day_master")
        .and_then(|v| v.get("element"))
        .and_then(|v| v.as_str())
        .and_then(element_label_from_text)
        .unwrap_or("토");
    let same = balance_count(result, day_element);
    let resource_element = generating_element(day_element);
    let output_element = generated_element(day_element);
    let wealth_element = controlled_element(day_element);
    let authority_element = controlling_element(day_element);
    let resource = balance_count(result, resource_element);
    let output = balance_count(result, output_element);
    let wealth = balance_count(result, wealth_element);
    let authority = balance_count(result, authority_element);
    let raw_score = 50 + same * 12 + resource * 10 - output * 6 - wealth * 7 - authority * 8;
    let score = raw_score.clamp(20, 90);
    let strength = if score >= 62 {
        "신강"
    } else if score <= 45 {
        "신약"
    } else {
        "중화"
    };
    let useful = useful_elements_for(strength, day_element);
    json!({
        "day_master_element": day_element,
        "strength": strength,
        "score": score,
        "basis": {
            "same_element": { "element": day_element, "count": same },
            "resource_element": { "element": resource_element, "count": resource },
            "output_element": { "element": output_element, "count": output },
            "wealth_element": { "element": wealth_element, "count": wealth },
            "authority_element": { "element": authority_element, "count": authority },
        },
        "useful_elements": useful.iter().map(|(role, element, reason)| json!({
            "role": role,
            "element": element,
            "reason": reason,
        })).collect::<Vec<_>>(),
        "summary": format!("일간 {} 기준으로 같은 기운 {}개, 도와주는 기운 {}개를 보고 {} 흐름으로 추정합니다.", day_element, same, resource, strength),
        "confidence_note": "이 값은 오행 분포와 일간 관계를 이용한 달결의 1차 추정입니다. 정밀 용신 판단은 월령, 조후, 지장간, 합충형해, 대운을 함께 본 뒤 보정해야 합니다.",
    })
}

fn annual_fortune(year: i32, branches: &[&str]) -> Value {
    let year_branch = cycle_branch(year);
    let relation = simple_relation(&year_branch, branches);
    json!({
        "year": year,
        "stem_branch": format!("{}{}", cycle_stem(year), year_branch),
        "theme": relation.0,
        "score": relation.1,
        "advice": format!("{}년은 원국의 지지와 {} 흐름이 생깁니다. 새 일을 벌이기보다 반복되는 패턴을 먼저 확인하세요.", year, relation.0),
    })
}

fn monthly_fortunes(year: i32, branches: &[&str]) -> Vec<Value> {
    BRANCH_CYCLE
        .iter()
        .enumerate()
        .map(|(idx, branch)| {
            let info = branch_info(branch).unwrap_or(BRANCHES[0]);
            let relation = simple_relation(branch, branches);
            json!({
                "month": idx + 1,
                "branch": branch,
                "element": info.element,
                "score": (relation.1 + ((idx as i32 % 3) - 1) * 3).clamp(45, 92),
                "theme": format!("{}월 {} 기운", idx + 1, info.element),
                "advice": format!("{}년 {}월은 {}. {}", year, idx + 1, relation.0, monthly_advice(info.element)),
            })
        })
        .collect()
}

fn life_timeline(year: i32, branches: &[&str]) -> Vec<Value> {
    let relation = simple_relation(cycle_branch(year).as_str(), branches);
    [
        (
            0,
            19,
            "기초 형성",
            "타고난 기질과 가족 환경의 영향이 강하게 남는 구간",
        ),
        (
            20,
            29,
            "방향 선택",
            "직업·관계·생활권의 선택지가 빠르게 넓어지는 구간",
        ),
        (30, 39, "역할 확장", "책임과 성취 욕구가 함께 커지는 구간"),
        (
            40,
            49,
            "재정비",
            "지금까지 쌓은 것을 골라내고 중심을 다시 잡는 구간",
        ),
        (
            50,
            59,
            "안정화",
            "일과 관계에서 오래 남길 구조를 만드는 구간",
        ),
        (60, 79, "전환", "속도보다 건강·관계·의미가 중요해지는 구간"),
        (80, 100, "정리", "남기는 것과 내려놓는 것을 구분하는 구간"),
    ]
    .into_iter()
    .enumerate()
    .map(|(idx, (start, end, title, summary))| {
        json!({
            "age_start": start,
            "age_end": end,
            "title": title,
            "summary": summary,
            "focus": if idx % 2 == 0 { relation.0 } else { "균형 회복" },
        })
    })
    .collect()
}

fn domain_fortunes(year: i32, branches: &[&str]) -> Vec<Value> {
    let base = simple_relation(cycle_branch(year).as_str(), branches).1;
    [
        ("overall", "총운", "지금은 강한 흐름과 약한 흐름을 함께 조정하는 시기입니다."),
        ("career", "직업운", "강점이 반복되는 역할에 집중하면 성과가 납니다."),
        ("wealth", "재물운", "돈의 크기보다 새는 구멍과 반복 지출을 먼저 봐야 합니다."),
        ("love", "연애운", "끌림이 빠를수록 관계의 속도를 의식적으로 늦추는 편이 좋습니다."),
        ("marriage", "결혼운", "생활 리듬과 책임 분담이 맞을 때 안정성이 커집니다."),
        ("health", "건강운", "무리한 몰입 뒤에 회복 시간이 부족해지지 않게 관리해야 합니다."),
        ("study", "학업운", "짧은 집중보다 반복 루틴이 결과를 만듭니다."),
        ("business", "사업운", "확장보다 구조화, 감보다 검증이 우선입니다."),
        ("children", "자녀운", "가르치고 돌보는 역할에서는 기준보다 리듬이 중요합니다."),
        ("travel", "여행운", "이동은 기분 전환보다 관점 전환의 의미가 큽니다."),
        ("move", "이동/변화운", "이사·이직·해외 이동은 준비 기간이 길수록 좋습니다."),
        ("relations", "인간관계운", "관계의 폭보다 오래 남을 역할 정리가 중요합니다."),
        ("family", "가족운", "가까운 관계일수록 말보다 역할 정리가 필요합니다."),
        ("mental", "심리운", "감정의 원인을 외부보다 반복 패턴에서 찾는 편이 빠릅니다."),
        ("image", "이미지운", "첫인상은 강점이지만 과하면 거리감으로 보일 수 있습니다."),
    ]
    .into_iter()
    .enumerate()
    .map(|(idx, (key, title, advice))| {
        let score = (base + ((idx as i32 % 5) - 2) * 4).clamp(45, 92);
        json!({
            "key": key,
            "title": title,
            "score": score,
            "summary": format!("{}은 원국의 균형과 현재 흐름을 함께 보아 {}점 흐름입니다.", title, score),
            "advice": advice,
        })
    })
    .collect()
}

fn ai_prompts() -> Vec<Value> {
    vec![
        json!({ "title": "직업 상담", "prompt": "내 사주 기준으로 잘 맞는 직업과 피해야 할 업무 방식을 설명해줘." }),
        json!({ "title": "연애 상담", "prompt": "내 사주에서 반복되는 연애 패턴과 관계 조언을 알려줘." }),
        json!({ "title": "올해 흐름", "prompt": "올해 세운과 월운 중 조심해야 할 달을 짚어줘." }),
        json!({ "title": "돈 관리", "prompt": "내 재물운에서 돈이 새는 패턴과 보완 방법을 정리해줘." }),
    ]
}

fn simple_relation(target: &str, branches: &[&str]) -> (&'static str, i32) {
    if branches.iter().any(|b| clash_pair(target, b)) {
        ("충돌과 변화", 58)
    } else if branches.iter().any(|b| harmony_pair(target, b)) {
        ("협력과 확장", 82)
    } else if branches.contains(&target) {
        ("반복과 강화", 74)
    } else {
        ("완만한 조정", 68)
    }
}

fn cycle_stem(year: i32) -> &'static str {
    STEM_CYCLE[((year - 4).rem_euclid(10)) as usize]
}

fn cycle_branch(year: i32) -> String {
    BRANCH_CYCLE[((year - 4).rem_euclid(12)) as usize].to_string()
}

fn monthly_advice(element: &str) -> &'static str {
    match element {
        "목" => "새 계획과 배움에 힘을 실으세요.",
        "화" => "표현은 좋지만 과열은 피하세요.",
        "토" => "정리와 약속 관리가 핵심입니다.",
        "금" => "결정과 기준을 분명히 하세요.",
        "수" => "정보 수집과 회복 시간을 확보하세요.",
        _ => "균형을 우선하세요.",
    }
}

fn seasonal_advice(element: &str) -> &'static str {
    match element {
        "목" => "성장 속도는 좋지만 마무리 기준을 따로 세워야 합니다.",
        "화" => "표현력은 강하지만 감정 소모를 줄이는 장치가 필요합니다.",
        "토" => "안정감은 좋지만 결정을 미루지 않는 연습이 필요합니다.",
        "금" => "판단력은 좋지만 관계에서 너무 딱딱해지지 않게 조절하세요.",
        "수" => "관찰력은 좋지만 생각이 길어질 때 실행 단위를 작게 쪼개세요.",
        _ => "강한 기운과 부족한 기운을 함께 보완하세요.",
    }
}

fn pillar_label(pillar: &str) -> &'static str {
    match pillar {
        "year" => "년주",
        "month" => "월주",
        "day" => "일주",
        "hour" => "시주",
        _ => "기둥",
    }
}

fn branch_info(branch: &str) -> Option<BranchInfo> {
    BRANCHES.iter().find(|info| info.ko == branch).copied()
}

fn balance_count(result: &Value, element: &str) -> i32 {
    let Some(key) = element_balance_key(element) else {
        return 0;
    };
    result
        .get("element_balance")
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32
}

fn element_label_from_text(text: &str) -> Option<&'static str> {
    ["목", "화", "토", "금", "수"]
        .into_iter()
        .find(|element| text.contains(element))
}

fn element_balance_key(element: &str) -> Option<&'static str> {
    match element {
        "목" => Some("wood"),
        "화" => Some("fire"),
        "토" => Some("earth"),
        "금" => Some("metal"),
        "수" => Some("water"),
        _ => None,
    }
}

fn generating_element(element: &str) -> &'static str {
    match element {
        "목" => "수",
        "화" => "목",
        "토" => "화",
        "금" => "토",
        "수" => "금",
        _ => "화",
    }
}

fn generated_element(element: &str) -> &'static str {
    match element {
        "목" => "화",
        "화" => "토",
        "토" => "금",
        "금" => "수",
        "수" => "목",
        _ => "금",
    }
}

fn controlled_element(element: &str) -> &'static str {
    match element {
        "목" => "토",
        "화" => "금",
        "토" => "수",
        "금" => "목",
        "수" => "화",
        _ => "수",
    }
}

fn controlling_element(element: &str) -> &'static str {
    match element {
        "목" => "금",
        "화" => "수",
        "토" => "목",
        "금" => "화",
        "수" => "토",
        _ => "목",
    }
}

fn useful_elements_for(
    strength: &str,
    day_element: &'static str,
) -> Vec<(&'static str, &'static str, &'static str)> {
    match strength {
        "신약" => vec![
            (
                "보강",
                day_element,
                "일간과 같은 기운을 보강해 중심을 세웁니다.",
            ),
            (
                "지원",
                generating_element(day_element),
                "일간을 생해주는 기운으로 회복력과 기반을 보완합니다.",
            ),
        ],
        "신강" => vec![
            (
                "표현",
                generated_element(day_element),
                "강한 기운을 결과물과 표현으로 흘려보냅니다.",
            ),
            (
                "현실화",
                controlled_element(day_element),
                "강한 추진력을 재물·성과·관리로 구체화합니다.",
            ),
        ],
        _ => vec![
            (
                "균형",
                generated_element(day_element),
                "정체되지 않도록 표현과 실행을 더합니다.",
            ),
            (
                "조율",
                controlling_element(day_element),
                "기준과 책임을 세워 균형을 유지합니다.",
            ),
        ],
    }
}

fn hidden_stems_for(branch: &str) -> Option<&'static [HiddenStem]> {
    HIDDEN_STEMS
        .iter()
        .find(|(b, _)| *b == branch)
        .map(|(_, stems)| *stems)
}

fn clash_pair(a: &str, b: &str) -> bool {
    [
        ["자", "오"],
        ["축", "미"],
        ["인", "신"],
        ["묘", "유"],
        ["진", "술"],
        ["사", "해"],
    ]
    .iter()
    .any(|pair| pair.contains(&a) && pair.contains(&b))
}

fn harmony_pair(a: &str, b: &str) -> bool {
    [
        ["자", "축"],
        ["인", "해"],
        ["묘", "술"],
        ["진", "유"],
        ["사", "신"],
        ["오", "미"],
    ]
    .iter()
    .any(|pair| pair.contains(&a) && pair.contains(&b))
}

fn is_yang_stem(stem: &str) -> bool {
    ["갑", "병", "무", "경", "임"].contains(&stem)
}

fn is_yang_branch(branch: &str) -> bool {
    ["자", "인", "진", "오", "신", "술"].contains(&branch)
}

fn naeum_for(pair: &str) -> Option<(&'static str, &'static str, &'static str)> {
    NAEUM_TABLE
        .iter()
        .find(|(p, _, _, _)| *p == pair)
        .map(|(_, name, element, meaning)| (*name, *element, *meaning))
}

fn ten_god_plain_meaning(god: &str) -> &'static str {
    match god {
        "비견(比肩)" => "독립성, 자존감, 동료성",
        "겁재(劫財)" => "경쟁, 협력, 지출 관리",
        "식신(食神)" => "생산성, 꾸준함, 건강한 표현",
        "상관(傷官)" => "창의성, 말, 규칙 밖의 표현",
        "편재(偏財)" => "기회 포착, 영업, 움직이는 돈",
        "정재(正財)" => "현실 감각, 저축, 안정 수입",
        "편관(偏官)" => "도전, 압박, 위기 대응",
        "정관(正官)" => "책임, 질서, 사회적 신뢰",
        "편인(偏印)" => "직감, 아이디어, 비정형 학습",
        "정인(正印)" => "보호, 공부, 안정된 지식",
        _ => "역할 기운",
    }
}

const STEM_CYCLE: [&str; 10] = ["갑", "을", "병", "정", "무", "기", "경", "신", "임", "계"];
const BRANCH_CYCLE: [&str; 12] = [
    "자", "축", "인", "묘", "진", "사", "오", "미", "신", "유", "술", "해",
];

const TEN_GOD_LABELS: [&str; 10] = [
    "비견(比肩)",
    "겁재(劫財)",
    "식신(食神)",
    "상관(傷官)",
    "편재(偏財)",
    "정재(正財)",
    "편관(偏官)",
    "정관(正官)",
    "편인(偏印)",
    "정인(正印)",
];

const NAEUM_TABLE: [(&str, &str, &str, &str); 60] = [
    (
        "갑자",
        "해중금",
        "금",
        "깊은 곳에 감춰진 금처럼 잠재력과 축적의 의미가 큽니다.",
    ),
    (
        "을축",
        "해중금",
        "금",
        "깊은 곳에 감춰진 금처럼 잠재력과 축적의 의미가 큽니다.",
    ),
    (
        "병인",
        "노중화",
        "화",
        "화로 속 불처럼 꾸준한 열기와 집중력을 뜻합니다.",
    ),
    (
        "정묘",
        "노중화",
        "화",
        "화로 속 불처럼 꾸준한 열기와 집중력을 뜻합니다.",
    ),
    (
        "무진",
        "대림목",
        "목",
        "큰 숲처럼 성장성과 보호 본능이 함께 드러납니다.",
    ),
    (
        "기사",
        "대림목",
        "목",
        "큰 숲처럼 성장성과 보호 본능이 함께 드러납니다.",
    ),
    (
        "경오",
        "노방토",
        "토",
        "길가의 흙처럼 사람과 현실을 이어주는 힘이 있습니다.",
    ),
    (
        "신미",
        "노방토",
        "토",
        "길가의 흙처럼 사람과 현실을 이어주는 힘이 있습니다.",
    ),
    (
        "임신",
        "검봉금",
        "금",
        "칼끝의 금처럼 판단과 절단의 힘이 선명합니다.",
    ),
    (
        "계유",
        "검봉금",
        "금",
        "칼끝의 금처럼 판단과 절단의 힘이 선명합니다.",
    ),
    (
        "갑술",
        "산두화",
        "화",
        "산 위의 불처럼 멀리 보이는 존재감과 명예욕을 뜻합니다.",
    ),
    (
        "을해",
        "산두화",
        "화",
        "산 위의 불처럼 멀리 보이는 존재감과 명예욕을 뜻합니다.",
    ),
    (
        "병자",
        "간하수",
        "수",
        "계곡물처럼 빠른 흐름과 적응력을 뜻합니다.",
    ),
    (
        "정축",
        "간하수",
        "수",
        "계곡물처럼 빠른 흐름과 적응력을 뜻합니다.",
    ),
    (
        "무인",
        "성두토",
        "토",
        "성벽의 흙처럼 방어와 기준을 세우는 힘입니다.",
    ),
    (
        "기묘",
        "성두토",
        "토",
        "성벽의 흙처럼 방어와 기준을 세우는 힘입니다.",
    ),
    (
        "경진",
        "백랍금",
        "금",
        "정제 전 금속처럼 다듬어질수록 가치가 커집니다.",
    ),
    (
        "신사",
        "백랍금",
        "금",
        "정제 전 금속처럼 다듬어질수록 가치가 커집니다.",
    ),
    (
        "임오",
        "양류목",
        "목",
        "버드나무처럼 유연하게 관계를 이어가는 힘입니다.",
    ),
    (
        "계미",
        "양류목",
        "목",
        "버드나무처럼 유연하게 관계를 이어가는 힘입니다.",
    ),
    (
        "갑신",
        "천중수",
        "수",
        "샘물처럼 맑은 지혜와 정보 흐름을 뜻합니다.",
    ),
    (
        "을유",
        "천중수",
        "수",
        "샘물처럼 맑은 지혜와 정보 흐름을 뜻합니다.",
    ),
    (
        "병술",
        "옥상토",
        "토",
        "지붕 위 흙처럼 보호와 경계의 의미가 있습니다.",
    ),
    (
        "정해",
        "옥상토",
        "토",
        "지붕 위 흙처럼 보호와 경계의 의미가 있습니다.",
    ),
    (
        "무자",
        "벽력화",
        "화",
        "번개불처럼 순간적인 결단과 반전의 힘입니다.",
    ),
    (
        "기축",
        "벽력화",
        "화",
        "번개불처럼 순간적인 결단과 반전의 힘입니다.",
    ),
    (
        "경인",
        "송백목",
        "목",
        "소나무처럼 오래 버티는 의지와 원칙을 뜻합니다.",
    ),
    (
        "신묘",
        "송백목",
        "목",
        "소나무처럼 오래 버티는 의지와 원칙을 뜻합니다.",
    ),
    (
        "임진",
        "장류수",
        "수",
        "큰 강물처럼 장기 흐름과 넓은 수용력을 뜻합니다.",
    ),
    (
        "계사",
        "장류수",
        "수",
        "큰 강물처럼 장기 흐름과 넓은 수용력을 뜻합니다.",
    ),
    (
        "갑오",
        "사중금",
        "금",
        "모래 속 금처럼 숨어 있는 가치와 선별의 의미가 있습니다.",
    ),
    (
        "을미",
        "사중금",
        "금",
        "모래 속 금처럼 숨어 있는 가치와 선별의 의미가 있습니다.",
    ),
    (
        "병신",
        "산하화",
        "화",
        "산 아래 불처럼 현실 속에서 쓰이는 표현력과 열기입니다.",
    ),
    (
        "정유",
        "산하화",
        "화",
        "산 아래 불처럼 현실 속에서 쓰이는 표현력과 열기입니다.",
    ),
    (
        "무술",
        "평지목",
        "목",
        "평지의 나무처럼 생활 기반과 성장성이 함께 있습니다.",
    ),
    (
        "기해",
        "평지목",
        "목",
        "평지의 나무처럼 생활 기반과 성장성이 함께 있습니다.",
    ),
    (
        "경자",
        "벽상토",
        "토",
        "벽 위의 흙처럼 경계와 보호, 구조를 세우는 힘입니다.",
    ),
    (
        "신축",
        "벽상토",
        "토",
        "벽 위의 흙처럼 경계와 보호, 구조를 세우는 힘입니다.",
    ),
    (
        "임인",
        "금박금",
        "금",
        "얇게 펴진 금처럼 세련된 표현과 가치 포장이 중요합니다.",
    ),
    (
        "계묘",
        "금박금",
        "금",
        "얇게 펴진 금처럼 세련된 표현과 가치 포장이 중요합니다.",
    ),
    (
        "갑진",
        "복등화",
        "화",
        "등불처럼 가까운 곳을 밝히는 꾸준한 지혜입니다.",
    ),
    (
        "을사",
        "복등화",
        "화",
        "등불처럼 가까운 곳을 밝히는 꾸준한 지혜입니다.",
    ),
    (
        "병오",
        "천하수",
        "수",
        "하늘의 물처럼 큰 흐름과 이상을 품는 기운입니다.",
    ),
    (
        "정미",
        "천하수",
        "수",
        "하늘의 물처럼 큰 흐름과 이상을 품는 기운입니다.",
    ),
    (
        "무신",
        "대역토",
        "토",
        "넓은 터전의 흙처럼 판을 깔고 일을 키우는 힘입니다.",
    ),
    (
        "기유",
        "대역토",
        "토",
        "넓은 터전의 흙처럼 판을 깔고 일을 키우는 힘입니다.",
    ),
    (
        "경술",
        "차천금",
        "금",
        "비녀와 장신구의 금처럼 정교함과 품격을 뜻합니다.",
    ),
    (
        "신해",
        "차천금",
        "금",
        "비녀와 장신구의 금처럼 정교함과 품격을 뜻합니다.",
    ),
    (
        "임자",
        "상자목",
        "목",
        "뽕나무처럼 실용성과 돌봄의 의미가 강합니다.",
    ),
    (
        "계축",
        "상자목",
        "목",
        "뽕나무처럼 실용성과 돌봄의 의미가 강합니다.",
    ),
    (
        "갑인",
        "대계수",
        "수",
        "큰 계곡물처럼 빠른 배움과 흐름 전환이 강합니다.",
    ),
    (
        "을묘",
        "대계수",
        "수",
        "큰 계곡물처럼 빠른 배움과 흐름 전환이 강합니다.",
    ),
    (
        "병진",
        "사중토",
        "토",
        "모래 속 흙처럼 유연하지만 기반을 만드는 힘입니다.",
    ),
    (
        "정사",
        "사중토",
        "토",
        "모래 속 흙처럼 유연하지만 기반을 만드는 힘입니다.",
    ),
    (
        "무오",
        "천상화",
        "화",
        "하늘의 불처럼 넓게 드러나는 명예와 표현력입니다.",
    ),
    (
        "기미",
        "천상화",
        "화",
        "하늘의 불처럼 넓게 드러나는 명예와 표현력입니다.",
    ),
    (
        "경신",
        "석류목",
        "목",
        "석류나무처럼 단단한 껍질 안에 성과를 품습니다.",
    ),
    (
        "신유",
        "석류목",
        "목",
        "석류나무처럼 단단한 껍질 안에 성과를 품습니다.",
    ),
    (
        "임술",
        "대해수",
        "수",
        "큰 바다처럼 넓은 포용력과 장기 흐름을 뜻합니다.",
    ),
    (
        "계해",
        "대해수",
        "수",
        "큰 바다처럼 넓은 포용력과 장기 흐름을 뜻합니다.",
    ),
];

const BRANCHES: [BranchInfo; 12] = [
    BranchInfo {
        ko: "자",
        element: "수",
        season: "겨울",
    },
    BranchInfo {
        ko: "축",
        element: "토",
        season: "겨울 끝",
    },
    BranchInfo {
        ko: "인",
        element: "목",
        season: "초봄",
    },
    BranchInfo {
        ko: "묘",
        element: "목",
        season: "봄",
    },
    BranchInfo {
        ko: "진",
        element: "토",
        season: "봄 끝",
    },
    BranchInfo {
        ko: "사",
        element: "화",
        season: "초여름",
    },
    BranchInfo {
        ko: "오",
        element: "화",
        season: "여름",
    },
    BranchInfo {
        ko: "미",
        element: "토",
        season: "여름 끝",
    },
    BranchInfo {
        ko: "신",
        element: "금",
        season: "초가을",
    },
    BranchInfo {
        ko: "유",
        element: "금",
        season: "가을",
    },
    BranchInfo {
        ko: "술",
        element: "토",
        season: "가을 끝",
    },
    BranchInfo {
        ko: "해",
        element: "수",
        season: "초겨울",
    },
];

const HIDDEN_STEMS: [(&str, &[HiddenStem]); 12] = [
    (
        "자",
        &[HiddenStem {
            ko: "계",
            element: "수",
            role: "본기",
        }],
    ),
    (
        "축",
        &[
            HiddenStem {
                ko: "기",
                element: "토",
                role: "본기",
            },
            HiddenStem {
                ko: "계",
                element: "수",
                role: "중기",
            },
            HiddenStem {
                ko: "신",
                element: "금",
                role: "여기",
            },
        ],
    ),
    (
        "인",
        &[
            HiddenStem {
                ko: "갑",
                element: "목",
                role: "본기",
            },
            HiddenStem {
                ko: "병",
                element: "화",
                role: "중기",
            },
            HiddenStem {
                ko: "무",
                element: "토",
                role: "여기",
            },
        ],
    ),
    (
        "묘",
        &[HiddenStem {
            ko: "을",
            element: "목",
            role: "본기",
        }],
    ),
    (
        "진",
        &[
            HiddenStem {
                ko: "무",
                element: "토",
                role: "본기",
            },
            HiddenStem {
                ko: "을",
                element: "목",
                role: "중기",
            },
            HiddenStem {
                ko: "계",
                element: "수",
                role: "여기",
            },
        ],
    ),
    (
        "사",
        &[
            HiddenStem {
                ko: "병",
                element: "화",
                role: "본기",
            },
            HiddenStem {
                ko: "무",
                element: "토",
                role: "중기",
            },
            HiddenStem {
                ko: "경",
                element: "금",
                role: "여기",
            },
        ],
    ),
    (
        "오",
        &[
            HiddenStem {
                ko: "정",
                element: "화",
                role: "본기",
            },
            HiddenStem {
                ko: "기",
                element: "토",
                role: "중기",
            },
        ],
    ),
    (
        "미",
        &[
            HiddenStem {
                ko: "기",
                element: "토",
                role: "본기",
            },
            HiddenStem {
                ko: "정",
                element: "화",
                role: "중기",
            },
            HiddenStem {
                ko: "을",
                element: "목",
                role: "여기",
            },
        ],
    ),
    (
        "신",
        &[
            HiddenStem {
                ko: "경",
                element: "금",
                role: "본기",
            },
            HiddenStem {
                ko: "임",
                element: "수",
                role: "중기",
            },
            HiddenStem {
                ko: "무",
                element: "토",
                role: "여기",
            },
        ],
    ),
    (
        "유",
        &[HiddenStem {
            ko: "신",
            element: "금",
            role: "본기",
        }],
    ),
    (
        "술",
        &[
            HiddenStem {
                ko: "무",
                element: "토",
                role: "본기",
            },
            HiddenStem {
                ko: "신",
                element: "금",
                role: "중기",
            },
            HiddenStem {
                ko: "정",
                element: "화",
                role: "여기",
            },
        ],
    ),
    (
        "해",
        &[
            HiddenStem {
                ko: "임",
                element: "수",
                role: "본기",
            },
            HiddenStem {
                ko: "갑",
                element: "목",
                role: "중기",
            },
        ],
    ),
];

const RELATIONS: &[RelationRule] = &[
    RelationRule {
        kind: "three_harmony",
        label: "삼합",
        branches: &["신", "자", "진"],
        description: "세 지지가 모여 하나의 수 기운을 강하게 만드는 흐름입니다.",
    },
    RelationRule {
        kind: "three_harmony",
        label: "삼합",
        branches: &["인", "오", "술"],
        description: "세 지지가 모여 하나의 화 기운을 강하게 만드는 흐름입니다.",
    },
    RelationRule {
        kind: "three_harmony",
        label: "삼합",
        branches: &["해", "묘", "미"],
        description: "세 지지가 모여 하나의 목 기운을 강하게 만드는 흐름입니다.",
    },
    RelationRule {
        kind: "three_harmony",
        label: "삼합",
        branches: &["사", "유", "축"],
        description: "세 지지가 모여 하나의 금 기운을 강하게 만드는 흐름입니다.",
    },
    RelationRule {
        kind: "six_harmony",
        label: "육합",
        branches: &["자", "축"],
        description: "서로 다른 기운이 협력 관계를 만들기 쉽습니다.",
    },
    RelationRule {
        kind: "six_harmony",
        label: "육합",
        branches: &["인", "해"],
        description: "서로 다른 기운이 협력 관계를 만들기 쉽습니다.",
    },
    RelationRule {
        kind: "six_harmony",
        label: "육합",
        branches: &["묘", "술"],
        description: "서로 다른 기운이 협력 관계를 만들기 쉽습니다.",
    },
    RelationRule {
        kind: "six_harmony",
        label: "육합",
        branches: &["진", "유"],
        description: "서로 다른 기운이 협력 관계를 만들기 쉽습니다.",
    },
    RelationRule {
        kind: "six_harmony",
        label: "육합",
        branches: &["사", "신"],
        description: "서로 다른 기운이 협력 관계를 만들기 쉽습니다.",
    },
    RelationRule {
        kind: "six_harmony",
        label: "육합",
        branches: &["오", "미"],
        description: "서로 다른 기운이 협력 관계를 만들기 쉽습니다.",
    },
    RelationRule {
        kind: "seasonal_harmony",
        label: "방합",
        branches: &["인", "묘", "진"],
        description: "봄의 방향으로 묶이는 기운이라 성장과 시작의 힘이 강해집니다.",
    },
    RelationRule {
        kind: "seasonal_harmony",
        label: "방합",
        branches: &["사", "오", "미"],
        description: "여름의 방향으로 묶이는 기운이라 표현과 확산의 힘이 강해집니다.",
    },
    RelationRule {
        kind: "seasonal_harmony",
        label: "방합",
        branches: &["신", "유", "술"],
        description: "가을의 방향으로 묶이는 기운이라 정리와 결실의 힘이 강해집니다.",
    },
    RelationRule {
        kind: "seasonal_harmony",
        label: "방합",
        branches: &["해", "자", "축"],
        description: "겨울의 방향으로 묶이는 기운이라 저장과 사고의 힘이 강해집니다.",
    },
    RelationRule {
        kind: "clash",
        label: "충",
        branches: &["자", "오"],
        description: "서로 맞부딪치는 기운이라 변화와 이동 신호가 강합니다.",
    },
    RelationRule {
        kind: "clash",
        label: "충",
        branches: &["축", "미"],
        description: "서로 맞부딪치는 기운이라 변화와 이동 신호가 강합니다.",
    },
    RelationRule {
        kind: "clash",
        label: "충",
        branches: &["인", "신"],
        description: "서로 맞부딪치는 기운이라 변화와 이동 신호가 강합니다.",
    },
    RelationRule {
        kind: "clash",
        label: "충",
        branches: &["묘", "유"],
        description: "서로 맞부딪치는 기운이라 변화와 이동 신호가 강합니다.",
    },
    RelationRule {
        kind: "clash",
        label: "충",
        branches: &["진", "술"],
        description: "서로 맞부딪치는 기운이라 변화와 이동 신호가 강합니다.",
    },
    RelationRule {
        kind: "clash",
        label: "충",
        branches: &["사", "해"],
        description: "서로 맞부딪치는 기운이라 변화와 이동 신호가 강합니다.",
    },
    RelationRule {
        kind: "punishment",
        label: "형",
        branches: &["인", "사", "신"],
        description: "강한 압박과 조급함이 생기기 쉬워 무리한 결정에 주의가 필요합니다.",
    },
    RelationRule {
        kind: "punishment",
        label: "형",
        branches: &["축", "술", "미"],
        description: "묵은 문제를 다시 건드리는 흐름이라 관계와 건강 관리가 중요합니다.",
    },
    RelationRule {
        kind: "punishment",
        label: "형",
        branches: &["자", "묘"],
        description: "감정선과 예민함이 부딪히기 쉬운 관계 신호입니다.",
    },
    RelationRule {
        kind: "break",
        label: "파",
        branches: &["자", "유"],
        description: "이미 잡힌 구조가 깨지기 쉬워 약속과 계약을 다시 확인해야 합니다.",
    },
    RelationRule {
        kind: "break",
        label: "파",
        branches: &["축", "진"],
        description: "이미 잡힌 구조가 깨지기 쉬워 약속과 계약을 다시 확인해야 합니다.",
    },
    RelationRule {
        kind: "break",
        label: "파",
        branches: &["인", "해"],
        description: "이미 잡힌 구조가 깨지기 쉬워 약속과 계약을 다시 확인해야 합니다.",
    },
    RelationRule {
        kind: "break",
        label: "파",
        branches: &["묘", "오"],
        description: "이미 잡힌 구조가 깨지기 쉬워 약속과 계약을 다시 확인해야 합니다.",
    },
    RelationRule {
        kind: "break",
        label: "파",
        branches: &["사", "신"],
        description: "이미 잡힌 구조가 깨지기 쉬워 약속과 계약을 다시 확인해야 합니다.",
    },
    RelationRule {
        kind: "break",
        label: "파",
        branches: &["미", "술"],
        description: "이미 잡힌 구조가 깨지기 쉬워 약속과 계약을 다시 확인해야 합니다.",
    },
    RelationRule {
        kind: "harm",
        label: "해",
        branches: &["자", "미"],
        description: "겉으로는 약해 보여도 누적 피로와 엇갈림을 만들 수 있습니다.",
    },
    RelationRule {
        kind: "harm",
        label: "해",
        branches: &["축", "오"],
        description: "겉으로는 약해 보여도 누적 피로와 엇갈림을 만들 수 있습니다.",
    },
    RelationRule {
        kind: "harm",
        label: "해",
        branches: &["인", "사"],
        description: "겉으로는 약해 보여도 누적 피로와 엇갈림을 만들 수 있습니다.",
    },
    RelationRule {
        kind: "harm",
        label: "해",
        branches: &["묘", "진"],
        description: "겉으로는 약해 보여도 누적 피로와 엇갈림을 만들 수 있습니다.",
    },
    RelationRule {
        kind: "harm",
        label: "해",
        branches: &["신", "해"],
        description: "겉으로는 약해 보여도 누적 피로와 엇갈림을 만들 수 있습니다.",
    },
    RelationRule {
        kind: "harm",
        label: "해",
        branches: &["유", "술"],
        description: "겉으로는 약해 보여도 누적 피로와 엇갈림을 만들 수 있습니다.",
    },
    RelationRule {
        kind: "wonjin",
        label: "원진",
        branches: &["자", "미"],
        description: "감정적으로 쉽게 걸리는 지점이라 관계 해석에서 따로 봅니다.",
    },
    RelationRule {
        kind: "wonjin",
        label: "원진",
        branches: &["축", "오"],
        description: "감정적으로 쉽게 걸리는 지점이라 관계 해석에서 따로 봅니다.",
    },
    RelationRule {
        kind: "wonjin",
        label: "원진",
        branches: &["인", "유"],
        description: "감정적으로 쉽게 걸리는 지점이라 관계 해석에서 따로 봅니다.",
    },
    RelationRule {
        kind: "wonjin",
        label: "원진",
        branches: &["묘", "신"],
        description: "감정적으로 쉽게 걸리는 지점이라 관계 해석에서 따로 봅니다.",
    },
    RelationRule {
        kind: "wonjin",
        label: "원진",
        branches: &["진", "해"],
        description: "감정적으로 쉽게 걸리는 지점이라 관계 해석에서 따로 봅니다.",
    },
    RelationRule {
        kind: "wonjin",
        label: "원진",
        branches: &["사", "술"],
        description: "감정적으로 쉽게 걸리는 지점이라 관계 해석에서 따로 봅니다.",
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_saju() -> Value {
        json!({
            "four_pillars": { "year": "갑자", "month": "을축", "day": "병인", "hour": "정묘" },
            "four_pillars_detail": {
                "year": { "branch": { "korean": "자" } },
                "month": { "branch": { "korean": "축" } },
                "day": { "branch": { "korean": "인" } },
                "hour": { "branch": { "korean": "묘" } }
            },
            "lead": { "signal": "s", "risk": "r", "action": "a", "question": "q" }
        })
    }

    #[test]
    fn enrich_adds_hidden_stems_and_relations() {
        let mut body = sample_saju();
        assert!(enrich_saju_result(&mut body));
        assert_eq!(body["hidden_stems"].as_array().unwrap().len(), 4);
        assert!(
            body["branch_relations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|rel| rel["label"] == "육합")
        );
    }

    #[test]
    fn enrich_adds_twelve_monthly_fortunes() {
        let mut body = sample_saju();
        enrich_saju_result(&mut body);
        assert_eq!(body["monthly_fortunes"].as_array().unwrap().len(), 12);
    }

    #[test]
    fn enrichment_version_marks_current() {
        let mut body = sample_saju();
        enrich_saju_result(&mut body);
        assert!(is_current_enriched_saju(&body));
    }
}
