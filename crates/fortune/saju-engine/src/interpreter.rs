use super::ten_gods;
use super::types::{Element, ElementBalance, FourPillars, Stem, TenGod};

/// 일간(day master) 성격 해석
pub fn personality(day_master: Stem) -> &'static str {
    match day_master {
        Stem::Gap => {
            "갑목(甲木) 일간: 큰 나무와 같은 성격으로 곧고 바르며 진취적입니다. 리더십이 강하고 자존심이 높으며, 목표를 향해 꾸준히 성장하는 힘이 있습니다. 다만 고집이 세고 융통성이 부족할 수 있습니다."
        }
        Stem::Eul => {
            "을목(乙木) 일간: 풀이나 덩굴과 같은 성격으로 유연하고 적응력이 뛰어납니다. 부드러운 외모 속에 강한 생명력이 있으며, 인간관계가 좋고 협조적입니다. 때로 우유부단한 면이 있을 수 있습니다."
        }
        Stem::Byeong => {
            "병화(丙火) 일간: 태양과 같은 성격으로 밝고 활발하며 열정적입니다. 사교성이 좋고 표현력이 풍부하며, 주변을 밝게 만드는 에너지가 있습니다. 다만 성급하고 지속력이 부족할 수 있습니다."
        }
        Stem::Jeong => {
            "정화(丁火) 일간: 촛불과 같은 성격으로 섬세하고 집중력이 강합니다. 지적 호기심이 많고 예술적 감각이 뛰어나며, 내면의 열정이 깊습니다. 신경이 예민하고 걱정이 많을 수 있습니다."
        }
        Stem::Mu => {
            "무토(戊土) 일간: 산이나 큰 바위와 같은 성격으로 묵직하고 신뢰감이 있습니다. 포용력이 크고 중심을 잡아주는 역할을 하며, 안정적이고 책임감이 강합니다. 변화에 느리게 반응할 수 있습니다."
        }
        Stem::Gi => {
            "기토(己土) 일간: 논밭의 흙과 같은 성격으로 온화하고 수용적입니다. 만물을 품는 대지처럼 배려심이 깊고 현실적이며, 실속을 중시합니다. 소심하거나 의존적일 수 있습니다."
        }
        Stem::Gyeong => {
            "경금(庚金) 일간: 쇠붙이나 칼과 같은 성격으로 강직하고 결단력이 있습니다. 정의감이 강하고 의리를 중시하며, 행동력이 뛰어납니다. 다만 거칠고 공격적인 면이 있을 수 있습니다."
        }
        Stem::Sin => {
            "신금(辛金) 일간: 보석이나 장신구와 같은 성격으로 세련되고 심미안이 있습니다. 예리한 판단력과 섬세한 감각을 가졌으며, 자기만의 기준이 뚜렷합니다. 예민하고 까다로울 수 있습니다."
        }
        Stem::Im => {
            "임수(壬水) 일간: 큰 바다나 강물과 같은 성격으로 지혜롭고 포용력이 넓습니다. 자유로운 사고를 하며 창의력이 뛰어나고 적응력이 강합니다. 변덕스럽고 방향을 잃기 쉬울 수 있습니다."
        }
        Stem::Gye => {
            "계수(癸水) 일간: 이슬이나 빗물과 같은 성격으로 조용하고 인내심이 강합니다. 직관력이 뛰어나고 감수성이 풍부하며, 깊은 사색을 좋아합니다. 소극적이고 내성적일 수 있습니다."
        }
    }
}

/// 오행 밸런스 해석
pub fn element_balance_analysis(balance: &ElementBalance) -> String {
    let mut lines = Vec::new();

    lines.push(format!(
        "오행 분포: 목({}) 화({}) 토({}) 금({}) 수({})",
        balance.wood, balance.fire, balance.earth, balance.metal, balance.water
    ));

    let dominant = balance.dominant();
    let weakest = balance.weakest();

    lines.push(format!(
        "가장 강한 오행: {} — {}",
        dominant.korean(),
        dominant_meaning(dominant)
    ));

    lines.push(format!(
        "가장 약한 오행: {} — {}",
        weakest.korean(),
        weak_meaning(weakest)
    ));

    // 특수 패턴
    let counts = [
        balance.wood,
        balance.fire,
        balance.earth,
        balance.metal,
        balance.water,
    ];
    let zeros = counts.iter().filter(|&&c| c == 0).count();
    if zeros > 0 {
        lines.push("오행 중 빠진 기운이 있어 보완이 필요합니다.".into());
    }
    let max_val = *counts.iter().max().unwrap();
    if max_val >= 4 {
        lines.push("특정 오행이 매우 강하여 편중된 기운을 조절하는 것이 좋습니다.".into());
    }

    lines.join("\n")
}

