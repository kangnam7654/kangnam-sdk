//! Brand-agnostic craft references — the universal craft rules a competent
//! designer applies on top of any [`DESIGN.md`][ds]. Where `DESIGN.md` says
//! "use *this* color/font," craft references say "use *that* letter-spacing
//! at *this* size, never break this contrast ratio, here's the canonical
//! cap on accent color usage."
//!
//! ```text
//! ┌─ skills/      Artifact shape          (saas-landing, dashboard, …)
//! ├─ systems/     Brand visual language   (linear-app, apple, notion, …)
//! └─ crafts/      ← THIS CRATE            Universal craft knowledge
//! ```
//!
//! ## Eight built-in crafts
//!
//! | Id | When to require |
//! |----|-----------------|
//! | [`TYPOGRAPHY`] | Any skill that emits typed content (≈ all) |
//! | [`COLOR`] | Any skill that emits styled output (≈ all) |
//! | [`ANTI_AI_SLOP`] | Marketing pages, landing pages, decks |
//! | [`STATE_COVERAGE`] | Dashboards, mobile apps, forms, tables |
//! | [`ANIMATION_DISCIPLINE`] | Mobile apps, multi-screen flows, microinteractions |
//! | [`ACCESSIBILITY_BASELINE`] | Anything with focus / labels / keyboard paths |
//! | [`RTL_AND_BIDI`] | Localized text or Arabic / Hebrew / Persian render paths |
//! | [`README`] | Reference: how the craft axis fits skills + systems |
//!
//! ## Skill opt-in protocol
//!
//! Open-design / kangnam-design-skill skills declare their craft requirements
//! in their SKILL.md frontmatter:
//!
//! ```yaml
//! od:
//!   craft:
//!     requires: [typography, color, anti-ai-slop]
//! ```
//!
//! Resolve those slugs into vendored craft bodies for system-prompt injection:
//!
//! ```
//! use kangnam_design_craft::{requires_to_crafts, render_for_prompt};
//!
//! let crafts = requires_to_crafts(&["typography", "anti-ai-slop"]);
//! assert_eq!(crafts.len(), 2);
//! let block = render_for_prompt(crafts);
//! assert!(block.contains("Typography craft rules"));
//! assert!(block.contains("Anti-AI-slop"));
//! ```
//!
//! Unknown slugs are silently dropped (forward-compat: a skill can list a
//! planned slug today and start benefiting once the matching craft is vendored
//! in a follow-up release, with no skill edit needed).
//!
//! ## Attribution
//!
//! Craft markdown is adapted from the MIT-licensed
//! [refero_skill](https://github.com/referodesign/refero_skill) project
//! (© Refero Design), via open-design's `craft/` directory (Apache-2.0).
//! Bodies are vendored verbatim into [`crafts/`](https://github.com/kangnam7654/kangnam-sdk/tree/main/crates/design/design-craft/crafts).
//!
//! [ds]: ../kangnam_design_system/index.html

pub mod loader;

pub use loader::{CraftRef, LoadError, OwnedCraft, list_craft_ids, load_crafts_from_dir};

use serde::{Deserialize, Serialize};

/// One vendored craft reference. `body` is the full markdown of the file
/// at `crafts/<id>.md`. Cheap to clone (all `&'static`).
#[derive(Debug, Clone, Copy)]
pub struct Craft {
    /// kebab-case id matching the skill `od.craft.requires` slug
    /// (e.g. `"typography"` → `crafts/typography.md`).
    pub id: &'static str,
    /// Human title of the section (used as the heading on prompt injection).
    pub title: &'static str,
    /// Short hint about which skill shapes should opt in.
    pub when_to_require: &'static str,
    /// Full markdown body — `include_str!("../crafts/<id>.md")`.
    pub body: &'static str,
}

impl Craft {
    /// Promote into an owned, fully-serializable record.
    pub fn to_owned(&self) -> OwnedCraft {
        OwnedCraft {
            id: self.id.into(),
            title: self.title.into(),
            when_to_require: self.when_to_require.into(),
            body: self.body.into(),
        }
    }
}

/// Universal typography rules — letter-spacing, scale, leading, etc.
pub const TYPOGRAPHY: Craft = Craft {
    id: "typography",
    title: "Typography craft rules",
    when_to_require: "Any skill that emits typed content (≈ all skills)",
    body: include_str!("../crafts/typography.md"),
};

/// Color usage discipline — accent caps, contrast, semantic tokens.
pub const COLOR: Craft = Craft {
    id: "color",
    title: "Color craft rules",
    when_to_require: "Any skill that emits styled output (≈ all skills)",
    body: include_str!("../crafts/color.md"),
};

