//! Project metadata — what the new-project panel captures, surfaced to the
//! agent at compose time so it knows the artifact kind, fidelity, etc.

use serde::{Deserialize, Serialize};

/// Top-level artifact kind. Drives mode selection (deck framework, mobile
/// frame …) and the "What are we making?" default in the discovery form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProjectKind {
    #[default]
    Prototype,
    Deck,
    Dashboard,
    Mobile,
    Editorial,
    DesignSystem,
}

impl ProjectKind {
    pub fn label(&self) -> &'static str {
        match self {
            ProjectKind::Prototype => "Single web prototype / landing",
            ProjectKind::Deck => "Slide deck / pitch",
            ProjectKind::Dashboard => "Dashboard / tool UI",
            ProjectKind::Mobile => "Multi-screen app prototype",
            ProjectKind::Editorial => "Editorial / marketing page",
            ProjectKind::DesignSystem => "Design system",
        }
    }
}

/// Skill-level mode hint mirrored from `SKILL.md`'s `od.mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillMode {
    Prototype,
    Deck,
    Template,
    DesignSystem,
}

/// Optional fidelity hint — wireframe / hi-fi / print-quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Fidelity {
    Wireframe,
    #[default]
    Hi,
    Print,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub kind: ProjectKind,
    /// `true` when the user named a brand in the brief — triggers the brand
    /// asset protocol block in the composed prompt.
    #[serde(default)]
    pub brand_named: bool,
    #[serde(default)]
    pub fidelity: Fidelity,
    /// Whether speaker notes are expected (deck-mode only).
    #[serde(default)]
    pub speaker_notes: bool,
    /// Whether motion / animation is in scope.
    #[serde(default)]
    pub animations: bool,
    /// Inspiration ids the user pinned (e.g. design-system ids).
    #[serde(default)]
    pub inspiration_ids: Vec<String>,
    /// Optional title.
    #[serde(default)]
    pub title: Option<String>,
}

impl ProjectMetadata {
    /// Render a `## Project metadata` markdown block for splicing into the
    /// composed system prompt. Returns `None` when the metadata is the
    /// default empty value.
    pub fn render_block(&self, template: Option<&ProjectTemplate>) -> Option<String> {
        if self.is_empty() && template.is_none() {
            return None;
        }
        let mut s = String::from("## Project metadata\n\n");
        s.push_str(&format!("- **Kind**: {}\n", self.kind.label()));
        s.push_str(&format!("- **Fidelity**: {:?}\n", self.fidelity));
        if self.brand_named {
            s.push_str("- **Brand context**: a specific brand was named — run brand asset protocol below.\n");
        }
        if self.speaker_notes {
            s.push_str("- **Speaker notes**: expected (emit `<aside class=\"speaker-notes\">` siblings).\n");
        }
        if self.animations {
            s.push_str("- **Motion**: in scope.\n");
        }
        if !self.inspiration_ids.is_empty() {
            s.push_str(&format!(
                "- **Inspiration**: {}.\n",
                self.inspiration_ids.join(", ")
            ));
        }
        if let Some(t) = &self.title {
            s.push_str(&format!("- **Title**: {t}\n"));
        }
        if let Some(t) = template {
            if !t.files.is_empty() {
                s.push_str(&format!(
                    "- **Starter template**: {} files attached. Use as a starting reference, not a fixed deliverable.\n",
                    t.files.len()
                ));
            }
        }
        Some(s)
    }

    fn is_empty(&self) -> bool {
        matches!(self.kind, ProjectKind::Prototype)
            && !self.brand_named
            && matches!(self.fidelity, Fidelity::Hi)
            && !self.speaker_notes
            && !self.animations
            && self.inspiration_ids.is_empty()
            && self.title.is_none()
    }
}

/// Optional starter template — raw HTML files the user picked from the
/// "From template" tab. We surface the count, not the full content (the
/// agent reads files via the harness).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectTemplate {
    pub id: String,
    pub files: Vec<TemplateFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateFile {
    pub path: String,
    pub size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_metadata_renders_no_block() {
        assert!(ProjectMetadata::default().render_block(None).is_none());
    }

    #[test]
    fn brand_named_block_mentions_protocol() {
        let m = ProjectMetadata {
            brand_named: true,
            ..Default::default()
        };
        let block = m.render_block(None).unwrap();
        assert!(block.contains("brand asset protocol"));
    }

    #[test]
    fn deck_metadata_includes_speaker_notes_when_set() {
        let m = ProjectMetadata {
            kind: ProjectKind::Deck,
            speaker_notes: true,
            ..Default::default()
        };
        let block = m.render_block(None).unwrap();
        assert!(block.contains("Slide deck"));
        assert!(block.contains("Speaker notes"));
    }
}
