//! Tauri commands for the design-mode FileWorkspace (Phase 5c-07).
//!
//! Three operations:
//! - `project_files_list(working_dir)` — recursive tree listing,
//!   skipping hidden + node_modules / target / .git
//! - `project_file_read(working_dir, rel_path)` — read text body
//! - `project_file_write(working_dir, rel_path, body)` — write text
//!   body, creating parents
//!
//! All paths are resolved as `working_dir.join(rel_path)` and verified
//! to remain inside `working_dir` (path traversal guard).

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::path_guard::{ensure_no_symlink_components, validate_relative_path};

#[derive(Serialize)]
pub struct ProjectFileEntry {
    /// POSIX-style relative path from `working_dir`.
    pub path: String,
    /// `dir` | `file`.
    pub kind: &'static str,
    /// Bytes (for files only). `None` for directories.
    pub size: Option<u64>,
}

const SKIP_DIRS: &[&str] = &["node_modules", "target", ".git", "dist", ".next", ".turbo"];
const MAX_BYTES: u64 = 5 * 1024 * 1024; // 5 MiB read cap

fn resolve_inside(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let candidate = root.join(rel);
    let canon_root = root
        .canonicalize()
        .map_err(|e| format!("canonicalize root: {e}"))?;
    let canon = candidate
        .canonicalize()
        .map_err(|e| format!("canonicalize target: {e}"))?;
    if !canon.starts_with(&canon_root) {
        return Err(format!("path escapes working_dir: {rel}"));
    }
    Ok(canon)
}

#[tauri::command]
pub fn project_files_list(working_dir: String) -> Result<Vec<ProjectFileEntry>, String> {
    let root = PathBuf::from(&working_dir);
    if !root.is_dir() {
        return Err(format!("not a directory: {working_dir}"));
    }
    let mut out: Vec<ProjectFileEntry> = Vec::new();
    walk(&root, &root, &mut out)?;
    // Stable order: dirs first, then files, both alphabetical.
    out.sort_by(|a, b| {
        let ak = a.kind == "dir";
        let bk = b.kind == "dir";
        if ak != bk {
            return bk.cmp(&ak); // dirs first
        }
        a.path.cmp(&b.path)
    });
    Ok(out)
}

fn walk(root: &Path, cur: &Path, out: &mut Vec<ProjectFileEntry>) -> Result<(), String> {
    let rd = std::fs::read_dir(cur).map_err(|e| format!("read_dir: {e}"))?;
    for entry in rd.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if name.starts_with('.') {
            continue;
        }
        let ft = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        let rel = match path.strip_prefix(root) {
            Ok(p) => p.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        if ft.is_dir() {
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            out.push(ProjectFileEntry {
                path: rel.clone(),
                kind: "dir",
                size: None,
            });
            walk(root, &path, out)?;
        } else if ft.is_file() {
            let size = path.metadata().ok().map(|m| m.len());
            out.push(ProjectFileEntry {
                path: rel,
                kind: "file",
                size,
            });
        }
    }
    Ok(())
}

#[tauri::command]
pub fn project_file_read(working_dir: String, rel_path: String) -> Result<String, String> {
    let root = PathBuf::from(&working_dir);
    let target = resolve_inside(&root, &rel_path)?;
    let meta = target.metadata().map_err(|e| format!("metadata: {e}"))?;
    if meta.len() > MAX_BYTES {
        return Err(format!(
            "file too large: {} bytes (max {} bytes)",
            meta.len(),
            MAX_BYTES
        ));
    }
    std::fs::read_to_string(&target).map_err(|e| format!("read: {e}"))
}

#[tauri::command]
pub fn project_file_write(
    working_dir: String,
    rel_path: String,
    body: String,
) -> Result<(), String> {
    let root = PathBuf::from(&working_dir);
    let canon_root = root
        .canonicalize()
        .map_err(|e| format!("canonicalize root: {e}"))?;
    let clean_rel = validate_relative_path(&rel_path, "rel_path")?;
    ensure_no_symlink_components(&canon_root, &clean_rel)?;

    let target = canon_root.join(&clean_rel);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir parents: {e}"))?;
        let canon_parent = parent
            .canonicalize()
            .map_err(|e| format!("canonicalize parent: {e}"))?;
        if !canon_parent.starts_with(&canon_root) {
            return Err(format!("path escapes working_dir: {rel_path}"));
        }
    }
    std::fs::write(&target, body.as_bytes()).map_err(|e| format!("write: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_rejects_parent_traversal_before_creating_dirs() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let before = outside.path().join("created");

        let rel = format!(
            "../{}/created/file.txt",
            outside.path().file_name().unwrap().to_string_lossy()
        );
        let err = project_file_write(
            root.path().to_string_lossy().to_string(),
            rel,
            "x".to_string(),
        )
        .unwrap_err();

        assert!(err.contains("relative path without traversal"));
        assert!(!before.exists());
    }

    #[cfg(unix)]
    #[test]
    fn write_rejects_symlink_targets() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_file = outside.path().join("outside.txt");
        std::fs::write(&outside_file, "before").unwrap();
        std::os::unix::fs::symlink(&outside_file, root.path().join("link.txt")).unwrap();

        let err = project_file_write(
            root.path().to_string_lossy().to_string(),
            "link.txt".to_string(),
            "after".to_string(),
        )
        .unwrap_err();

        assert!(err.contains("refusing to follow symlink"));
        assert_eq!(std::fs::read_to_string(outside_file).unwrap(), "before");
    }
}
