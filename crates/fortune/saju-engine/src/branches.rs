use super::types::{Branch, Element};

/// 삼합 결과
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SamhapResult {
    Full(Element),
    Half(Element),
}

/// 육합 결과
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YukhapResult {
    pub pair: (Branch, Branch),
    pub element: Element,
}

/// 상충 결과
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClashResult {
    pub pair: (Branch, Branch),
}

/// 상형 유형
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PunishmentType {
    Ingratitude, // 무은지형 (寅巳申)
    Power,       // 지세지형 (丑戌未)
    Rudeness,    // 무례지형 (子卯)
    SelfHarm,    // 자형 (辰辰, 午午, 酉酉, 亥亥)
}

/// 상형 결과
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PunishmentResult {
    pub branches: Vec<Branch>,
    pub punishment_type: PunishmentType,
}

/// 지지 관계 종합 분석 결과
#[derive(Debug, Clone)]
pub struct BranchAnalysis {
    pub samhap: Vec<SamhapResult>,
    pub yukhap: Vec<YukhapResult>,
    pub clashes: Vec<ClashResult>,
    pub punishments: Vec<PunishmentResult>,
    pub samhap_count: usize,
    pub yukhap_count: usize,
    pub sangchung_count: usize,
    pub sanghyeong_count: usize,
}

// 삼합 그룹
const SAMHAP_GROUPS: [([Branch; 3], Element); 4] = [
    ([Branch::In, Branch::O, Branch::Sul], Element::Fire),
    ([Branch::Sa, Branch::Yu, Branch::Chuk], Element::Metal),
    ([Branch::Sin, Branch::Ja, Branch::Jin], Element::Water),
    ([Branch::Hae, Branch::Myo, Branch::Mi], Element::Wood),
];

// 육합 쌍
const YUKHAP_PAIRS: [(Branch, Branch, Element); 6] = [
    (Branch::Ja, Branch::Chuk, Element::Earth),
    (Branch::In, Branch::Hae, Element::Wood),
    (Branch::Myo, Branch::Sul, Element::Fire),
    (Branch::Jin, Branch::Yu, Element::Metal),
    (Branch::Sa, Branch::Sin, Element::Water),
    (Branch::O, Branch::Mi, Element::Fire),
];

// 상충 쌍
const SANGCHUNG_PAIRS: [(Branch, Branch); 6] = [
    (Branch::Ja, Branch::O),
    (Branch::Chuk, Branch::Mi),
    (Branch::In, Branch::Sin),
    (Branch::Myo, Branch::Yu),
    (Branch::Jin, Branch::Sul),
    (Branch::Sa, Branch::Hae),
];

// 자형 대상
const SELF_HARM_BRANCHES: [Branch; 4] = [Branch::Jin, Branch::O, Branch::Yu, Branch::Hae];

/// 삼합 탐색: 양측 지지를 합쳐서 삼합 완성/반합 검출
pub fn find_samhap(branches: &[Branch]) -> Vec<SamhapResult> {
    SAMHAP_GROUPS
        .iter()
        .filter_map(|(group, element)| {
            let count = group.iter().filter(|b| branches.contains(b)).count();
            match count {
                3 => Some(SamhapResult::Full(*element)),
                2 => Some(SamhapResult::Half(*element)),
                _ => None,
            }
        })
        .collect()
}

/// 육합 탐색: 한쪽 지지와 다른쪽 지지 사이에서 쌍 검출
pub fn find_yukhap(subject_branches: &[Branch], target_branches: &[Branch]) -> Vec<YukhapResult> {
    YUKHAP_PAIRS
        .iter()
        .filter_map(|(a, b, element)| {
            let found = (subject_branches.contains(a) && target_branches.contains(b))
                || (subject_branches.contains(b) && target_branches.contains(a));
            found.then_some(YukhapResult {
                pair: (*a, *b),
                element: *element,
            })
        })
        .collect()
}

