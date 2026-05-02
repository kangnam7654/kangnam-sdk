use super::types::{Branch, Stem};

/// 월지 고정 매핑: 음력 1월=인(寅), 2월=묘(卯), ..., 12월=축(丑)
pub const MONTH_BRANCHES: [Branch; 12] = [
    Branch::In,   // 1월 (양력 ~2월)
    Branch::Myo,  // 2월
    Branch::Jin,  // 3월
    Branch::Sa,   // 4월
    Branch::O,    // 5월
    Branch::Mi,   // 6월
    Branch::Sin,  // 7월
    Branch::Yu,   // 8월
    Branch::Sul,  // 9월
    Branch::Hae,  // 10월
    Branch::Ja,   // 11월
    Branch::Chuk, // 12월
];

/// 년간별 월간 시작 (갑/기→병인, 을/경→무인, 병/신→경인, 정/임→임인, 무/계→갑인)
/// 인월(1월) 천간 인덱스: 갑→2(병), 을→4(무), 병→6(경), 정→8(임), 무→0(갑)
pub const MONTH_STEM_START: [usize; 10] = [
    2, // 갑년 → 병인월
    4, // 을년 → 무인월
    6, // 병년 → 경인월
    8, // 정년 → 임인월
    0, // 무년 → 갑인월
    2, // 기년 → 병인월
    4, // 경년 → 무인월
    6, // 신년 → 경인월
    8, // 임년 → 임인월
    0, // 계년 → 갑인월
];

/// 시지 고정 매핑: 23-01시=자, 01-03시=축, ..., 21-23시=해
pub const HOUR_BRANCHES: [Branch; 12] = [
    Branch::Ja,   // 23:00-00:59
    Branch::Chuk, // 01:00-02:59
    Branch::In,   // 03:00-04:59
    Branch::Myo,  // 05:00-06:59
    Branch::Jin,  // 07:00-08:59
    Branch::Sa,   // 09:00-10:59
    Branch::O,    // 11:00-12:59
    Branch::Mi,   // 13:00-14:59
    Branch::Sin,  // 15:00-16:59
    Branch::Yu,   // 17:00-18:59
    Branch::Sul,  // 19:00-20:59
    Branch::Hae,  // 21:00-22:59
];

/// 일간별 시간 천간 시작 (자시 천간)
/// 갑/기→갑자, 을/경→병자, 병/신→무자, 정/임→경자, 무/계→임자
pub const HOUR_STEM_START: [usize; 10] = [
    0, // 갑일 → 갑자시
    2, // 을일 → 병자시
    4, // 병일 → 무자시
    6, // 정일 → 경자시
    8, // 무일 → 임자시
    0, // 기일 → 갑자시
    2, // 경일 → 병자시
    4, // 신일 → 무자시
    6, // 임일 → 경자시
    8, // 계일 → 임자시
];

/// 절기 기반 월 경계 (양력 기준 근사값)
/// (월, 일) = 해당 월의 절입일 (節入日)
/// 이 날짜 이전이면 이전 월로 취급
pub const SOLAR_MONTH_BOUNDARIES: [(u32, u32); 12] = [
    (2, 4),  // 입춘 → 인월(1월) 시작
    (3, 6),  // 경칩 → 묘월(2월)
    (4, 5),  // 청명 → 진월(3월)
    (5, 6),  // 입하 → 사월(4월)
    (6, 6),  // 망종 → 오월(5월)
    (7, 7),  // 소서 → 미월(6월)
    (8, 8),  // 입추 → 신월(7월)
    (9, 8),  // 백로 → 유월(8월)
    (10, 8), // 한로 → 술월(9월)
    (11, 7), // 입동 → 해월(10월)
    (12, 7), // 대설 → 자월(11월)
    (1, 6),  // 소한 → 축월(12월)
];

/// 양력 날짜로 사주 월 인덱스(0=인월 ~ 11=축월) 결정
pub fn solar_month_index(month: u32, day: u32) -> usize {
    // 절기 경계 확인: 해당 월의 절입일 이전이면 이전 달
    for (i, &(boundary_month, boundary_day)) in SOLAR_MONTH_BOUNDARIES.iter().enumerate() {
        if month == boundary_month && day >= boundary_day {
            return i;
        }
    }
    // 절입일 이전이면 이전 월
    for (i, &(boundary_month, _)) in SOLAR_MONTH_BOUNDARIES.iter().enumerate() {
        if month == boundary_month {
            return if i == 0 { 11 } else { i - 1 };
        }
    }
    0 // fallback
}

/// 월간 계산
pub fn month_stem(year_stem: Stem, month_index: usize) -> Stem {
    let start = MONTH_STEM_START[year_stem.index()];
    Stem::from_index(start + month_index)
}

/// 시간(0-23)에서 시지 인덱스
pub fn hour_branch_index(hour: u32) -> usize {
    match hour {
        23 | 0 => 0,
        1..=2 => 1,
        3..=4 => 2,
        5..=6 => 3,
        7..=8 => 4,
        9..=10 => 5,
        11..=12 => 6,
        13..=14 => 7,
        15..=16 => 8,
        17..=18 => 9,
        19..=20 => 10,
        21..=22 => 11,
        _ => 0,
    }
}

/// 시간 천간 계산
pub fn hour_stem(day_stem: Stem, hour_index: usize) -> Stem {
    let start = HOUR_STEM_START[day_stem.index()];
    Stem::from_index(start + hour_index)
}
