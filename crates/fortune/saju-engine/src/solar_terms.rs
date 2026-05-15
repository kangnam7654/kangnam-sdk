//! 24절기 정밀 시각 테이블 (KST 기준, 1900-01-01 ~ 2100-12-31).
//!
//! `tables::solar_month_index`는 양력 근사값(소한=항상 1/6 등)을 쓰기 때문에
//! (a) 같은 날 절입 시각 전후로 갈리는 출생자 (b) 년도별 절기 일자 변동을
//! 정확히 처리하지 못한다. 이 모듈은 1900~2100 200년 ×24절기 시각을 정적
//! 데이터로 박아두고, 임의 datetime이 어느 月支에 속하는지 정밀하게 결정한다.
//!
//! 데이터 출처: `lunar-python` (6tail/lunar의 Java/Python 포트). KST(UTC+9)로
//! 변환된 unix timestamp + 절기 인덱스 (입춘=0..대한=23) 정렬 배열.
//!
//! 月支 boundary는 짝수 인덱스 절기:
//!   입춘(0)→인월(0), 경칩(2)→묘월(1), 청명(4)→진월(2), 입하(6)→사월(3),
//!   망종(8)→오월(4), 소서(10)→미월(5), 입추(12)→신월(6), 백로(14)→유월(7),
//!   한로(16)→술월(8), 입동(18)→해월(9), 대설(20)→자월(10), 소한(22)→축월(11).

use chrono::{FixedOffset, NaiveDate, TimeZone};

mod data {
    include!("solar_terms_data.rs");
}

/// KST 기준 datetime의 unix timestamp(초)를 받아 정밀한 月支 index(0=인월..11=축월)를 반환.
///
/// 타임라인을 binary search 해서 입력 timestamp 이하의 가장 최근 짝수 인덱스
/// 절기를 찾고, 그 절기에 매핑된 월지를 돌려준다.
pub fn precise_month_index_kst(timestamp_kst: i64) -> usize {
    // 입력 시점 이하의 entries 끝 index
    let upper = data::SOLAR_TERMS_KST.partition_point(|&(t, _)| t <= timestamp_kst);
    // 그 이하 영역에서 가장 최근 짝수(月支 boundary) 절기 찾기
    for i in (0..upper).rev() {
        let (_, term_idx) = data::SOLAR_TERMS_KST[i];
        if term_idx % 2 == 0 {
            // 입춘=0→인월(0), 경칩=2→묘월(1), ... 대설=20→자월(10), 소한=22→축월(11)
            return (term_idx / 2) as usize;
        }
    }
    // 데이터 범위 이전 (1900-01-01 이전): fallback to 양력 근사값 분기 안 함 — 호출 측 책임
    11
}

/// `(year, month, day, hour, minute)` KST 기준으로 月支 index 반환.
/// 1900~2100 범위 밖이면 `None`.
pub fn precise_month_index(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
) -> Option<usize> {
    if !(1900..=2100).contains(&year) {
        return None;
    }
    let kst = FixedOffset::east_opt(9 * 3600).unwrap();
    let dt = NaiveDate::from_ymd_opt(year, month, day)?.and_hms_opt(hour, minute, 0)?;
    let ts = kst.from_local_datetime(&dt).single()?.timestamp();
    Some(precise_month_index_kst(ts))
}