/// Common AI-generated tells (Tailwind indigo, two-stop hero gradients,
/// emoji-as-icons) and how to avoid them. P0 list is auto-checked by
/// `lint-artifact.ts` upstream.
pub const ANTI_AI_SLOP: Craft = Craft {
    id: "anti-ai-slop",
    title: "Anti-AI-slop checklist",
    when_to_require: "Marketing pages, landing pages, decks",
    body: include_str!("../crafts/anti-ai-slop.md"),
};

/// State coverage — empty / loading / error / success / hover / focus /
/// disabled paths. Required for stateful UI.
pub const STATE_COVERAGE: Craft = Craft {
    id: "state-coverage",
    title: "State coverage",
    when_to_require: "Dashboards, mobile apps, forms, list/table views",
    body: include_str!("../crafts/state-coverage.md"),
};

/// Motion / microinteraction discipline — duration, easing, choreography.
pub const ANIMATION_DISCIPLINE: Craft = Craft {
    id: "animation-discipline",
    title: "Animation discipline",
    when_to_require: "Any skill that ships motion (mobile apps, multi-screen flows, gamified UI, microinteractions)",
    body: include_str!("../crafts/animation-discipline.md"),
};

/// Accessibility baseline — focus rings, labels, keyboard paths, contrast.
pub const ACCESSIBILITY_BASELINE: Craft = Craft {
    id: "accessibility-baseline",
    title: "Accessibility baseline",
    when_to_require: "Any skill that ships interactive UI (dashboards, forms, mobile flows)",
    body: include_str!("../crafts/accessibility-baseline.md"),
};

/// RTL + bidi text handling — Arabic, Hebrew, Persian, mixed-direction.
pub const RTL_AND_BIDI: Craft = Craft {
    id: "rtl-and-bidi",
    title: "RTL & bidi",
    when_to_require: "Any skill that ships localized text or layout (blogs, docs, mobile apps)",
    body: include_str!("../crafts/rtl-and-bidi.md"),
};

/// Cross-craft README — explains the third-axis model and skill opt-in
/// protocol. Not normally injected into system prompts; useful as
/// onboarding docs.
pub const README: Craft = Craft {
    id: "_readme",
    title: "Craft references — README",
    when_to_require: "Reference (not normally injected)",
    body: include_str!("../crafts/README.md"),
};

/// Every built-in craft, indexed by [`Craft::id`]. Stable ordering matches
/// the README's recommended-require order.
pub const BUILTIN_CRAFTS: &[&Craft] = &[
    &TYPOGRAPHY,
    &COLOR,
    &ANTI_AI_SLOP,
    &STATE_COVERAGE,
    &ANIMATION_DISCIPLINE,
    &ACCESSIBILITY_BASELINE,
    &RTL_AND_BIDI,
];

/// Every craft including [`README`]. Use this when you need to enumerate
/// all vendored markdown for a "show me everything" UX; prefer
/// [`BUILTIN_CRAFTS`] for prompt injection.
pub const ALL_CRAFTS: &[&Craft] = &[
    &TYPOGRAPHY,
    &COLOR,
    &ANTI_AI_SLOP,
    &STATE_COVERAGE,
    &ANIMATION_DISCIPLINE,
    &ACCESSIBILITY_BASELINE,
    &RTL_AND_BIDI,
    &README,
];

/// Look up a single craft by id (e.g. `"typography"`). Case-sensitive,
/// kebab-case. Returns `None` for unknown slugs (forward-compat).
pub fn craft_by_id(id: &str) -> Option<&'static Craft> {
    BUILTIN_CRAFTS.iter().copied().find(|c| c.id == id)
}

/// Resolve a list of `od.craft.requires` slugs into the matching crafts,
/// preserving input order and deduplicating. Unknown slugs are silently
/// dropped — see crate-level docs for the forward-compat rationale.
pub fn requires_to_crafts<S: AsRef<str>>(requires: &[S]) -> Vec<&'static Craft> {
    let mut out = Vec::with_capacity(requires.len());
    let mut seen = std::collections::HashSet::new();
    for slug in requires {
        let slug = slug.as_ref();
        if !seen.insert(slug.to_string()) {
            continue;
        }
        if let Some(c) = craft_by_id(slug) {
            out.push(c);
        }
    }
    out
}

/// Polymorphic adapter — implemented for [`Craft`], [`OwnedCraft`], and
/// references to either, so [`render_for_prompt`] can mix built-ins and
/// disk-loaded crafts in one call.
pub trait AsCraftRef {
    fn as_craft_ref(&self) -> CraftRef<'_>;
}

