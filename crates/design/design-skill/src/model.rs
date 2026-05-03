//! Typed `DesignSkill` and the `od:` namespace.
//!
//! `DesignSkill` is the in-memory record produced by the [`loader`] from a
//! `SKILL.md` directory. Convertible to `kangnam_harness_core::Skill` via
//! [`DesignSkill::to_harness_skill`] (lossy: `triggers: Vec<String>` →
//! `SkillTrigger::Auto { keywords }`; the `od:`/Anthropic-extras land in
//! `frontmatter_extras`).

use std::path::PathBuf;

use kangnam_harness_core::{Scope, Skill, SkillReference, SkillTrigger};
use serde::{Deserialize, Serialize};

/// `od:` namespace from open-design's SKILL.md extension.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OdMetadata {
    /// `prototype` / `deck` / `template` / `design-system` (free-form).
    #[serde(default)]
    pub mode: Option<String>,
    /// `desktop` / `mobile` / `tablet` / etc.
    #[serde(default)]
    pub platform: Option<String>,
    /// Free-form (`design`, `marketing`, `docs`, …).
    #[serde(default)]
    pub scenario: Option<String>,
    /// Preview hint for the host UI.
    #[serde(default)]
    pub preview: Option<OdPreview>,
    /// Whether the skill needs an active DESIGN.md.
    #[serde(default)]
    pub kangnam_design_system: Option<OdDesignSystem>,
    /// Catch-all for any further keys we haven't promoted.
    #[serde(flatten)]
    pub extras: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OdPreview {
    #[serde(default, rename = "type", alias = "kind")]
    pub kind: Option<String>,
    #[serde(default)]
    pub entry: Option<String>,
    #[serde(flatten)]
    pub extras: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OdDesignSystem {
    #[serde(default)]
    pub requires: bool,
    #[serde(default)]
    pub sections: Vec<String>,
}

/// In-memory representation of one SKILL.md bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignSkill {
    /// Directory name (kebab-case id), e.g. `web-prototype`.
    pub id: String,
    /// Anthropic frontmatter `name` field (often equals `id`).
    pub name: String,
    /// Anthropic `description`.
    #[serde(default)]
    pub description: String,
    /// Anthropic `triggers: [string]`. Empty for manual-only skills.
    #[serde(default)]
    pub triggers: Vec<String>,
    /// Body markdown (everything after the closing `---`).
    pub body: String,
    /// Open-design `od:` extension. Empty if the skill predates it.
    #[serde(default)]
    pub od: OdMetadata,
    /// Open-codesign / Anthropic V1 extras (user_invocable, allowed_tools,
    /// disable_model_invocation, …) that don't map to canonical columns.
    /// Preserved verbatim.
    #[serde(default)]
    pub frontmatter_extras: serde_json::Value,
    /// On-disk path of the directory holding `SKILL.md` + `assets/` +
    /// `references/`. Side-file content is loaded lazily by callers.
    #[serde(skip_serializing, skip_deserializing)]
    pub root: PathBuf,
}

impl DesignSkill {
    /// Convert into the harness-core canonical [`Skill`]. Lossy by design:
    /// `triggers` → `SkillTrigger::Auto { keywords }`; everything else that
    /// doesn't map to a canonical column lands in `frontmatter_extras` so
    /// the SQLite blob roundtrip preserves it.
    pub fn to_harness_skill(&self) -> Skill {
        let trigger = if self.triggers.is_empty() {
            SkillTrigger::Manual
        } else {
            SkillTrigger::Auto {
                keywords: self.triggers.clone(),
            }
        };
        // Compose extras: od: + frontmatter_extras, with frontmatter_extras
        // winning on conflict (it carries fields the caller explicitly passed
        // in addition to od:).
        let mut extras = serde_json::Map::new();
        if let Ok(serde_json::Value::Object(od_obj)) = serde_json::to_value(&self.od) {
            extras.extend(od_obj);
        }
        if let serde_json::Value::Object(extra_obj) = self.frontmatter_extras.clone() {
            extras.extend(extra_obj);
        }
        Skill {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            trigger,
            content: self.body.clone(),
            references: Vec::<SkillReference>::new(),
            scope: Scope::User,
            sort_order: 0,
            frontmatter_extras: serde_json::Value::Object(extras),
        }
    }
}
