use super::types::{Element, FourPillars, Stem, TenGod};

/// 일간 기준으로 다른 천간의 십신 도출
pub fn derive_ten_god(day_master: Stem, target: Stem) -> TenGod {
    let dm_elem = day_master.element();
    let tg_elem = target.element();
    let same_polarity = day_master.polarity() == target.polarity();

    match (relation_type(dm_elem, tg_elem), same_polarity) {
        (RelType::Same, true) => TenGod::Bigyeon,
        (RelType::Same, false) => TenGod::Geupjae,
        (RelType::IGenerate, true) => TenGod::Sikshin,
        (RelType::IGenerate, false) => TenGod::Sanggwan,
        (RelType::IControl, true) => TenGod::Pyeonjae,
        (RelType::IControl, false) => TenGod::Jeongjae,
        (RelType::ControlsMe, true) => TenGod::Pyeongwan,
        (RelType::ControlsMe, false) => TenGod::Jeonggwan,
        (RelType::GeneratesMe, true) => TenGod::Pyeonin,
        (RelType::GeneratesMe, false) => TenGod::Jeongin,
    }
}

#[derive(Debug)]
enum RelType {
    Same,
    IGenerate,
    IControl,
    ControlsMe,
    GeneratesMe,
}

fn relation_type(me: Element, other: Element) -> RelType {
    use Element::*;
    if me == other {
        return RelType::Same;
    }
    // 내가 생하는 것
    let i_generate = match me {
        Wood => Fire,
        Fire => Earth,
        Earth => Metal,
        Metal => Water,
        Water => Wood,
    };
    if other == i_generate {
        return RelType::IGenerate;
    }

    // 내가 극하는 것
    let i_control = match me {
        Wood => Earth,
        Fire => Metal,
        Earth => Water,
        Metal => Wood,
        Water => Fire,
    };
    if other == i_control {
        return RelType::IControl;
    }

    // 나를 극하는 것
    let controls_me = match me {
        Wood => Metal,
        Fire => Water,
        Earth => Wood,
        Metal => Fire,
        Water => Earth,
    };
    if other == controls_me {
        return RelType::ControlsMe;
    }

    // 나를 생하는 것
    RelType::GeneratesMe
}

/// 사주팔자에서 모든 천간의 십신 분석
pub fn analyze_ten_gods(
    pillars: &FourPillars,
    has_birth_time: bool,
) -> Vec<(&'static str, TenGod)> {
    let dm = pillars.day.stem;
    let mut result = vec![
        ("년간", derive_ten_god(dm, pillars.year.stem)),
        ("월간", derive_ten_god(dm, pillars.month.stem)),
        ("일간", TenGod::Bigyeon), // 일간은 자기 자신 = 비견
    ];
    if has_birth_time {
        result.push(("시간", derive_ten_god(dm, pillars.hour.stem)));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Stem;

    #[test]
    fn test_same_stem() {
        assert_eq!(derive_ten_god(Stem::Gap, Stem::Gap), TenGod::Bigyeon);
    }

    #[test]
    fn test_gap_to_eul() {
        // 갑 → 을: 같은 목, 다른 음양 = 겁재
        assert_eq!(derive_ten_god(Stem::Gap, Stem::Eul), TenGod::Geupjae);
    }

    #[test]
    fn test_gap_to_byeong() {
        // 갑(목) → 병(화): 내가 생, 같은 양 = 식신
        assert_eq!(derive_ten_god(Stem::Gap, Stem::Byeong), TenGod::Sikshin);
    }

    #[test]
    fn test_gap_to_gyeong() {
        // 갑(목) → 경(금): 나를 극, 같은 양 = 편관
        assert_eq!(derive_ten_god(Stem::Gap, Stem::Gyeong), TenGod::Pyeongwan);
    }

    #[test]
    fn test_gap_to_im() {
        // 갑(목) → 임(수): 나를 생, 같은 양 = 편인
        assert_eq!(derive_ten_god(Stem::Gap, Stem::Im), TenGod::Pyeonin);
    }
}
