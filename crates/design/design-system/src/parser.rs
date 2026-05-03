//! Lightweight DESIGN.md parser. Splits the document at top-level
//! `## <section>` headings and exposes the 9 canonical sections.
//!
//! We deliberately avoid a full markdown AST parser — the schema is just
//! "split at H2", and downstream consumers want the raw markdown text per
//! section anyway (for prompt injection or rendering). Robust to extra
//! sections and missing optional ones.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The 9 canonical sections of a `DESIGN.md` (per awesome-design-md).
/// Each is the raw markdown body under that heading; missing sections are
/// `None`. Extra sections fall into `extras`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NineSections {
    /// First-line `# <Title>` if present.
    pub title: Option<String>,
    /// Optional preamble before the first `##` section.
    pub preamble: Option<String>,
    pub color: Option<String>,
    pub typography: Option<String>,
    pub spacing: Option<String>,
    pub layout: Option<String>,
    pub components: Option<String>,
    pub motion: Option<String>,
    pub voice: Option<String>,
    pub brand: Option<String>,
    pub anti_patterns: Option<String>,
    /// Any `##` section whose heading didn't match a canonical name.
    pub extras: Vec<NamedSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedSection {
    pub heading: String,
    pub body: String,
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("DESIGN.md is empty")]
    Empty,
}

/// Parse a `DESIGN.md` body string into the 9-section structure. Returns
/// [`ParseError::Empty`] only on a totally blank input — even a partial
/// DESIGN.md (missing sections) is accepted.
pub fn parse_design_md(input: &str) -> Result<NineSections, ParseError> {
    if input.trim().is_empty() {
        return Err(ParseError::Empty);
    }

    let mut out = NineSections::default();
    let mut current_heading: Option<String> = None;
    let mut current_body = String::new();
    let mut preamble = String::new();
    let mut saw_first_heading = false;

    for line in input.lines() {
        // Title (first H1 only)
        if !saw_first_heading && out.title.is_none() {
            if let Some(t) = line.strip_prefix("# ") {
                out.title = Some(t.trim().to_string());
                continue;
            }
        }
        // H2 — section boundary. Emit the previous one.
        if let Some(rest) = line.strip_prefix("## ") {
            saw_first_heading = true;
            // flush the previous section
            if let Some(h) = current_heading.take() {
                place_section(&mut out, &h, current_body.trim().to_string());
                current_body.clear();
            } else if !preamble.is_empty() {
                out.preamble = Some(preamble.trim().to_string());
            }
            current_heading = Some(rest.trim().to_string());
            continue;
        }
        if current_heading.is_some() {
            current_body.push_str(line);
            current_body.push('\n');
        } else {
            preamble.push_str(line);
            preamble.push('\n');
        }
    }
    // flush the last section
    if let Some(h) = current_heading.take() {
        place_section(&mut out, &h, current_body.trim().to_string());
    } else if out.preamble.is_none() && !preamble.trim().is_empty() {
        out.preamble = Some(preamble.trim().to_string());
    }

    Ok(out)
}

fn canonical_key(heading: &str) -> Option<&'static str> {
    // Headings often look like "1. Color Palette & Roles", "Typography", etc.
    // Lower-case + strip leading numbering, then prefix-match.
    let trimmed = heading
        .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ' ')
        .to_lowercase();
    if trimmed.contains("color") || trimmed.contains("palette") {
        Some("color")
    } else if trimmed.contains("typograph") || trimmed.contains("font") {
        Some("typography")
    } else if trimmed.contains("spacing") || trimmed.contains("rhythm") {
        Some("spacing")
    } else if trimmed.contains("layout") || trimmed.contains("grid") {
        Some("layout")
    } else if trimmed.contains("component") {
        Some("components")
    } else if trimmed.contains("motion") || trimmed.contains("animation") {
        Some("motion")
    } else if trimmed.contains("voice") || trimmed.contains("tone") {
        Some("voice")
    } else if trimmed.contains("brand") || trimmed.contains("identity") {
        Some("brand")
    } else if trimmed.contains("anti") || trimmed.contains("avoid") {
        Some("anti_patterns")
    } else {
        None
    }
}

