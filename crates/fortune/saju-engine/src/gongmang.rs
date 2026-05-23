//! 공망(空亡) — 일주 기준 60갑자 그룹의 빈 지지 2개.
//!
//! 천간 10과 지지 12가 순서대로 짝을 지으면 천간이 한 바퀴 돌 때마다 지지
//! 2개가 짝을 못 찾고 남는다. 60갑자를 10개씩 6 그룹(순/旬)으로 나눴을 때
//! 각 그룹에서 비는 두 지지가 그 그룹에 속한 일주의 공망이다.
//!
//! - 갑자순(甲子旬, 일주 idx 0~9)   → 공망 술(戌)·해(亥)
//! - 갑술순(甲戌旬, 일주 idx 10~19) → 공망 신(申)·유(酉)
//! - 갑신순(甲申旬, 일주 idx 20~29) → 공망 오(午)·미(未)
//! - 갑오순(甲午旬, 일주 idx 30~39) → 공망 진(辰)·사(巳)
//! - 갑진순(甲辰旬, 일주 idx 40~49) → 공망 인(寅)·묘(卯)
//! - 갑인순(甲寅旬, 일주 idx 50~59) → 공망 자(子)·축(丑)
use crate::ten_gods::derive_ten_god;
use crate::types::{Branch, FourPillars, Pillar, Stem, TenGod};
use serde::Serialize;

/// 공망이 자리한 기둥 (일주 자기 자신은 제외 — 자기 일주의 공망은 자기 자신이 될 수 없음).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Palace {
    Year,
    Month,
    Hour,
}

/// 공망 계산값. 사용자-facing prose는 포함하지 않는다.
#[derive(Debug, Clone, Serialize)]
pub struct GongmangFacts {
    /// 일주가 속한 60갑자 그룹 인덱스 (0 = 갑자순, 5 = 갑인순).
    pub group_index: u8,
    /// 그룹 이름 (예: "갑자순(甲子旬)").
    pub group_name: &'static str,
    /// 그룹의 공망 지지 2개.
    pub empty_branches: [Branch; 2],
    /// 사주 원국에서 공망 지지가 자리한 기둥 (일주 제외).
    pub affected_palaces: Vec<Palace>,
    /// 공망 지지의 본기 천간 기준 십성 (일간과의 상대 관계).
    pub affected_ten_gods: Vec<TenGod>,
}

/// 일주(`pillars.day`)에서 공망을 산출하고 사주 원국과 비교한다.
///
/// `has_birth_time = false` 이면 시주를 비교 대상에서 제외 (출생 시각 미상 케이스).
pub fn calculate(pillars: &FourPillars, has_birth_time: bool) -> GongmangFacts {
    let day = pillars.day;
    let group_index = group_index_of(day);
    let empty = empty_branches_for_group(group_index);

    let mut affected_palaces = Vec::new();
    if empty.contains(&pillars.year.branch) {
        affected_palaces.push(Palace::Year);
    }
    if empty.contains(&pillars.month.branch) {
        affected_palaces.push(Palace::Month);
    }
    if has_birth_time && empty.contains(&pillars.hour.branch) {
        affected_palaces.push(Palace::Hour);
    }

    let dm = day.stem;
    let affected_ten_gods: Vec<TenGod> = empty
        .iter()
        .map(|b| derive_ten_god(dm, branch_primary_stem(*b)))
        .collect();

    let group_name = group_name(group_index);

    GongmangFacts {
        group_index,
        group_name,
        empty_branches: empty,
        affected_palaces,
        affected_ten_gods,
    }
}

/// 일주가 60갑자 어느 그룹(순, 旬)에 속하는지.
///
/// 60갑자 위치 n에서 `n % 10 = stem_idx`, `n % 12 = branch_idx`.
/// 유효한 (stem, branch) 쌍은 mod 60에서 unique한 n을 가지므로 k(0..6) 중
/// `(stem_idx + 10k) % 12 == branch_idx` 인 k가 그룹 인덱스.
fn group_index_of(pillar: Pillar) -> u8 {
    let stem_idx = pillar.stem.index();
    let branch_idx = pillar.branch.index();
    for k in 0..6u8 {
        let n = stem_idx + 10 * k as usize;
        if n % 12 == branch_idx {
            return k;
        }
    }
    // Unreachable for valid 60갑자 pairs (e.g. 갑축 같은 invalid pair는 만들 수 없음).
    0
}

