#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportParts {
    pub summary: String,
    pub advice: String,
    pub encouragement: String,
}

pub const REPORT_SYSTEM_PROMPT: &str = "\
당신은 '달결'의 AI 상담사입니다. 방금 종료된 상담 대화를 바탕으로 사용자에게 전달할 리포트를 작성합니다.

## 출력 형식 (반드시 이 순서로, 각 섹션 헤더 정확히 일치)
### 요약
(사용자가 어떤 고민을 가져왔고 상담사가 어떤 관점으로 응답했는지 3~5문장)

### 조언
(사용자에게 실질적으로 도움이 될 방향 3가지, 각각 1~2문장. 마크다운 리스트(- 항목) 사용 가능)

### 격려
(따뜻한 마무리 메시지 2~3문장. 근거 없는 낙관은 피하되 희망적인 톤)

## 제약
- 존댓말(해요체).
- 의료/법률/금융 투자 구체적 조언 금지.
- 대화에 나온 내용만 요약/재구성. 새로운 해석 추가 금지.
- 섹션 헤더(### 요약, ### 조언, ### 격려)를 반드시 사용. 다른 헤더 구조 금지.
";

pub fn parse_report_sections(text: &str) -> Option<ReportParts> {
    const SUMMARY: &[&str] = &["### 요약", "## 요약", "# 요약", "**요약**", "요약:"];
    const ADVICE: &[&str] = &["### 조언", "## 조언", "# 조언", "**조언**", "조언:"];
    const ENCOURAGEMENT: &[&str] = &[
        "### 격려",
        "## 격려",
        "# 격려",
        "**격려**",
        "격려:",
        "### 마무리",
        "## 마무리",
        "마무리:",
    ];

    let boundaries: Vec<&str> = SUMMARY
        .iter()
        .chain(ADVICE.iter())
        .chain(ENCOURAGEMENT.iter())
        .copied()
        .collect();

    let find_section = |headers: &[&str]| -> Option<String> {
        headers.iter().find_map(|header| {
            let content = extract_section(text, header, &boundaries)?;
            (!content.is_empty()).then_some(content)
        })
    };

    Some(ReportParts {
        summary: find_section(SUMMARY)?,
        advice: find_section(ADVICE)?,
        encouragement: find_section(ENCOURAGEMENT)?,
    })
}

pub fn parse_report_fallback(text: &str) -> ReportParts {
    ReportParts {
        summary: text.trim().to_string(),
        advice: String::new(),
        encouragement: String::new(),
    }
}

fn extract_section(text: &str, header: &str, boundaries: &[&str]) -> Option<String> {
    let start = text.find(header)? + header.len();
    let rest = &text[start..];
    let end = boundaries
        .iter()
        .filter_map(|candidate| rest.find(candidate))
        .min()
        .unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_report_success() {
        let text = "### 요약\n사용자는 직장 고민을 가져왔어요.\n\n### 조언\n- 강점을 돌아보세요.\n\n### 격려\n좋은 흐름이 기다리고 있어요.";
        let parts = parse_report_sections(text).expect("should parse");
        assert!(parts.summary.contains("직장 고민"));
        assert!(parts.advice.contains("강점"));
        assert!(parts.encouragement.contains("흐름"));
    }

    #[test]
    fn parse_report_missing_section_returns_none() {
        let text = "### 요약\n내용\n### 격려\n끝";
        assert!(parse_report_sections(text).is_none());
    }

    #[test]
    fn parse_report_headers_out_of_order() {
        let text = "### 격려\n힘내세요\n### 요약\n요약본\n### 조언\n조언본";
        let parts = parse_report_sections(text).expect("should parse");
        assert_eq!(parts.summary, "요약본");
        assert_eq!(parts.advice, "조언본");
        assert_eq!(parts.encouragement, "힘내세요");
    }

    #[test]
    fn fallback_uses_raw_text_as_summary() {
        let parts = parse_report_fallback("그냥 요약");
        assert_eq!(parts.summary, "그냥 요약");
        assert!(parts.advice.is_empty());
        assert!(parts.encouragement.is_empty());
    }
}