/// 상충 탐색: 한쪽 지지와 다른쪽 지지 사이에서 충돌 검출
pub fn find_clashes(subject_branches: &[Branch], target_branches: &[Branch]) -> Vec<ClashResult> {
    SANGCHUNG_PAIRS
        .iter()
        .filter_map(|(a, b)| {
            let found = (subject_branches.contains(a) && target_branches.contains(b))
                || (subject_branches.contains(b) && target_branches.contains(a));
            found.then_some(ClashResult { pair: (*a, *b) })
        })
        .collect()
}

/// 상형 탐색: 양측 지지를 합쳐서 형벌 관계 검출
pub fn find_punishments(
    subject_branches: &[Branch],
    target_branches: &[Branch],
) -> Vec<PunishmentResult> {
    let all: Vec<Branch> = subject_branches
        .iter()
        .chain(target_branches.iter())
        .copied()
        .collect();
    let mut results = Vec::new();

    // 무은지형: 寅巳申
    {
        let group = [Branch::In, Branch::Sa, Branch::Sin];
        let count = group.iter().filter(|b| all.contains(b)).count();
        if count >= 2 {
            let matched: Vec<Branch> = group.iter().filter(|b| all.contains(b)).copied().collect();
            results.push(PunishmentResult {
                branches: matched,
                punishment_type: PunishmentType::Ingratitude,
            });
        }
    }

    // 지세지형: 丑戌未
    {
        let group = [Branch::Chuk, Branch::Sul, Branch::Mi];
        let count = group.iter().filter(|b| all.contains(b)).count();
        if count >= 2 {
            let matched: Vec<Branch> = group.iter().filter(|b| all.contains(b)).copied().collect();
            results.push(PunishmentResult {
                branches: matched,
                punishment_type: PunishmentType::Power,
            });
        }
    }

    // 무례지형: 子卯
    if all.contains(&Branch::Ja) && all.contains(&Branch::Myo) {
        results.push(PunishmentResult {
            branches: vec![Branch::Ja, Branch::Myo],
            punishment_type: PunishmentType::Rudeness,
        });
    }

    // 자형: 같은 지지가 양쪽에 모두 존재
    for &b in &SELF_HARM_BRANCHES {
        if subject_branches.contains(&b) && target_branches.contains(&b) {
            results.push(PunishmentResult {
                branches: vec![b],
                punishment_type: PunishmentType::SelfHarm,
            });
        }
    }

    results
}

/// 종합 분석: 두 사람의 지지를 분석하여 모든 관계를 반환
pub fn analyze(subject_branches: &[Branch], target_branches: &[Branch]) -> BranchAnalysis {
    let all_branches: Vec<Branch> = subject_branches
        .iter()
        .chain(target_branches.iter())
        .copied()
        .collect();

    let samhap = find_samhap(&all_branches);
    let yukhap = find_yukhap(subject_branches, target_branches);
    let clashes = find_clashes(subject_branches, target_branches);
    let punishments = find_punishments(subject_branches, target_branches);

    let samhap_count = samhap.len();
    let yukhap_count = yukhap.len();
    let sangchung_count = clashes.len();
    let sanghyeong_count = punishments.len();

    BranchAnalysis {
        samhap,
        yukhap,
        clashes,
        punishments,
        samhap_count,
        yukhap_count,
        sangchung_count,
        sanghyeong_count,
    }
}

/// 지지 관계 유형의 한국어 해석
pub fn relation_description(
    samhap: bool,
    yukhap: bool,
    clash: bool,
    punishment: bool,
) -> &'static str {
    if samhap {
        "세 가지 기운이 하나로 모여 강한 조화를 이룹니다."
    } else if yukhap {
        "자연스러운 끌림과 조화의 관계입니다."
    } else if clash {
        "서로 다른 에너지가 부딪혀 갈등이 생길 수 있습니다."
    } else if punishment {
        "관계에서 미묘한 갈등 요소가 있습니다."
    } else {
        ""
    }
}