impl AsCraftRef for Craft {
    fn as_craft_ref(&self) -> CraftRef<'_> {
        CraftRef {
            id: self.id,
            title: self.title,
            when_to_require: self.when_to_require,
            body: self.body,
        }
    }
}

impl<T: AsCraftRef + ?Sized> AsCraftRef for &T {
    fn as_craft_ref(&self) -> CraftRef<'_> {
        (**self).as_craft_ref()
    }
}

/// Render a list of crafts into one prompt-injection block. Each craft is
/// emitted under a `## <title>` heading, separated by a single blank line.
/// The body is included verbatim (already markdown).
///
/// Accepts anything iterable into something that implements [`AsCraftRef`]
/// — built-in [`Craft`] refs, borrowed [`OwnedCraft`]s loaded from disk,
/// or a mix of both.
pub fn render_for_prompt<I, T>(crafts: I) -> String
where
    I: IntoIterator<Item = T>,
    T: AsCraftRef,
{
    let mut out = String::new();
    for (i, item) in crafts.into_iter().enumerate() {
        let c = item.as_craft_ref();
        if i > 0 {
            out.push('\n');
        }
        out.push_str("## ");
        out.push_str(c.title);
        out.push_str("\n\n");
        out.push_str(c.body.trim());
        out.push('\n');
    }
    out
}

/// Owned counterpart to [`Craft`] — produced by [`Craft::to_owned`] or by
/// loading user-supplied craft files via [`load_crafts_from_dir`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftRecord {
    pub id: String,
    pub title: String,
    pub when_to_require: String,
    pub body: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_builtin_has_nonempty_body() {
        for c in BUILTIN_CRAFTS {
            assert!(!c.id.is_empty(), "id must be non-empty");
            assert!(!c.body.trim().is_empty(), "{} body must be non-empty", c.id);
            assert!(!c.title.is_empty(), "{} title must be non-empty", c.id);
        }
    }

    #[test]
    fn ids_are_unique_and_kebab_case() {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        for c in ALL_CRAFTS {
            assert!(seen.insert(c.id), "duplicate id: {}", c.id);
            assert!(
                c.id.chars()
                    .all(|c| c.is_ascii_lowercase() || c == '-' || c == '_'),
                "non-kebab id: {}",
                c.id
            );
        }
    }

    #[test]
    fn craft_by_id_known_and_unknown() {
        assert_eq!(craft_by_id("typography").unwrap().id, "typography");
        assert_eq!(craft_by_id("color").unwrap().id, "color");
        assert!(craft_by_id("does-not-exist").is_none());
        // README is in ALL_CRAFTS but not in BUILTIN_CRAFTS — `craft_by_id`
        // looks at BUILTIN only, so the underscore-prefixed README is hidden
        // from the skill-opt-in API surface.
        assert!(craft_by_id("_readme").is_none());
    }

    #[test]
    fn requires_to_crafts_dedupes_and_preserves_order() {
        let req = vec!["color", "typography", "color", "unknown-slug"];
        let crafts = requires_to_crafts(&req);
        assert_eq!(crafts.len(), 2);
        assert_eq!(crafts[0].id, "color");
        assert_eq!(crafts[1].id, "typography");
    }

    #[test]
    fn requires_to_crafts_unknown_silently_drops() {
        let crafts = requires_to_crafts(&["totally-fictional"]);
        assert!(crafts.is_empty());
    }

    #[test]
    fn render_for_prompt_concatenates_with_headings() {
        let crafts = requires_to_crafts(&["typography", "color"]);
        let out = render_for_prompt(crafts);
        assert!(out.contains("## Typography craft rules"));
        assert!(out.contains("## Color craft rules"));
        // Each section starts with `## ` not `# ` so we can stack them under
        // a parent `# Craft references` heading downstream.
        assert!(!out.starts_with("# "));
    }

    #[test]
    fn render_for_prompt_empty_yields_empty_string() {
        let crafts: Vec<&Craft> = vec![];
        let out = render_for_prompt(crafts);
        assert!(out.is_empty());
    }

    #[test]
    fn typography_body_contains_letter_spacing_rule() {
        // Spot-check that the vendored body is the real refero/open-design content.
        assert!(TYPOGRAPHY.body.contains("Letter-spacing"));
        assert!(TYPOGRAPHY.body.contains("ALL CAPS") || TYPOGRAPHY.body.contains("0.06em"));
    }
}
