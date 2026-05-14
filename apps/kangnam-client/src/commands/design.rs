//! Tauri commands for the design-mode catalogs.
//!
//! Surfaces the `design-skill` and `design-system` crates' on-disk
//! catalog scanners to the renderer over `window.api.design.*`. The
//! frontend's NewProjectPanel (Phase 5b-13) consumes these to
//! populate skill / DS pickers; the FALLBACK lists in the panel are
//! used when no host is wired (web-only dev).
//!
//! Resolution policy:
//! - Default catalog roots are bundled with the SDK at
//!   `crates/design/design-skill/skills/` and
//!   `crates/design/design-system/systems/`.
//! - At runtime, the host first checks `KANGNAM_DESIGN_SKILLS_DIR`
//!   / `KANGNAM_DESIGN_SYSTEMS_DIR` env vars; falls back to a path
//!   relative to the binary's parent so dev builds work without
//!   configuration.
//! - The Tauri-bundled production layout will need a manifest path
//!   override; deferred to Phase 5c.

use std::path::PathBuf;

use serde::Serialize;

#[derive(Serialize)]
pub struct DesignSkillEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Triggers from Anthropic SkillFrontmatterV1.
    pub triggers: Vec<String>,
}

#[derive(Serialize)]
pub struct DesignSystemEntry {
    pub id: String,
    pub name: String,
    /// Brief — first paragraph of the DESIGN.md when present.
    pub description: String,
}

/// Full DESIGN.md preview payload — for the design-system preview modal
/// (Phase 5c-06). Returned by `design_system_get`.
#[derive(Serialize)]
pub struct DesignSystemDetail {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Full DESIGN.md body (markdown).
    pub body: String,
    /// Hex/oklch color tokens extracted by design-system parser.
    pub colors: Vec<String>,
}

fn resolve_dir(env_var: &str, default_rel: &str) -> Option<PathBuf> {
    if let Ok(p) = std::env::var(env_var) {
        let pb = PathBuf::from(p);
        if pb.is_dir() {
            return Some(pb);
        }
    }
    // Walk up from the running binary looking for the workspace
    // root. Dev builds run from `target/debug/...`; tauri bundles
    // typically copy resources alongside the binary.
    if let Ok(exe) = std::env::current_exe() {
        let mut cur = exe.parent().map(|p| p.to_path_buf());
        while let Some(p) = cur {
            let candidate = p.join(default_rel);
            if candidate.is_dir() {
                return Some(candidate);
            }
            cur = p.parent().map(|q| q.to_path_buf())
        }
    }
    None
}

#[tauri::command]
pub fn design_skill_list() -> Result<Vec<DesignSkillEntry>, String> {
    let dir = resolve_dir(
        "KANGNAM_DESIGN_SKILLS_DIR",
        "crates/design/design-skill/skills",
    )
    .ok_or_else(|| "design skills directory not found".to_string())?;
    let skills = kangnam_design_skill::load_skills_from_dir(&dir).map_err(|e| e.to_string())?;
    let out = skills
        .into_iter()
        .map(|s| DesignSkillEntry {
            id: s.id,
            name: s.name,
            description: s.description,
            triggers: s.triggers,
        })
        .collect();
    Ok(out)
}

#[tauri::command]
pub fn design_system_list() -> Result<Vec<DesignSystemEntry>, String> {
    let dir = resolve_dir(
        "KANGNAM_DESIGN_SYSTEMS_DIR",
        "crates/design/design-system/systems",
    )
    .ok_or_else(|| "design systems directory not found".to_string())?;
    let systems = kangnam_design_system::load_systems_from_dir(&dir).map_err(|e| e.to_string())?;
    let out = systems
        .into_iter()
        .map(|s| DesignSystemEntry {
            // DesignSystem doesn't carry a separate `name`/`description`
            // — derive from the id (titlecase) and the first non-empty
            // non-heading line of the body. Phase 5c can lift these
            // into structured catalog metadata if needed.
            description: extract_first_paragraph(&s.body),
            name: titlecase_id(&s.id),
            id: s.id,
        })
        .collect();
    Ok(out)
}

#[tauri::command]
pub fn design_system_get(id: String) -> Result<DesignSystemDetail, String> {
    let dir = resolve_dir(
        "KANGNAM_DESIGN_SYSTEMS_DIR",
        "crates/design/design-system/systems",
    )
    .ok_or_else(|| "design systems directory not found".to_string())?;
    let systems = kangnam_design_system::load_systems_from_dir(&dir).map_err(|e| e.to_string())?;
    let sys = systems
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| format!("design system not found: {id}"))?;
    let tokens = kangnam_design_system::tokens::extract_color_tokens(&sys.body);
    let colors: Vec<String> = tokens
        .into_iter()
        .filter(|t| matches!(t.kind, kangnam_design_system::tokens::ColorKind::Hex))
        .take(12)
        .map(|t| t.value)
        .collect();
    Ok(DesignSystemDetail {
        description: extract_first_paragraph(&sys.body),
        name: titlecase_id(&sys.id),
        body: sys.body,
        colors,
        id: sys.id,
    })
}

fn titlecase_id(id: &str) -> String {
    id.split('-')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_first_paragraph(body: &str) -> String {
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with("---") {
            continue;
        }
        return t.chars().take(160).collect();
    }
    String::new()
}
