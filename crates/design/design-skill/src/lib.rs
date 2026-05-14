//! SKILL.md catalog — Anthropic-compat frontmatter + `od:` extensions.
//!
//! Each skill is a directory containing `SKILL.md` (frontmatter + body) plus
//! optional `assets/` and `references/` side files. Frontmatter follows
//! Anthropic's [SkillFrontmatterV1][skv] convention augmented with the
//! open-design [`od:`][odp] namespace (mode / platform / scenario / preview /
//! kangnam_design_system / craft) and open-codesign extras (user_invocable,
//! allowed_tools, disable_model_invocation).
//!
//! Ships a 64-skill vendored catalog at `skills/` covering web-prototype +
//! taste variants, dashboard, mobile-app, the html-ppt family (16 themes),
//! kami-deck/kami-landing, hatch-pet, guizang-ppt, replit-deck, and others —
//! Apache-2.0, with bundled `guizang-ppt`, `html-ppt`, and `hatch-pet`
//! skills retaining their original LICENSE files.
//!
//! Skills opt into brand-agnostic craft rulebooks (typography, color, a11y,
//! motion, RTL/BiDi, anti-AI-slop, state coverage) via the `od.craft.requires`
//! array — see the [`kangnam-design-craft`] crate.
//!
//! ## Protocol references
//!
//! Vendored from open-design at `docs/`:
//! - [`docs/skills-protocol.md`](https://github.com/kangnam7654/kangnam-sdk/blob/main/crates/design/design-skill/docs/skills-protocol.md)
//!   — full `od:` namespace spec, including `od.craft.requires`,
//!   `od.kangnam_design_system`, mode/platform/scenario taxonomy.
//! - [`docs/modes.md`](https://github.com/kangnam7654/kangnam-sdk/blob/main/crates/design/design-skill/docs/modes.md)
//!   — the 7 mode/surface combinations (`prototype`, `deck`, `template`,
//!   `design-system`, `image`, `video`, `audio`).
//!
//! [skv]: https://docs.anthropic.com/en/docs/claude-code/skills
//! [odp]: https://github.com/nexu-io/open-design/blob/main/docs/skills-protocol.md

pub mod frontmatter;
pub mod loader;
pub mod model;

pub use frontmatter::{FrontmatterError, parse_frontmatter};
pub use loader::{LoadError, list_skill_ids, load_skill, load_skills_from_dir};
pub use model::{DesignSkill, OdCraft, OdDesignSystem, OdMetadata, OdPreview};
