//! P0/P1/P2 self-check on agent-emitted HTML artifacts.
//!
//! Cheap pattern checks — not a full HTML parser. Catches the most common
//! AI-slop tells and invented metrics so the agent can iterate before
//! emitting. Distilled from open-design's [`daemon/lint-artifact.js`][l]
//! (Apache-2.0).
//!
//! [l]: https://github.com/nexu-io/open-design/blob/main/daemon/lint-artifact.js

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LintSeverity {
    /// Must-fix — anti-AI-slop or broken artifact.
    P0,
    /// Should-fix — quality regressions.
    P1,
    /// Nice-to-fix — style / consistency.
    P2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintFinding {
    pub severity: LintSeverity,
    pub rule: &'static str,
    pub message: String,
    /// Best-effort excerpt of the offending content for context.
    pub excerpt: Option<String>,
}

/// Lint a rendered artifact body. Returns findings in source order.
pub fn lint_artifact(html: &str) -> Vec<LintFinding> {
    let mut out = Vec::new();
    let lower = html.to_ascii_lowercase();

    // P0: aggressive purple→pink hero gradient (the canonical AI tell).
    if lower.contains("linear-gradient")
        && (lower.contains("#a855f7")
            || lower.contains("#9333ea")
            || lower.contains("#7c3aed")
            || lower.contains("rgb(168, 85, 247)"))
        && (lower.contains("#ec4899") || lower.contains("#db2777") || lower.contains("#f472b6"))
    {
        out.push(LintFinding {
            severity: LintSeverity::P0,
            rule: "anti-slop:purple-pink-gradient",
            message: "Aggressive purple→pink gradient detected. This is the canonical AI-slop hero tell — replace with a single accent color from the active design system.".into(),
            excerpt: None,
        });
    }

    // P0: img with empty / missing alt.
    for cap in find_imgs_with_alt_issue(html) {
        out.push(LintFinding {
            severity: LintSeverity::P0,
            rule: "a11y:img-missing-alt",
            message:
                "<img> missing or empty alt — accessibility regression and signals lazy content."
                    .into(),
            excerpt: Some(cap),
        });
    }

    // P1: invented metric pattern — "10× faster" / "10x faster" / "300% more".
    if let Some(m) = find_pattern(
        html,
        &["10× faster", "10x faster", "300% more", "100× faster"],
    ) {
        out.push(LintFinding {
            severity: LintSeverity::P1,
            rule: "content:invented-metric",
            message: "Looks like an invented metric. Replace with a real number, an em-dash placeholder, or a labelled grey block.".into(),
            excerpt: Some(m),
        });
    }

    // P1: Inter as the *display* face — `font-family` mentions Inter on
    // an h1/h2/.display selector.
    if lower.contains("font-family") && lower.contains("inter") {
        // Heuristic: if the same rule names h1/h2/.display/.hero, flag.
        for snippet in extract_font_family_rules(html) {
            let s = snippet.to_ascii_lowercase();
            if s.contains("inter")
                && (s.contains("h1")
                    || s.contains("h2")
                    || s.contains("display")
                    || s.contains("hero"))
            {
                out.push(LintFinding {
                    severity: LintSeverity::P1,
                    rule: "typography:inter-as-display",
                    message: "Inter used as a display face. Inter is a body sans; pick a serif or geometric sans for headlines.".into(),
                    excerpt: Some(snippet),
                });
                break;
            }
        }
    }

    // P2: rounded card with left border accent — the AI block-quote tell.
    if lower.contains("border-left")
        && (lower.contains("border-radius") || lower.contains("rounded"))
    {
        out.push(LintFinding {
            severity: LintSeverity::P2,
            rule: "anti-slop:bordered-rounded-card",
            message: "Rounded card with a left-border accent stripe is the canonical AI block-quote tell. Use one or the other, not both.".into(),
            excerpt: None,
        });
    }

    // P2: hand-drawn human SVG (very crude detection — `<svg>` containing
    // `circle` for a head + `path` for a body).
    if lower.contains("<svg") && lower.contains("circle") && lower.contains("d=\"m") {
        // Most decorative SVGs match this — only flag if it appears to be a
        // standalone illustration (large width/height).
        if lower.contains("width=\"200\"") || lower.contains("width=\"300\"") {
            out.push(LintFinding {
                severity: LintSeverity::P2,
                rule: "anti-slop:hand-drawn-human-svg",
                message: "Possible hand-drawn human SVG. Replace with a real photograph or a labelled grey block.".into(),
                excerpt: None,
            });
        }
    }

    out
}

fn find_imgs_with_alt_issue(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut search = 0usize;
    while let Some(rel) = lower[search..].find("<img") {
        let abs = search + rel;
        let end = match html[abs..].find('>') {
            Some(e) => abs + e + 1,
            None => break,
        };
        let tag = &html[abs..end];
        let lower_tag = tag.to_ascii_lowercase();
        let has_alt = lower_tag.contains("alt=");
        let empty_alt = lower_tag.contains("alt=\"\"");
        if !has_alt || empty_alt {
            out.push(tag.to_string());
        }
        search = end;
    }
    out
}

fn find_pattern(html: &str, patterns: &[&str]) -> Option<String> {
    for p in patterns {
        if let Some(idx) = html.find(p) {
            let start = idx.saturating_sub(20);
            let end = (idx + p.len() + 20).min(html.len());
            return Some(html[start..end].to_string());
        }
    }
    None
}

fn extract_font_family_rules(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut search = 0usize;
    while let Some(rel) = lower[search..].find("{") {
        let block_start = search + rel;
        // Find matching `}`.
        let block_end = match html[block_start..].find('}') {
            Some(e) => block_start + e + 1,
            None => break,
        };
        // Selector is everything from previous `}` (or start) to `{`.
        let sel_start = html[..block_start].rfind('}').map(|p| p + 1).unwrap_or(0);
        let snippet = html[sel_start..block_end].trim().to_string();
        if snippet.to_ascii_lowercase().contains("font-family") {
            out.push(snippet);
        }
        search = block_end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_purple_pink_gradient_p0() {
        let html =
            "<div style=\"background: linear-gradient(135deg, #a855f7 0%, #ec4899 100%);\"></div>";
        let findings = lint_artifact(html);
        assert!(
            findings
                .iter()
                .any(|f| f.severity == LintSeverity::P0
                    && f.rule == "anti-slop:purple-pink-gradient")
        );
    }

    #[test]
    fn flags_img_without_alt_p0() {
        let findings = lint_artifact("<img src=\"x.png\">");
        assert!(
            findings
                .iter()
                .any(|f| f.severity == LintSeverity::P0 && f.rule == "a11y:img-missing-alt")
        );
    }

    #[test]
    fn passes_img_with_real_alt() {
        let findings = lint_artifact("<img src=\"x.png\" alt=\"a real description\">");
        assert!(!findings.iter().any(|f| f.rule == "a11y:img-missing-alt"));
    }

    #[test]
    fn flags_invented_metric() {
        let findings = lint_artifact("Our system is 10× faster than the competition.");
        assert!(findings.iter().any(|f| f.rule == "content:invented-metric"));
    }

    #[test]
    fn flags_inter_as_display() {
        let css = ".hero h1 { font-family: 'Inter', sans-serif; font-size: 96px; }";
        let findings = lint_artifact(&format!("<style>{css}</style>"));
        assert!(
            findings
                .iter()
                .any(|f| f.rule == "typography:inter-as-display")
        );
    }

    #[test]
    fn no_findings_on_clean_artifact() {
        let html = "<article><h1 style=\"font-family: Newsreader, serif\">Title</h1><img src=\"hero.jpg\" alt=\"Decorative hero photograph\"></article>";
        let findings = lint_artifact(html);
        assert!(
            findings.is_empty(),
            "expected no findings, got {:?}",
            findings
        );
    }
}