impl PunishmentType {
    pub fn korean(&self) -> &'static str {
        match self {
            PunishmentType::Ingratitude => "무은지형(無恩之刑)",
            PunishmentType::Power => "지세지형(恃勢之刑)",
            PunishmentType::Rudeness => "무례지형(無禮之刑)",
            PunishmentType::SelfHarm => "자형(自刑)",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 삼합 테스트
    #[test]
    fn test_samhap_fire() {
        let branches = vec![Branch::In, Branch::O, Branch::Sul];
        let result = find_samhap(&branches);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], SamhapResult::Full(Element::Fire));
    }

    #[test]
    fn test_samhap_metal() {
        let branches = vec![Branch::Sa, Branch::Yu, Branch::Chuk];
        let result = find_samhap(&branches);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], SamhapResult::Full(Element::Metal));
    }

    #[test]
    fn test_samhap_water() {
        let branches = vec![Branch::Sin, Branch::Ja, Branch::Jin];
        let result = find_samhap(&branches);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], SamhapResult::Full(Element::Water));
    }

    #[test]
    fn test_samhap_wood() {
        let branches = vec![Branch::Hae, Branch::Myo, Branch::Mi];
        let result = find_samhap(&branches);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], SamhapResult::Full(Element::Wood));
    }

    #[test]
    fn test_samhap_half() {
        let branches = vec![Branch::In, Branch::O]; // 술 없음
        let result = find_samhap(&branches);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], SamhapResult::Half(Element::Fire));
    }

    // 육합 테스트
    #[test]
    fn test_yukhap_ja_chuk() {
        let s = vec![Branch::Ja];
        let t = vec![Branch::Chuk];
        let result = find_yukhap(&s, &t);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].pair, (Branch::Ja, Branch::Chuk));
        assert_eq!(result[0].element, Element::Earth);
    }

    #[test]
    fn test_yukhap_in_hae() {
        let s = vec![Branch::Hae];
        let t = vec![Branch::In];
        let result = find_yukhap(&s, &t);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].pair, (Branch::In, Branch::Hae));
    }

    #[test]
    fn test_yukhap_myo_sul() {
        let s = vec![Branch::Myo];
        let t = vec![Branch::Sul];
        let result = find_yukhap(&s, &t);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].pair, (Branch::Myo, Branch::Sul));
    }

    // 상충 테스트
    #[test]
    fn test_clash_ja_o() {
        let s = vec![Branch::Ja];
        let t = vec![Branch::O];
        let result = find_clashes(&s, &t);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].pair, (Branch::Ja, Branch::O));
    }

    #[test]
    fn test_clash_in_sin() {
        let s = vec![Branch::In];
        let t = vec![Branch::Sin];
        let result = find_clashes(&s, &t);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_clash_sa_hae() {
        let s = vec![Branch::Sa];
        let t = vec![Branch::Hae];
        let result = find_clashes(&s, &t);
        assert_eq!(result.len(), 1);
    }

    // 상형 테스트
    #[test]
    fn test_punishment_in_sa_sin() {
        let s = vec![Branch::In];
        let t = vec![Branch::Sa, Branch::Sin];
        let result = find_punishments(&s, &t);
        assert!(
            result
                .iter()
                .any(|r| r.punishment_type == PunishmentType::Ingratitude)
        );
    }

    #[test]
    fn test_punishment_ja_myo() {
        let s = vec![Branch::Ja];
        let t = vec![Branch::Myo];
        let result = find_punishments(&s, &t);
        assert!(
            result
                .iter()
                .any(|r| r.punishment_type == PunishmentType::Rudeness)
        );
    }

    // 종합 분석 테스트
    #[test]
    fn test_analyze_mixed() {
        // A: 인오술 (삼합 화), B: 자축 (육합) + 자오 (상충)
        let s = vec![Branch::In, Branch::O, Branch::Sul];
        let t = vec![Branch::Ja, Branch::Chuk, Branch::Mi];
        let result = analyze(&s, &t);
        assert!(result.samhap_count >= 1); // 인오술 삼합
        assert!(result.yukhap_count >= 1); // 자축 육합
        assert!(result.sangchung_count >= 1); // 자오 상충 or 축미 상충
    }

    #[test]
    fn test_no_relations() {
        let s = vec![Branch::In];
        let t = vec![Branch::Chuk];
        let result = analyze(&s, &t);
        assert_eq!(result.sangchung_count, 0);
        // 인해 육합이 아니므로 육합도 0
        assert_eq!(result.yukhap_count, 0);
    }
}
