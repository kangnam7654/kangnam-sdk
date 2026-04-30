//! System-prompt composer for the design family.
//!
//! Stacks 4 prompt layers + active skill body + active DESIGN.md + project
//! metadata into a single system-prompt string ready for `kangnam-router`.
//! Modeled after open-design's [`composeSystemPrompt`][cs] (Apache-2.0):
//!
//! ```text
//! [DISCOVERY directives]                  ← ./prompts/discovery.md (Rule 1/2/3)
//!   ↓ overrides …
//! [Identity charter]                      ← ./prompts/identity.md
//!   ↓ then …
//! [Active design system DESIGN.md]        ← caller-supplied
//!   ↓ then …
//! [Active skill SKILL.md body]            ← caller-supplied (+ pre-flight if seed)
//!   ↓ then …
//! [Project metadata block]                ← kind / fidelity / animations / …
//!   ↓ pinned LAST when deck …
//! [Deck framework directive]              ← ./prompts/deck-framework.md
//! [Brand asset protocol]                  ← ./prompts/brand-asset-protocol.md (when brand named)
//! ```
//!
//! Brand-asset-protocol stitches in any time the metadata's `brand_named` is
//! true. The deck-framework block pins **last** when the project is a deck
//! AND the active skill doesn't already ship a seed template (we defer to
//! skill seeds when present).
//!
//! [cs]: https://github.com/nexu-io/open-design/blob/main/src/prompts/system.ts

pub mod metadata;

pub use metadata::{ProjectKind, ProjectMetadata, ProjectTemplate, SkillMode};

/// The OD-adapted "expert designer" identity charter — workflow,
/// content philosophy, anti-AI-slop discipline. Ported from
/// open-design's `OFFICIAL_DESIGNER_PROMPT` (Apache-2.0).
pub const IDENTITY_PROMPT: &str = include_str!("../prompts/identity.md");

/// Three hard rules + 5-dim self-critique gate. Ported from open-design's
/// `DISCOVERY_AND_PHILOSOPHY` (Apache-2.0). Stacks BEFORE the identity
/// charter so its rules win precedence over softer "skip questions for
/// small tweaks" wording in the base prompt.
pub const DISCOVERY_PROMPT: &str = include_str!("../prompts/discovery.md");

/// Deck framework — load-bearing nav / counter / scroll-snap / print
/// scaffolding. Pinned LAST in deck mode so it overrides any softer
/// slide-handling wording earlier in the stack.
pub const DECK_FRAMEWORK_PROMPT: &str = include_str!("../prompts/deck-framework.md");

/// 5-step brand asset protocol — locate · download · grep hex · codify
/// `brand-spec.md` · vocalise.
pub const BRAND_ASSET_PROMPT: &str = include_str!("../prompts/brand-asset-protocol.md");

/// Composer input. All fields except metadata are optional — composing with
/// no skill / DS still produces a usable system prompt, just less specific.
#[derive(Debug, Clone, Default)]
pub struct ComposeInput {
    /// Active skill body (markdown, post-frontmatter). Empty when no skill.
    pub skill_body: Option<String>,
    /// Active skill name, used in the section header.
    pub skill_name: Option<String>,
    /// Active skill's mode — drives the deck-framework decision.
    pub skill_mode: Option<SkillMode>,
    /// Active design system body (DESIGN.md markdown).
    pub design_system_body: Option<String>,
    /// Active design system display title (e.g. "Linear").
    pub design_system_title: Option<String>,
    /// Project metadata captured at create time (drives behaviour).
    pub metadata: Option<ProjectMetadata>,
    /// Optional starter template snapshot (raw HTML files).
    pub template: Option<ProjectTemplate>,
}