/// 그룹 g의 공망 지지 두 개. 패턴: idx (10 - 2g) % 12, (11 - 2g) % 12.
fn empty_branches_for_group(g: u8) -> [Branch; 2] {
    let i1 = ((10 - 2 * g as i32).rem_euclid(12)) as usize;
    let i2 = ((11 - 2 * g as i32).rem_euclid(12)) as usize;
    [Branch::ALL[i1], Branch::ALL[i2]]
}

fn group_name(g: u8) -> &'static str {
    match g {
        0 => "갑자순(甲子旬)",
        1 => "갑술순(甲戌旬)",
        2 => "갑신순(甲申旬)",
        3 => "갑오순(甲午旬)",
        4 => "갑진순(甲辰旬)",
        5 => "갑인순(甲寅旬)",
        _ => "",
    }
}

/// 지지의 본기(本氣) 천간 — 십신 도출의 기준.
/// 지지 자체의 음양과 본기 천간의 음양이 다를 수 있으므로 (예: 자=양지지·계=음천간)
/// element + 음양을 정확히 보존하는 매핑이 중요.
fn branch_primary_stem(branch: Branch) -> Stem {
    match branch {
        Branch::Ja => Stem::Gye,     // 자 → 계수 (음수)
        Branch::Chuk => Stem::Gi,    // 축 → 기토 (음토)
        Branch::In => Stem::Gap,     // 인 → 갑목 (양목)
        Branch::Myo => Stem::Eul,    // 묘 → 을목 (음목)
        Branch::Jin => Stem::Mu,     // 진 → 무토 (양토)
        Branch::Sa => Stem::Byeong,  // 사 → 병화 (양화)
        Branch::O => Stem::Jeong,    // 오 → 정화 (음화)
        Branch::Mi => Stem::Gi,      // 미 → 기토 (음토)
        Branch::Sin => Stem::Gyeong, // 신 → 경금 (양금)
        Branch::Yu => Stem::Sin,     // 유 → 신금 (음금)
        Branch::Sul => Stem::Mu,     // 술 → 무토 (양토)
        Branch::Hae => Stem::Im,     // 해 → 임수 (양수)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 60갑자 첫 그룹 — 갑자순. 공망은 술/해.
    #[test]
    fn gapja_group_empty_is_sul_hae() {
        let pillar = Pillar::new(Stem::Gap, Branch::Ja); // 갑자
        let g = group_index_of(pillar);
        assert_eq!(g, 0);
        assert_eq!(empty_branches_for_group(g), [Branch::Sul, Branch::Hae]);
    }

    #[test]
    fn gapsul_group_empty_is_sin_yu() {
        let pillar = Pillar::new(Stem::Gap, Branch::Sul); // 갑술
        let g = group_index_of(pillar);
        assert_eq!(g, 1);
        assert_eq!(empty_branches_for_group(g), [Branch::Sin, Branch::Yu]);
    }

    #[test]
    fn gapsin_group_empty_is_o_mi() {
        let pillar = Pillar::new(Stem::Gap, Branch::Sin); // 갑신
        let g = group_index_of(pillar);
        assert_eq!(g, 2);
        assert_eq!(empty_branches_for_group(g), [Branch::O, Branch::Mi]);
    }

    #[test]
    fn gapo_group_empty_is_jin_sa() {
        let pillar = Pillar::new(Stem::Gap, Branch::O); // 갑오
        let g = group_index_of(pillar);
        assert_eq!(g, 3);
        assert_eq!(empty_branches_for_group(g), [Branch::Jin, Branch::Sa]);
    }

    #[test]
    fn gapjin_group_empty_is_in_myo() {
        let pillar = Pillar::new(Stem::Gap, Branch::Jin); // 갑진
        let g = group_index_of(pillar);
        assert_eq!(g, 4);
        assert_eq!(empty_branches_for_group(g), [Branch::In, Branch::Myo]);
    }

    #[test]
    fn gapin_group_empty_is_ja_chuk() {
        let pillar = Pillar::new(Stem::Gap, Branch::In); // 갑인
        let g = group_index_of(pillar);
        assert_eq!(g, 5);
        assert_eq!(empty_branches_for_group(g), [Branch::Ja, Branch::Chuk]);
    }

    /// 갑자순 마지막 일주 = 계유. 공망은 갑자순과 같은 술/해.
    #[test]
    fn gyeyu_in_gapja_group() {
        let pillar = Pillar::new(Stem::Gye, Branch::Yu); // 계유
        let g = group_index_of(pillar);
        assert_eq!(g, 0);
        assert_eq!(empty_branches_for_group(g), [Branch::Sul, Branch::Hae]);
    }

    /// 갑술순 마지막 일주 = 계미. 공망 신/유.
    #[test]
    fn gyemi_in_gapsul_group() {
        let pillar = Pillar::new(Stem::Gye, Branch::Mi); // 계미
        let g = group_index_of(pillar);
        assert_eq!(g, 1);
        assert_eq!(empty_branches_for_group(g), [Branch::Sin, Branch::Yu]);
    }

    /// 60갑자 모든 일주가 정확히 6 그룹 중 하나에 매핑됨.
    #[test]
    fn all_60_jiazi_pairs_map_to_six_groups() {
        let mut counts = [0; 6];
        for k in 0..60usize {
            let stem = Stem::from_index(k % 10);
            let branch = Branch::from_index(k % 12);
            let g = group_index_of(Pillar::new(stem, branch)) as usize;
            counts[g] += 1;
        }
        // 각 그룹마다 정확히 10개 일주.
        assert_eq!(counts, [10; 6]);
    }

    /// affected_palaces: 일주 갑자 + 연지 술 + 월지 해 → 두 자리 모두 검출.
    #[test]
    fn detect_affected_palaces() {
        let pillars = FourPillars {
            year: Pillar::new(Stem::Gyeong, Branch::Sul),
            month: Pillar::new(Stem::Gye, Branch::Hae),
            day: Pillar::new(Stem::Gap, Branch::Ja),
            hour: Pillar::new(Stem::Eul, Branch::Chuk),
        };
        let g = calculate(&pillars, true);
        assert_eq!(g.group_index, 0);
        assert_eq!(g.affected_palaces, vec![Palace::Year, Palace::Month]);
    }

    /// has_birth_time = false 이면 시지 공망은 무시.
    #[test]
    fn skip_hour_when_no_birth_time() {
        let pillars = FourPillars {
            year: Pillar::new(Stem::Gyeong, Branch::Sul),
            month: Pillar::new(Stem::Gi, Branch::Myo),
            day: Pillar::new(Stem::Gap, Branch::Ja),
            hour: Pillar::new(Stem::Eul, Branch::Hae), // 시지가 공망인데
        };
        let g = calculate(&pillars, false);
        assert!(!g.affected_palaces.contains(&Palace::Hour));
        assert!(g.affected_palaces.contains(&Palace::Year));
    }

    /// 공망이 사주 원국에 안 잡히면 affected_palaces가 비어야 한다.
    #[test]
    fn empty_palaces_when_no_branch_match() {
        let pillars = FourPillars {
            year: Pillar::new(Stem::Gap, Branch::Ja),
            month: Pillar::new(Stem::Eul, Branch::Chuk),
            day: Pillar::new(Stem::Gap, Branch::In), // 갑인 → 갑인순 → 공망 자/축
            hour: Pillar::new(Stem::Byeong, Branch::Myo),
        };
        // 일지 인은 공망 검사 대상 아님(자기 일주). 연지 자, 월지 축이 공망에 걸림 → 잡힘.
        let g = calculate(&pillars, true);
        assert_eq!(g.affected_palaces, vec![Palace::Year, Palace::Month]);
    }

    /// 본기 천간 매핑 잠금 — drift 방지.
    #[test]
    fn primary_stem_anchor_pairs() {
        assert_eq!(branch_primary_stem(Branch::Ja), Stem::Gye);
        assert_eq!(branch_primary_stem(Branch::In), Stem::Gap);
        assert_eq!(branch_primary_stem(Branch::Myo), Stem::Eul);
        assert_eq!(branch_primary_stem(Branch::O), Stem::Jeong);
        assert_eq!(branch_primary_stem(Branch::Yu), Stem::Sin);
        assert_eq!(branch_primary_stem(Branch::Hae), Stem::Im);
    }
}