fn timestamp_kst(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> Option<i64> {
    let kst = FixedOffset::east_opt(9 * 3600).unwrap();
    let dt = NaiveDate::from_ymd_opt(year, month, day)?.and_hms_opt(hour, minute, 0)?;
    Some(kst.from_local_datetime(&dt).single()?.timestamp())
}

/// 출생 시각 기준 다음/이전 24절기까지의 일수.
///
/// 대운 시작 나이 계산에서 쓰는 값이다. `forward=true`면 출생 후 다음 절기,
/// `forward=false`면 출생 전 이전 절기를 찾는다. KST 기준이며 1900~2100
/// 정밀 절기 테이블 범위 밖이면 `None`.
pub fn adjacent_solar_term_days(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    forward: bool,
) -> Option<i32> {
    if !(1900..=2100).contains(&year) {
        return None;
    }
    let ts = timestamp_kst(year, month, day, hour, minute)?;
    let idx = data::SOLAR_TERMS_KST.partition_point(|&(term_ts, _)| term_ts <= ts);
    let term_ts = if forward {
        data::SOLAR_TERMS_KST.get(idx).map(|(term_ts, _)| *term_ts)
    } else {
        idx.checked_sub(1)
            .and_then(|prev| data::SOLAR_TERMS_KST.get(prev))
            .map(|(term_ts, _)| *term_ts)
    }?;
    let seconds = if forward { term_ts - ts } else { ts - term_ts };
    let days = ((seconds + 86_399) / 86_400).max(1);
    i32::try_from(days).ok()
}

/// 입춘 절입 시각 기준 정확한 효력 년도 결정. 입춘 이전이면 전년도.
/// 1900~2100 범위 밖이면 `None`.
pub fn effective_year_for_pillar(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
) -> Option<i32> {
    if !(1900..=2100).contains(&year) {
        return None;
    }
    let kst = FixedOffset::east_opt(9 * 3600).unwrap();
    let dt = NaiveDate::from_ymd_opt(year, month, day)?.and_hms_opt(hour, minute, 0)?;
    let ts = kst.from_local_datetime(&dt).single()?.timestamp();

    // 해당 년도의 입춘(term_idx=0) 절입 시각 찾기.
    let ipchun = data::SOLAR_TERMS_KST.iter().find(|(_, idx)| {
        *idx == 0u8 && {
            // 정확한 매칭: 해당 year의 입춘 entry — 단순히 첫 입춘이 아님.
            // 데이터는 1900~2100 정렬이라 year별 1번씩 존재. naive하게 year 기준 매칭.
            true
        }
    });
    let _ = ipchun;

    // year별 입춘 timestamp 정확히 찾기 — KST 변환된 datetime의 year 비교
    let kst_year = |ts: i64| -> i32 {
        chrono::DateTime::from_timestamp(ts, 0)
            .unwrap()
            .with_timezone(&kst)
            .date_naive()
            .format("%Y")
            .to_string()
            .parse()
            .unwrap_or(0)
    };

    let target_ipchun = data::SOLAR_TERMS_KST
        .iter()
        .find(|(t, idx)| *idx == 0u8 && kst_year(*t) == year)
        .map(|(t, _)| *t);

    let Some(ipchun_ts) = target_ipchun else {
        return None;
    };

    if ts < ipchun_ts {
        Some(year - 1)
    } else {
        Some(year)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 月支 index → 한글 (검증 가독성용)
    fn idx_to_branch(i: usize) -> &'static str {
        const NAMES: [&str; 12] = [
            "인", "묘", "진", "사", "오", "미", "신", "유", "술", "해", "자", "축",
        ];
        NAMES[i]
    }

    // ─── 데이터 무결성 ────────────────────────────────────────────

    #[test]
    fn data_is_sorted_ascending() {
        for w in data::SOLAR_TERMS_KST.windows(2) {
            assert!(
                w[0].0 <= w[1].0,
                "SOLAR_TERMS_KST not sorted: {} > {}",
                w[0].0,
                w[1].0
            );
        }
    }

    #[test]
    fn data_term_indices_in_range() {
        for (i, &(_, idx)) in data::SOLAR_TERMS_KST.iter().enumerate() {
            assert!(idx < 24, "entry {i} has invalid term_idx {idx}");
        }
    }

    #[test]
    fn data_no_duplicates() {
        let mut prev: Option<(i64, u8)> = None;
        for &entry in data::SOLAR_TERMS_KST.iter() {
            if let Some(p) = prev {
                assert_ne!(p, entry, "duplicate entry: {:?}", entry);
            }
            prev = Some(entry);
        }
    }

    #[test]
    fn data_covers_full_range() {
        // 1900~2100 200년 × 24절기 = 4800개 ± 절기 시작/끝 누락 가능. 4500 이상이면 충분.
        let n = data::SOLAR_TERMS_KST.len();
        assert!(n >= 4500, "expected >= 4500 entries, got {}", n);
        assert!(n <= 5000, "expected <= 5000 entries, got {}", n);
    }

    #[test]
    fn data_each_year_has_24_terms() {
        // 1950~2050 같은 안전 범위에서 각 년도가 24개를 가져야 함 (range 끝은 truncate 가능).
        use chrono::Datelike;
        use std::collections::HashMap;
        let kst = FixedOffset::east_opt(9 * 3600).unwrap();
        let mut by_year: HashMap<i32, Vec<u8>> = HashMap::new();
        for &(ts, idx) in data::SOLAR_TERMS_KST.iter() {
            let dt = chrono::DateTime::from_timestamp(ts, 0)
                .unwrap()
                .with_timezone(&kst);
            by_year.entry(dt.year()).or_default().push(idx);
        }
        for year in 1950..=2050 {
            let count = by_year.get(&year).map(|v| v.len()).unwrap_or(0);
            assert_eq!(count, 24, "year {} has {} terms (expected 24)", year, count);
        }
    }

    // ─── data-driven boundary 테스트 ──────────────────────────────────

    /// (year, month, day, hour, minute, expected_branch). 각 月支 boundary 절기에서
    /// 절입 ±1분 케이스. 1996년을 대표로 12개 月支 모두 cover.
    const FIXTURE_1996: &[(i32, u32, u32, u32, u32, &str)] = &[
        // 立春 (2/4 22:07) → 인월
        (1996, 2, 4, 22, 6, "축"),
        (1996, 2, 4, 22, 8, "인"),
        // 驚蟄 (3/5 22:09 경) — 정밀 데이터 누락 케이스 회귀 — 묘월
        // 清明 (4/4 21:02) → 진월
        (1996, 4, 4, 21, 1, "묘"),
        (1996, 4, 4, 21, 3, "진"),
        // 立夏 (5/5 14:26) → 사월
        (1996, 5, 5, 14, 25, "진"),
        (1996, 5, 5, 14, 27, "사"),
        // 小暑 (7/7 5:00) → 미월
        (1996, 7, 7, 4, 59, "오"),
        (1996, 7, 7, 5, 1, "미"),
        // 立秋 (8/7 14:48) → 신월
        (1996, 8, 7, 14, 47, "미"),
        (1996, 8, 7, 14, 49, "신"),
        // 白露 (9/7 17:42) → 유월
        (1996, 9, 7, 17, 41, "신"),
        (1996, 9, 7, 17, 43, "유"),
        // 寒露 (10/8 9:18) → 술월
        (1996, 10, 8, 9, 17, "유"),
        (1996, 10, 8, 9, 19, "술"),
        // 立冬 (11/7 12:26) → 해월
        (1996, 11, 7, 12, 25, "술"),
        (1996, 11, 7, 12, 27, "해"),
        // 大雪 (12/7 5:14) → 자월
        (1996, 12, 7, 5, 13, "해"),
        (1996, 12, 7, 5, 15, "자"),
        // 小寒 (1/6 10:31) → 축월
        (1996, 1, 6, 10, 30, "자"),
        (1996, 1, 6, 10, 32, "축"),
    ];

    #[test]
    fn boundary_cases_1996_all_branches() {
        for &(y, m, d, h, mi, want) in FIXTURE_1996 {
            let idx = precise_month_index(y, m, d, h, mi).unwrap();
            let got = idx_to_branch(idx);
            assert_eq!(
                got, want,
                "{}-{}-{} {:02}:{:02} → expected {} but got {}",
                y, m, d, h, mi, want, got
            );
        }
    }

    /// 2025 입춘이 양력 2/3 23:10 KST이라 흔한 가정(2/4)과 다른 케이스. 회귀 보호.
    #[test]
    fn ipchun_2025_falls_on_feb_3rd() {
        // 2/3 22:00 → 입춘 전 → 축월 (전년도 효력)
        assert_eq!(
            idx_to_branch(precise_month_index(2025, 2, 3, 22, 0).unwrap()),
            "축"
        );
        // 2/4 00:00 → 입춘 후 → 인월
        assert_eq!(
            idx_to_branch(precise_month_index(2025, 2, 4, 0, 0).unwrap()),
            "인"
        );
        assert_eq!(effective_year_for_pillar(2025, 2, 3, 22, 0), Some(2024));
        assert_eq!(effective_year_for_pillar(2025, 2, 4, 0, 0), Some(2025));
    }

    /// 2017 소한이 양력 1/5 12:55 KST. 양력 근사값(1/6)을 쓰면 1/5 출생자
    /// 모두 자월로 잘못 잡히는 케이스. 회귀 보호.
    #[test]
    fn sohan_2017_falls_on_jan_5th() {
        assert_eq!(
            idx_to_branch(precise_month_index(2017, 1, 5, 10, 0).unwrap()),
            "자",
            "절입 12:55 전 — 자월"
        );
        assert_eq!(
            idx_to_branch(precise_month_index(2017, 1, 5, 14, 0).unwrap()),
            "축",
            "절입 12:55 후 — 축월"
        );
    }

    /// 한 시간 안 동시각 boundary (밀리초 단위 정확도 — 분 단위로 충분)
    #[test]
    fn within_one_minute_of_jeolib() {
        // 2024 입춘 = 2/4 17:27 KST.
        assert_eq!(
            idx_to_branch(precise_month_index(2024, 2, 4, 17, 26).unwrap()),
            "축"
        );
        assert_eq!(
            idx_to_branch(precise_month_index(2024, 2, 4, 17, 28).unwrap()),
            "인"
        );
    }

    /// 데이터 범위 끝 (1900-01-01, 2100-12-31)에서 월지가 일관성 있는지
    #[test]
    fn at_data_range_extremes() {
        // 1900-01-01 — 1900 입춘 전이라 효력년도 1899. 月支는 축(소한 후 가정)
        let idx = precise_month_index(1900, 1, 15, 12, 0).unwrap();
        // 1900 소한 = 1/6 ~. 1/15는 소한 후 → 축월
        assert_eq!(idx_to_branch(idx), "축");

        // 2100-12-31 — 2100 대설 후 → 자월
        let idx = precise_month_index(2100, 12, 31, 12, 0).unwrap();
        assert_eq!(idx_to_branch(idx), "자");
    }

    /// 6월 망종/하지 사이 — 정밀 데이터에 망종이 있어야 정상
    #[test]
    fn mid_year_terms_present() {
        // 2024 망종 = 6/5 20:10 KST. 6/15는 망종 후 → 오월
        let idx = precise_month_index(2024, 6, 15, 0, 0).unwrap();
        assert_eq!(idx_to_branch(idx), "오");
    }

    #[test]
    fn jeolgi_1996_sohan_before_jeolib() {
        // 1996 소한 = 1996-01-06 09:31 KST. 12:00 출생자는 절입 후 → 축월.
        // (이전 양력 근사 (1, 6) 기준이면 day=6에 도달했으므로 우연히 같음.)
        let idx = precise_month_index(1996, 1, 6, 12, 0).unwrap();
        assert_eq!(idx_to_branch(idx), "축", "got {}", idx_to_branch(idx));
    }

    #[test]
    fn jeolgi_1996_sohan_early_morning() {
        // 1996-01-06 05:00 — 절입(09:31) 전 → 자월
        let idx = precise_month_index(1996, 1, 6, 5, 0).unwrap();
        assert_eq!(idx_to_branch(idx), "자", "got {}", idx_to_branch(idx));
    }

    #[test]
    fn jeolgi_2017_sohan_5th_after_jeolib() {
        // 2017 소한 = 2017-01-05 12:55 KST. 14:00 출생자 → 축월.
        // (양력 근사 (1, 6)이라 day < 6 → 자월로 잘못 계산되는 케이스.)
        let idx = precise_month_index(2017, 1, 5, 14, 0).unwrap();
        assert_eq!(idx_to_branch(idx), "축", "got {}", idx_to_branch(idx));
    }

    #[test]
    fn jeolgi_2017_sohan_5th_before_jeolib() {
        let idx = precise_month_index(2017, 1, 5, 10, 0).unwrap();
        assert_eq!(idx_to_branch(idx), "자", "got {}", idx_to_branch(idx));
    }

    #[test]
    fn jeolgi_2024_ipchun_before_jeolib() {
        // 2024 입춘 = 2024-02-04 17:27 KST. 12:00 출생자 → 절입 전 → 축월.
        let idx = precise_month_index(2024, 2, 4, 12, 0).unwrap();
        assert_eq!(idx_to_branch(idx), "축", "got {}", idx_to_branch(idx));
    }

    #[test]
    fn jeolgi_2024_ipchun_after_jeolib() {
        let idx = precise_month_index(2024, 2, 4, 20, 0).unwrap();
        assert_eq!(idx_to_branch(idx), "인", "got {}", idx_to_branch(idx));
    }

    #[test]
    fn effective_year_before_ipchun() {
        // 2024-01-15는 입춘(2/4) 전 → 효력 년도 2023
        assert_eq!(effective_year_for_pillar(2024, 1, 15, 12, 0), Some(2023));
    }

    #[test]
    fn effective_year_after_ipchun() {
        assert_eq!(effective_year_for_pillar(2024, 3, 15, 12, 0), Some(2024));
    }

    #[test]
    fn effective_year_2024_ipchun_same_day() {
        // 2024 입춘 절입 17:27. 12:00은 전, 20:00은 후
        assert_eq!(effective_year_for_pillar(2024, 2, 4, 12, 0), Some(2023));
        assert_eq!(effective_year_for_pillar(2024, 2, 4, 20, 0), Some(2024));
    }

    #[test]
    fn out_of_range_returns_none() {
        assert_eq!(precise_month_index(1899, 6, 1, 12, 0), None);
        assert_eq!(precise_month_index(2101, 6, 1, 12, 0), None);
    }
}