fn dominant_meaning(elem: Element) -> &'static str {
    match elem {
        Element::Wood => "성장과 진취적 에너지가 풍부합니다",
        Element::Fire => "열정과 표현력이 넘치는 성향입니다",
        Element::Earth => "안정감과 신뢰를 주는 성향입니다",
        Element::Metal => "결단력과 의지가 강한 성향입니다",
        Element::Water => "지혜와 유연함이 뛰어난 성향입니다",
    }
}

fn weak_meaning(elem: Element) -> &'static str {
    match elem {
        Element::Wood => "계획성과 추진력을 보완하면 좋겠습니다",
        Element::Fire => "적극성과 자신감을 기르면 좋겠습니다",
        Element::Earth => "안정감과 인내심을 키우면 좋겠습니다",
        Element::Metal => "결단력과 실행력을 기르면 좋겠습니다",
        Element::Water => "유연성과 소통 능력을 키우면 좋겠습니다",
    }
}

/// 십신 기반 운세 전망
pub fn ten_gods_outlook(pillars: &FourPillars, has_birth_time: bool) -> String {
    let gods = ten_gods::analyze_ten_gods(pillars, has_birth_time);
    let mut lines = Vec::new();

    lines.push("【십신 분석】".to_string());
    for (position, god) in &gods {
        lines.push(format!("  {} : {}", position, god.korean()));
    }

    // 주요 십신 해석
    let year_god = gods[0].1;
    let month_god = gods[1].1;

    lines.push(String::new());
    lines.push(format!(
        "▶ 년주 {} — {}",
        year_god.korean(),
        ten_god_year_meaning(year_god)
    ));
    lines.push(format!(
        "▶ 월주 {} — {}",
        month_god.korean(),
        ten_god_month_meaning(month_god)
    ));

    if has_birth_time {
        let hour_god = gods[3].1;
        lines.push(format!(
            "▶ 시주 {} — {}",
            hour_god.korean(),
            ten_god_hour_meaning(hour_god)
        ));
    }

    lines.join("\n")
}

fn ten_god_year_meaning(god: TenGod) -> &'static str {
    match god {
        TenGod::Bigyeon => "초년에 형제나 친구의 도움이 있습니다",
        TenGod::Geupjae => "어린 시절 경쟁 환경에서 자랐습니다",
        TenGod::Sikshin => "풍족한 유년기를 보냈을 가능성이 높습니다",
        TenGod::Sanggwan => "자유로운 어린 시절을 보냈습니다",
        TenGod::Pyeonjae => "아버지의 영향이 크거나 재물 복이 있습니다",
        TenGod::Jeongjae => "안정된 가정환경에서 자랐습니다",
        TenGod::Pyeongwan => "엄격한 환경에서 성장했습니다",
        TenGod::Jeonggwan => "규율 있는 가정에서 자랐습니다",
        TenGod::Pyeonin => "학문적 재능이 어릴 때부터 나타납니다",
        TenGod::Jeongin => "어머니의 영향과 보살핌이 컸습니다",
    }
}

fn ten_god_month_meaning(god: TenGod) -> &'static str {
    match god {
        TenGod::Bigyeon => "청년기에 동료와 협력하는 운이 강합니다",
        TenGod::Geupjae => "사회생활에서 경쟁심이 원동력이 됩니다",
        TenGod::Sikshin => "직업에서 창의력을 발휘합니다",
        TenGod::Sanggwan => "예술적 재능이나 전문성이 뛰어납니다",
        TenGod::Pyeonjae => "사업이나 투자에 재능이 있습니다",
        TenGod::Jeongjae => "안정적인 직업에서 재물을 모읍니다",
        TenGod::Pyeongwan => "조직에서 리더십을 발휘합니다",
        TenGod::Jeonggwan => "공직이나 관리직에 적합합니다",
        TenGod::Pyeonin => "독창적인 분야에서 두각을 나타냅니다",
        TenGod::Jeongin => "학문이나 교육 분야에 인연이 깊습니다",
    }
}

fn ten_god_hour_meaning(god: TenGod) -> &'static str {
    match god {
        TenGod::Bigyeon => "말년에 형제나 동료의 도움이 있습니다",
        TenGod::Geupjae => "노후에 재물 관리에 주의가 필요합니다",
        TenGod::Sikshin => "말년에 자녀 복이 있고 편안합니다",
        TenGod::Sanggwan => "자유로운 노년을 보냅니다",
        TenGod::Pyeonjae => "말년에도 활발한 경제활동을 합니다",
        TenGod::Jeongjae => "안정적이고 풍족한 노후입니다",
        TenGod::Pyeongwan => "말년에도 사회적 활동이 활발합니다",
        TenGod::Jeonggwan => "존경받는 노년을 보냅니다",
        TenGod::Pyeonin => "말년에 학문이나 종교에 관심이 깊어집니다",
        TenGod::Jeongin => "자녀에게 좋은 가르침을 전합니다",
    }
}