fn place_section(out: &mut NineSections, heading: &str, body: String) {
    match canonical_key(heading) {
        Some("color") => out.color = Some(body),
        Some("typography") => out.typography = Some(body),
        Some("spacing") => out.spacing = Some(body),
        Some("layout") => out.layout = Some(body),
        Some("components") => out.components = Some(body),
        Some("motion") => out.motion = Some(body),
        Some("voice") => out.voice = Some(body),
        Some("brand") => out.brand = Some(body),
        Some("anti_patterns") => out.anti_patterns = Some(body),
        _ => out.extras.push(NamedSection {
            heading: heading.to_string(),
            body,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_errors() {
        assert!(matches!(parse_design_md("   "), Err(ParseError::Empty)));
    }

    #[test]
    fn parses_title_and_canonical_sections() {
        let md = r#"# Cursor Design System

> Editor that thinks in code.

## 1. Color Palette & Roles

Warm off-white `#f2f1ed`, dark `#26251e`.

## 2. Typography

CursorGothic for display, jjannon for body.

## 3. Layout

8px base spacing.
"#;
        let parsed = parse_design_md(md).unwrap();
        assert_eq!(parsed.title.as_deref(), Some("Cursor Design System"));
        assert!(parsed.preamble.unwrap().contains("Editor that thinks"));
        assert!(parsed.color.unwrap().contains("#f2f1ed"));
        assert!(parsed.typography.unwrap().contains("CursorGothic"));
        assert!(parsed.layout.unwrap().contains("8px"));
        assert!(parsed.spacing.is_none());
    }

    #[test]
    fn unknown_sections_go_into_extras() {
        let md = "# X\n\n## Sound Design\n\nbeep.\n";
        let parsed = parse_design_md(md).unwrap();
        assert_eq!(parsed.extras.len(), 1);
        assert_eq!(parsed.extras[0].heading, "Sound Design");
        assert_eq!(parsed.extras[0].body, "beep.");
    }

    #[test]
    fn covers_every_canonical_alias() {
        // One DESIGN.md that exercises every alias branch in `canonical_key`
        // — `palette`, `font`, `rhythm`, `grid`, `components`, `animation`,
        // `tone`, `identity`, `avoid` — none of which were previously covered.
        let md = "# X\n\
            ## Palette\nc\n\
            ## Font Stack\nt\n\
            ## Spacing Rhythm\nsp\n\
            ## Grid\ngr\n\
            ## Components\nco\n\
            ## Animation\nm\n\
            ## Tone of Voice\nv\n\
            ## Brand Identity\nbr\n\
            ## Things to Avoid\nap\n";
        let p = parse_design_md(md).unwrap();
        assert_eq!(p.color.as_deref(), Some("c"));
        assert_eq!(p.typography.as_deref(), Some("t"));
        assert_eq!(p.spacing.as_deref(), Some("sp"));
        assert_eq!(p.layout.as_deref(), Some("gr"));
        assert_eq!(p.components.as_deref(), Some("co"));
        assert_eq!(p.motion.as_deref(), Some("m"));
        assert_eq!(p.voice.as_deref(), Some("v"));
        assert_eq!(p.brand.as_deref(), Some("br"));
        assert_eq!(p.anti_patterns.as_deref(), Some("ap"));
        assert!(p.extras.is_empty());
    }

    #[test]
    fn numbered_headings_canonicalize() {
        // `## 3. Spacing & Rhythm` should still land in `spacing`.
        let md = "# X\n## 3. Spacing & Rhythm\n8px base.\n";
        let p = parse_design_md(md).unwrap();
        assert_eq!(p.spacing.as_deref(), Some("8px base."));
    }
}
