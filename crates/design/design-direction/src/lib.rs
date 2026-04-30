//! Built-in design-direction library — 5 curated visual schools with
//! deterministic OKLch palettes, font stacks, and posture cues.
//!
//! Distilled from open-design's [`src/prompts/directions.ts`][od] (Apache-2.0)
//! — when the user has no brand spec and selects "Pick a direction for me"
//! in the discovery form, the agent emits a `<question-form id="direction">`
//! whose options are the [`BUILTIN_DIRECTIONS`] entries below. Picking one
//! gives the agent a fully specified visual system: palette, font stacks,
//! posture cues, and real-world references — no improvisation.
//!
//! [od]: https://github.com/nexu-io/open-design/blob/main/src/prompts/directions.ts

use serde::{Deserialize, Serialize};

/// Six-token OKLch palette — bind directly into a seed template's `:root`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Palette {
    pub bg: String,
    pub surface: String,
    pub fg: String,
    pub muted: String,
    pub border: String,
    pub accent: String,
}

/// One curated visual direction. Pure data — no rendering.
///
/// Built-ins ([`BUILTIN_DIRECTIONS`]) are `&'static str` for zero-alloc
/// static storage; serialization goes through [`Direction::to_owned`] →
/// [`OwnedDirection`].
#[derive(Debug, Clone, Copy)]
pub struct Direction {
    /// kebab-case id, also the form-option label after `: `.
    pub id: &'static str,
    /// Short user-facing label, shown in the direction radio.
    pub label: &'static str,
    /// One-paragraph mood description.
    pub mood: &'static str,
    /// References / exemplars — real magazines, products, designers.
    pub references: &'static [&'static str],
    /// Headline (display) font stack. CSS-ready.
    pub display_font: &'static str,
    /// Body font stack. CSS-ready.
    pub body_font: &'static str,
    /// Optional mono override; falls back to `ui-monospace`.
    pub mono_font: Option<&'static str>,
    /// Six palette values in OKLch — bind directly into seed `:root`.
    pub palette: PaletteStatic,
    /// Layout posture cues for the agent. Concrete, not vague.
    pub posture: &'static [&'static str],
}

impl Direction {
    /// Promote this static record into an owned, fully serializable form
    /// (useful when emitting it through serde — e.g. into a question-form
    /// JSON payload or a system-prompt block).
    pub fn to_owned(&self) -> OwnedDirection {
        OwnedDirection {
            id: self.id.into(),
            label: self.label.into(),
            mood: self.mood.into(),
            references: self.references.iter().map(|s| (*s).into()).collect(),
            display_font: self.display_font.into(),
            body_font: self.body_font.into(),
            mono_font: self.mono_font.map(Into::into),
            palette: self.palette.to_palette(),
            posture: self.posture.iter().map(|s| (*s).into()).collect(),
        }
    }
}

/// Owned, serializable counterpart to [`Direction`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedDirection {
    pub id: String,
    pub label: String,
    pub mood: String,
    pub references: Vec<String>,
    #[serde(rename = "displayFont")]
    pub display_font: String,
    #[serde(rename = "bodyFont")]
    pub body_font: String,
    #[serde(rename = "monoFont", skip_serializing_if = "Option::is_none")]
    pub mono_font: Option<String>,
    pub palette: Palette,
    pub posture: Vec<String>,
}

/// `&'static str` mirror of [`Palette`] — used by the `BUILTIN_DIRECTIONS`
/// const so the whole table compiles to read-only data without allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteStatic {
    pub bg: &'static str,
    pub surface: &'static str,
    pub fg: &'static str,
    pub muted: &'static str,
    pub border: &'static str,
    pub accent: &'static str,
}

impl PaletteStatic {
    /// Promote to an owned [`Palette`] for serialization or downstream use.
    pub fn to_palette(&self) -> Palette {
        Palette {
            bg: self.bg.into(),
            surface: self.surface.into(),
            fg: self.fg.into(),
            muted: self.muted.into(),
            border: self.border.into(),
            accent: self.accent.into(),
        }
    }
}

pub mod builtin;

pub use builtin::BUILTIN_DIRECTIONS;

/// Look up a direction by its id. Returns `None` if no built-in matches.
pub fn find_by_id(id: &str) -> Option<&'static Direction> {
    BUILTIN_DIRECTIONS.iter().find(|d| d.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_builtins_present() {
        assert_eq!(BUILTIN_DIRECTIONS.len(), 5);
        for id in [
            "editorial-monocle",
            "modern-minimal",
            "warm-soft",
            "tech-utility",
            "brutalist-experimental",
        ] {
            assert!(
                find_by_id(id).is_some(),
                "expected built-in `{id}` to be registered"
            );
        }
    }

    #[test]
    fn find_by_id_unknown_returns_none() {
        assert!(find_by_id("not-a-real-direction").is_none());
    }

    #[test]
    fn each_palette_has_six_oklch_tokens() {
        for d in BUILTIN_DIRECTIONS.iter() {
            let p = &d.palette;
            for (label, val) in [
                ("bg", p.bg),
                ("surface", p.surface),
                ("fg", p.fg),
                ("muted", p.muted),
                ("border", p.border),
                ("accent", p.accent),
            ] {
                assert!(
                    val.contains("oklch(") || val.starts_with("oklch") || val == "oklch(100% 0 0)",
                    "direction `{}` palette[{label}] = {val:?} — expected oklch(...)",
                    d.id,
                );
            }
        }
    }

    #[test]
    fn palette_static_promotes_to_owned() {
        let d = find_by_id("editorial-monocle").unwrap();
        let owned = d.palette.to_palette();
        assert_eq!(owned.bg, d.palette.bg);
    }
}
