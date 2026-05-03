//! SKILL.md catalog — Anthropic-compat frontmatter + `od:` extensions.
//!
//! Each skill is a directory containing `SKILL.md` (frontmatter + body) plus
//! optional `assets/` and `references/` side files. Frontmatter follows
//! Anthropic's [SkillFrontmatterV1][skv] convention augmented with the
//! open-design [`od:`][odp] namespace (mode / platform / scenario / preview /
//! kangnam_design_system) and open-codesign extras (user_invocable, allowed_tools,
//! disable_model_invocation).
//!
//! Ships a 30-skill vendored catalog at `skills/` (web-prototype, dashboard,
//! mobile-app, guizang-ppt, …) — Apache-2.0, with the bundled `guizang-ppt`
//! skill retaining its original MIT LICENSE.
//!
//! [skv]: https://docs.anthropic.com/en/docs/claude-code/skills
//! [odp]: https://github.com/nexu-io/open-design/blob/main/docs/skills-protocol.md

pub mod frontmatter;
pub mod loader;
pub mod model;

pub use frontmatter::{parse_frontmatter, FrontmatterError};
pub use loader::{list_skill_ids, load_skill, load_skills_from_dir, LoadError};
pub use model::{DesignSkill, OdMetadata, OdPreview, OdDesignSystem};