/// Compose the full system prompt. Output ranges roughly 30–120 KB depending
/// on skill body + DESIGN.md size.
pub fn compose_system_prompt(input: &ComposeInput) -> String {
    let mut parts = Vec::<String>::new();

    // 1. DISCOVERY first (highest precedence).
    parts.push(DISCOVERY_PROMPT.to_string());

    // 2. Identity charter.
    parts.push("\n\n---\n\n# Identity and workflow charter (background)\n\n".into());
    parts.push(IDENTITY_PROMPT.to_string());

    // 3. Active design system, if any.
    if let Some(body) = input.design_system_body.as_deref() {
        let trimmed = body.trim();
        if !trimmed.is_empty() {
            let title = input.design_system_title.as_deref().unwrap_or("");
            let header = if title.is_empty() {
                "\n\n## Active design system\n\n".to_string()
            } else {
                format!("\n\n## Active design system — {title}\n\n")
            };
            parts.push(header);
            parts.push(
                "Treat the following DESIGN.md as authoritative for color, \
                 typography, spacing, and component rules. Do not invent tokens \
                 outside this palette. When you copy the active skill's seed \
                 template, bind these tokens into its `:root` block before \
                 generating any layout.\n\n"
                    .into(),
            );
            parts.push(trimmed.to_string());
        }
    }

    // 4. Active skill body.
    if let Some(body) = input.skill_body.as_deref() {
        let trimmed = body.trim();
        if !trimmed.is_empty() {
            let name = input.skill_name.as_deref().unwrap_or("");
            let header = if name.is_empty() {
                "\n\n## Active skill\n\n".to_string()
            } else {
                format!("\n\n## Active skill — {name}\n\n")
            };
            parts.push(header);
            parts.push("Follow this skill's workflow exactly.".into());
            // Pre-flight rule: if the skill body references its assets/refs,
            // tell the agent to read them before writing any layout.
            if body.contains("assets/template.html")
                || body.contains("references/layouts.md")
                || body.contains("references/checklist.md")
            {
                parts.push(
                    "\n\n**Pre-flight (before any layout):** read the skill's \
                     `assets/template.html` and `references/*.md` files end-to-end. \
                     Use the seed template + layout library — do not write CSS \
                     from scratch.\n\n"
                        .into(),
                );
            } else {
                parts.push("\n\n".into());
            }
            parts.push(trimmed.to_string());
        }
    }

    // 5. Project metadata block.
    if let Some(meta) = input.metadata.as_ref() {
        if let Some(block) = meta.render_block(input.template.as_ref()) {
            parts.push("\n\n".into());
            parts.push(block);
        }
    }

    // 6. Brand asset protocol (when relevant).
    if let Some(meta) = input.metadata.as_ref() {
        if meta.brand_named {
            parts.push("\n\n---\n\n".into());
            parts.push(BRAND_ASSET_PROMPT.to_string());
        }
    }

    // 7. Deck framework — pinned LAST in deck mode IF no skill seed.
    let is_deck = input.skill_mode == Some(SkillMode::Deck)
        || input
            .metadata
            .as_ref()
            .map(|m| m.kind == ProjectKind::Deck)
            .unwrap_or(false);
    let has_skill_seed = input
        .skill_body
        .as_deref()
        .map(|b| b.contains("assets/template.html"))
        .unwrap_or(false);
    if is_deck && !has_skill_seed {
        parts.push("\n\n---\n\n".into());
        parts.push(DECK_FRAMEWORK_PROMPT.to_string());
    }

    parts.concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_yields_discovery_plus_identity() {
        let out = compose_system_prompt(&ComposeInput::default());
        assert!(out.contains("RULE 1"));
        assert!(out.contains("Identity and workflow charter"));
        assert!(!out.contains("Active design system"));
        assert!(!out.contains("Active skill"));
        assert!(!out.contains("Brand asset protocol"));
    }

    #[test]
    fn includes_design_system_when_provided() {
        let input = ComposeInput {
            design_system_body: Some("# Linear\n\n## Color\n\nBlue.".into()),
            design_system_title: Some("Linear".into()),
            ..Default::default()
        };
        let out = compose_system_prompt(&input);
        assert!(out.contains("Active design system — Linear"));
        assert!(out.contains("# Linear"));
    }

    #[test]
    fn deck_mode_pins_framework_last_when_no_seed() {
        let input = ComposeInput {
            skill_body: Some("# A skill that doesn't ship a seed.".into()),
            skill_name: Some("simple-deck".into()),
            skill_mode: Some(SkillMode::Deck),
            ..Default::default()
        };
        let out = compose_system_prompt(&input);
        let framework_pos = out.find("Deck framework directive").unwrap();
        let identity_pos = out.find("Identity and workflow charter").unwrap();
        let skill_pos = out.find("Active skill").unwrap();
        assert!(framework_pos > identity_pos);
        assert!(framework_pos > skill_pos, "deck framework must come after the skill");
    }

    #[test]
    fn deck_mode_skips_framework_when_skill_has_seed() {
        let input = ComposeInput {
            skill_body: Some("Read assets/template.html first.\n# guizang".into()),
            skill_name: Some("guizang-ppt".into()),
            skill_mode: Some(SkillMode::Deck),
            ..Default::default()
        };
        let out = compose_system_prompt(&input);
        assert!(!out.contains("Deck framework directive"));
    }

    #[test]
    fn brand_named_includes_asset_protocol() {
        let input = ComposeInput {
            metadata: Some(ProjectMetadata {
                kind: ProjectKind::Prototype,
                brand_named: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        let out = compose_system_prompt(&input);
        assert!(out.contains("Brand asset protocol"));
    }

    #[test]
    fn pre_flight_inserted_when_skill_has_assets() {
        let input = ComposeInput {
            skill_body: Some(
                "Use assets/template.html, then references/layouts.md.".into(),
            ),
            skill_name: Some("web-prototype".into()),
            ..Default::default()
        };
        let out = compose_system_prompt(&input);
        assert!(out.contains("Pre-flight"));
    }
}
