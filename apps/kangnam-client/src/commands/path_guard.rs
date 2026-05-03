use std::path::{Component, Path, PathBuf};

pub fn validate_single_component(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{label} must not be empty"));
    }

    let mut components = Path::new(value).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(()),
        _ => Err(format!("{label} must be a single path component")),
    }
}

pub fn validate_relative_path(value: &str, label: &str) -> Result<PathBuf, String> {
    if value.is_empty() {
        return Err(format!("{label} must not be empty"));
    }

    let mut out = PathBuf::new();
    for component in Path::new(value).components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err(format!("{label} must be a relative path without traversal"));
            }
        }
    }

    if out.as_os_str().is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    Ok(out)
}

pub fn ensure_no_symlink_components(root: &Path, relative: &Path) -> Result<(), String> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Err("path must be normalized before symlink checks".to_string());
        };
        current.push(part);
        if let Ok(meta) = std::fs::symlink_metadata(&current) {
            if meta.file_type().is_symlink() {
                return Err(format!("refusing to follow symlink: {}", current.display()));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_component_rejects_traversal() {
        assert!(validate_single_component("../x", "name").is_err());
        assert!(validate_single_component("a/b", "name").is_err());
        assert!(validate_single_component("agent.md", "name").is_ok());
    }

    #[test]
    fn relative_path_rejects_escape_components() {
        assert!(validate_relative_path("../x", "path").is_err());
        assert!(validate_relative_path("/tmp/x", "path").is_err());
        assert!(validate_relative_path("refs/a.md", "path").is_ok());
    }
}
