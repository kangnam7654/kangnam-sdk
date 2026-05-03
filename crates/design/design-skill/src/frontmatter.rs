//! YAML frontmatter parser. Splits on `---` fences at the start of a file
//! and parses the body via `serde_yaml`. Falls back to `Value::Null` extras
//! when fields don't deserialize cleanly so vendored skills that use shapes
//! we don't model don't fail the catalog load.

use serde::Deserialize;
use thiserror::Error;

use crate::model::{DesignSkill, OdMetadata};

#[derive(Debug, Error)]
pub enum FrontmatterError {
    #[error("missing opening `---` fence")]
    MissingOpenFence,
    #[error("missing closing `---` fence")]
    MissingCloseFence,
    #[error("yaml parse: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("required frontmatter field `{0}` is missing or not a string")]
    RequiredField(&'static str),
}

/// Parse a `SKILL.md` file body into a (frontmatter Value, markdown body)
/// tuple. The frontmatter Value is a raw `serde_yaml::Value` — typed
/// extraction happens in [`build_design_skill`].
pub fn parse_frontmatter_raw(input: &str) -> Result<(serde_yaml::Value, String), FrontmatterError> {
    let trimmed = input.trim_start();
    if !trimmed.starts_with("---") {
        return Err(FrontmatterError::MissingOpenFence);
    }
    // Skip the opening fence line.
    let after_open = trimmed
        .splitn(2, '\n')
        .nth(1)
        .ok_or(FrontmatterError::MissingCloseFence)?;
    // The closing fence is a line that is exactly `---`.
    let mut frontmatter = String::new();
    let mut body_start: Option<usize> = None;
    let mut idx = 0usize;
    for line in after_open.split_inclusive('\n') {
        let stripped = line.trim_end_matches('\n');
        if stripped == "---" {
            body_start = Some(idx + line.len());
            break;
        }
        frontmatter.push_str(line);
        idx += line.len();
    }
    let Some(start) = body_start else {
        return Err(FrontmatterError::MissingCloseFence);
    };
    let body = after_open[start..].trim_start_matches('\n').to_string();
    let yaml: serde_yaml::Value = serde_yaml::from_str(&frontmatter)?;
    Ok((yaml, body))
}

/// Convenience: parse a SKILL.md, returning a typed [`DesignSkill`] (without
/// `id` / `root` set — those come from the directory loader).
pub fn parse_frontmatter(input: &str) -> Result<DesignSkill, FrontmatterError> {
    let (raw, body) = parse_frontmatter_raw(input)?;
    let map = raw.as_mapping().cloned().unwrap_or_default();

    fn pluck_string(map: &serde_yaml::Mapping, key: &str) -> Option<String> {
        let v = map.get(serde_yaml::Value::String(key.to_string()))?;
        // Accept plain string, or a multiline block scalar.
        v.as_str().map(|s| s.trim().to_string())
    }

    let name = pluck_string(&map, "name").ok_or(FrontmatterError::RequiredField("name"))?;
    let description = pluck_string(&map, "description").unwrap_or_default();

    // triggers may be an array, a single string, or absent.
    let triggers: Vec<String> = match map.get(serde_yaml::Value::String("triggers".into())) {
        Some(serde_yaml::Value::Sequence(seq)) => seq
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        Some(serde_yaml::Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    };

    // od: namespace — best-effort serde_yaml deserialize; on shape mismatch
    // we keep going with default.
    let od: OdMetadata = match map.get(serde_yaml::Value::String("od".into())) {
        Some(v) => OdMetadata::deserialize(v.clone()).unwrap_or_default(),
        None => OdMetadata::default(),
    };

    // frontmatter_extras: every other top-level key. Convert via JSON for
    // stable shape (serde_yaml::Value -> serde_json::Value).
    let mut extras_yaml = map.clone();
    for known in ["name", "description", "triggers", "od"] {
        extras_yaml.remove(serde_yaml::Value::String(known.into()));
    }
    let extras_json: serde_json::Value =
        serde_json::to_value(&extras_yaml).unwrap_or(serde_json::Value::Null);

    Ok(DesignSkill {
        id: String::new(),
        name,
        description,
        triggers,
        body,
        od,
        frontmatter_extras: extras_json,
        root: std::path::PathBuf::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_frontmatter_with_od() {
        let input = "---\nname: web-prototype\ndescription: |\n  A page.\ntriggers:\n  - mockup\n  - landing\nod:\n  mode: prototype\n  platform: desktop\n  preview:\n    type: html\n    entry: index.html\n  kangnam_design_system:\n    requires: true\n    sections: [color, typography]\n---\n\n# Body\nhello\n";
        let parsed = parse_frontmatter(input).unwrap();
        assert_eq!(parsed.name, "web-prototype");
        assert_eq!(parsed.description, "A page.");
        assert_eq!(parsed.triggers, vec!["mockup", "landing"]);
        assert_eq!(parsed.od.mode.as_deref(), Some("prototype"));
        assert_eq!(parsed.od.platform.as_deref(), Some("desktop"));
        let preview = parsed.od.preview.as_ref().unwrap();
        assert_eq!(preview.kind.as_deref(), Some("html"));
        let ds = parsed.od.kangnam_design_system.as_ref().unwrap();
        assert!(ds.requires);
        assert_eq!(ds.sections, vec!["color", "typography"]);
        assert!(parsed.body.starts_with("# Body"));
    }

    #[test]
    fn missing_open_fence_errors() {
        let err = parse_frontmatter("name: x\n").unwrap_err();
        assert!(matches!(err, FrontmatterError::MissingOpenFence));
    }

    #[test]
    fn missing_close_fence_errors() {
        let err = parse_frontmatter("---\nname: x\nno close\n").unwrap_err();
        assert!(matches!(err, FrontmatterError::MissingCloseFence));
    }

    #[test]
    fn missing_required_name_errors() {
        let err = parse_frontmatter("---\ndescription: x\n---\nbody\n").unwrap_err();
        assert!(matches!(err, FrontmatterError::RequiredField("name")));
    }

    #[test]
    fn arbitrary_extras_land_in_frontmatter_extras() {
        let input = "---\nname: x\nallowed_tools: [Read, Bash]\nuser_invocable: true\n---\nbody\n";
        let parsed = parse_frontmatter(input).unwrap();
        let extras = parsed.frontmatter_extras.as_object().unwrap();
        assert!(extras.contains_key("allowed_tools"));
        assert!(extras.contains_key("user_invocable"));
    }
}
