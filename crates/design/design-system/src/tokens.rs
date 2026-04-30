//! Color token extractor — finds `#rrggbb` / `#rrggbbaa` / `oklch(...)` /
//! `rgb(...)` / `rgba(...)` tokens in arbitrary text and counts occurrences
//! so a swatch grid can lift the most-used values per system.
//!
//! Deliberately tiny — no full CSS parser. Misses colors hidden inside
//! gradients-of-gradients but catches everything DESIGN.md authors actually
//! put in a "## Color" body.

use serde::{Deserialize, Serialize};

/// One observed color token from a DESIGN.md body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorToken {
    /// Verbatim representation (e.g. `#26251e`, `oklch(58% 0.16 35)`).
    pub value: String,
    /// How many times the token appears in the source.
    pub count: usize,
    /// Best-effort kind label for downstream UI grouping.
    pub kind: ColorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorKind {
    Hex,
    Oklch,
    Rgb,
    Rgba,
    Other,
}

/// Extract every color-like token from a body, count occurrences, and
/// return them sorted by count (descending). Skips obvious non-colors —
/// `#` followed by non-hex digits, bare `rgb(` without numbers.
pub fn extract_color_tokens(body: &str) -> Vec<ColorToken> {
    let mut tokens: Vec<(String, ColorKind)> = Vec::new();

    // Hex — #rrggbb or #rrggbbaa
    for cap in find_hex(body) {
        tokens.push((cap, ColorKind::Hex));
    }
    // oklch(...) — coarse: from `oklch(` to first matching `)`
    for cap in find_funcs(body, "oklch") {
        tokens.push((cap, ColorKind::Oklch));
    }
    for cap in find_funcs(body, "rgba") {
        tokens.push((cap, ColorKind::Rgba));
    }
    for cap in find_funcs(body, "rgb") {
        // Skip rgba duplicates (rgba is a superset of rgb in our func scan)
        if !cap.starts_with("rgba") {
            tokens.push((cap, ColorKind::Rgb));
        }
    }

    // Count + dedup
    let mut counts: std::collections::HashMap<(String, ColorKind), usize> =
        std::collections::HashMap::new();
    for (val, kind) in tokens {
        *counts.entry((val, kind)).or_insert(0) += 1;
    }
    let mut out: Vec<ColorToken> = counts
        .into_iter()
        .map(|((value, kind), count)| ColorToken { value, count, kind })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.value.cmp(&b.value)));
    out
}

fn find_hex(body: &str) -> Vec<String> {
    let bytes = body.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'#' {
            // collect up to 8 hex chars
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j] as char).is_ascii_hexdigit() && j - i <= 8 {
                j += 1;
            }
            let len = j - i - 1;
            if len == 6 || len == 8 || len == 3 || len == 4 {
                if let Ok(s) = std::str::from_utf8(&bytes[i..j]) {
                    out.push(s.to_string());
                }
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    out
}

fn find_funcs(body: &str, name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = body.to_lowercase();
    let needle = format!("{name}(");
    let mut search_from = 0usize;
    while let Some(rel) = lower[search_from..].find(&needle) {
        let abs = search_from + rel;
        // Find matching `)`
        let after_paren = abs + needle.len();
        if let Some(end_rel) = body[after_paren..].find(')') {
            let end_abs = after_paren + end_rel + 1;
            // Use original casing for the captured slice.
            let slice = &body[abs..end_abs];
            // Quick sanity: slice must contain at least one digit.
            if slice.bytes().any(|b| b.is_ascii_digit()) {
                out.push(slice.to_string());
            }
            search_from = end_abs;
        } else {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_hex_and_oklch() {
        let body = "Use `#26251e` for ink and `#f2f1ed` for paper. Accent: oklch(58% 0.16 35). Repeat `#f2f1ed`.";
        let toks = extract_color_tokens(body);
        let f2f = toks.iter().find(|t| t.value == "#f2f1ed").unwrap();
        assert_eq!(f2f.count, 2);
        assert_eq!(f2f.kind, ColorKind::Hex);
        assert!(toks.iter().any(|t| t.kind == ColorKind::Oklch));
    }

    #[test]
    fn skips_bogus_hex_lengths() {
        // `#abc` (3) is valid CSS short hex but we accept it; `#xyz` is not hex.
        let body = "#abc and #xyz and #aabbccdd and #abcde (5 hex — invalid)";
        let toks = extract_color_tokens(body);
        let vals: Vec<&str> = toks.iter().map(|t| t.value.as_str()).collect();
        assert!(vals.contains(&"#abc"));
        assert!(vals.contains(&"#aabbccdd"));
        assert!(!vals.iter().any(|v| v.starts_with("#xyz")));
        assert!(!vals.iter().any(|v| v.starts_with("#abcde")));
    }

    #[test]
    fn extracts_rgba_and_rgb_distinct_kinds() {
        let body = "rgba(0,0,0,0.1) and rgb(20, 30, 40)";
        let toks = extract_color_tokens(body);
        assert!(toks.iter().any(|t| t.kind == ColorKind::Rgba));
        assert!(toks.iter().any(|t| t.kind == ColorKind::Rgb));
    }

    #[test]
    fn empty_body_yields_no_tokens() {
        assert!(extract_color_tokens("nothing here").is_empty());
    }
}
