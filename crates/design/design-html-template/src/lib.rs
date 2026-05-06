//! Vendored HTML scaffold templates for deck-mode skills.
//!
//! When a deck-mode skill (e.g. `simple-deck`, `replit-deck`,
//! `html-ppt-pitch-deck`) doesn't ship its own seed template, the daemon
//! injects one of these scaffolds: a `<deck-stage>` with the
//! viewport-centered transform, deck chrome (counter / prev / next),
//! print stylesheet, and `:root` token slots ready for the active
//! design system to bind into.
//!
//! Adapted verbatim from open-design's `templates/` directory
//! (Apache-2.0). Both files use the "edit only inside the second
//! `<style>` block" convention so the framework rules stay
//! tamper-evident.
//!
//! ## Modules
//!
//! - [`DECK_FRAMEWORK`] — minimal deck baseline (nav, counter, scroll
//!   snap, print). Applied when the skill is `mode: deck` and ships no
//!   seed `assets/template.html`.
//! - [`KAMI_DECK`] — opinionated kami-deck variant (richer chrome,
//!   keyboard nav, fullscreen toggle).
//!
//! ## Usage
//!
//! ```
//! use kangnam_design_html_template::{HtmlTemplate, DECK_FRAMEWORK};
//!
//! assert_eq!(DECK_FRAMEWORK.id, "deck-framework");
//! assert!(DECK_FRAMEWORK.body.contains("<!doctype html>"));
//! assert!(DECK_FRAMEWORK.body.contains("--accent"));
//!
//! let by_id: Option<&'static HtmlTemplate> =
//!     kangnam_design_html_template::template_by_id("kami-deck");
//! assert!(by_id.is_some());
//! ```

/// One vendored HTML scaffold. `body` is the full file from
/// `templates/<id>.html` — typically 200–600 lines of HTML/CSS/JS.
#[derive(Debug, Clone, Copy)]
pub struct HtmlTemplate {
    /// kebab-case id, equals the file stem (`deck-framework`,
    /// `kami-deck`).
    pub id: &'static str,
    /// Short human title for UX pickers.
    pub title: &'static str,
    /// One-paragraph "when to use this scaffold" hint.
    pub when_to_use: &'static str,
    /// Full HTML body — `include_str!("../templates/<id>.html")`.
    pub body: &'static str,
}

/// Minimal deck baseline. Bulletproof viewport-center transform, deck
/// chrome (counter / prev / next), print stylesheet. Edit only inside
/// the second `<style>` block + `<section class="slide">` bodies.
pub const DECK_FRAMEWORK: HtmlTemplate = HtmlTemplate {
    id: "deck-framework",
    title: "Deck framework",
    when_to_use: "Default scaffold for `mode: deck` skills that don't ship their own seed `assets/template.html`. Ships viewport-centered transform, counter chrome, prev/next nav, and print rules.",
    body: include_str!("../templates/deck-framework.html"),
};

/// Opinionated kami-deck variant — richer chrome (keyboard nav,
/// fullscreen toggle, presenter notes affordances). Used by the
/// `kami-deck` and `kami-landing` skill family.
pub const KAMI_DECK: HtmlTemplate = HtmlTemplate {
    id: "kami-deck",
    title: "Kami deck",
    when_to_use: "Scaffold for the `kami-deck` / `kami-landing` skill family — adds keyboard nav, fullscreen toggle, and presenter affordances on top of the deck-framework baseline.",
    body: include_str!("../templates/kami-deck.html"),
};

/// Every vendored HTML template, ordered the way they appear in
/// open-design upstream.
pub const TEMPLATES: &[&HtmlTemplate] = &[&DECK_FRAMEWORK, &KAMI_DECK];

/// Look up a single template by id (e.g. `"deck-framework"`).
/// Case-sensitive, kebab-case. Returns `None` for unknown slugs.
pub fn template_by_id(id: &str) -> Option<&'static HtmlTemplate> {
    TEMPLATES.iter().copied().find(|t| t.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_template_has_doctype_and_root_tokens() {
        for t in TEMPLATES {
            assert!(t.body.starts_with("<!doctype html>"), "{} missing doctype", t.id);
            assert!(t.body.contains("--accent"), "{} missing accent slot", t.id);
            assert!(!t.body.is_empty(), "{} body is empty", t.id);
        }
    }

    #[test]
    fn ids_are_unique_and_kebab_case() {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        for t in TEMPLATES {
            assert!(seen.insert(t.id), "duplicate id: {}", t.id);
            assert!(
                t.id.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "non-kebab id: {}",
                t.id
            );
        }
    }

    #[test]
    fn template_by_id_known_and_unknown() {
        assert_eq!(template_by_id("deck-framework").unwrap().id, "deck-framework");
        assert_eq!(template_by_id("kami-deck").unwrap().id, "kami-deck");
        assert!(template_by_id("not-a-template").is_none());
    }

    #[test]
    fn deck_framework_has_chrome_elements() {
        assert!(DECK_FRAMEWORK.body.contains("deck-counter"));
        assert!(DECK_FRAMEWORK.body.contains("deck-stage"));
        assert!(DECK_FRAMEWORK.body.contains("class=\"slide active\"") || DECK_FRAMEWORK.body.contains("slide.active"));
    }

    #[test]
    fn templates_have_titles_and_when_to_use() {
        for t in TEMPLATES {
            assert!(!t.title.is_empty(), "{} missing title", t.id);
            assert!(!t.when_to_use.is_empty(), "{} missing when_to_use", t.id);
        }
    }
}
