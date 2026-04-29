use std::collections::HashMap;
use std::path::PathBuf;
use serde::Serialize;

// ── Claude Code file-based commands (~/.claude/commands/) ──
// Supports both single-file (name.md) and directory-based (name/SKILL.md + refs)

#[derive(Serialize)]
pub struct ClaudeCommandInfo {
    pub name: String,
    pub description: String,
    pub is_directory: bool,
}

#[derive(Serialize)]
pub struct ClaudeCommandFull {
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub is_directory: bool,
    pub refs: Vec<SkillFileInfo>,
}

#[derive(Serialize, Clone)]
pub struct SkillFileInfo {
    pub filename: String,
    pub size: u64,
    pub is_main: bool,
}

fn claude_commands_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("commands"))
}

fn claude_skills_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("skills"))
}

/// Find the actual directory a skill lives in (skills/ first, then commands/)
fn find_skill_path(name: &str) -> Option<PathBuf> {
    // Check ~/.claude/skills/{name}/ (directory)
    if let Some(skills_dir) = claude_skills_dir() {
        let dir_path = skills_dir.join(name);
        if dir_path.is_dir() && dir_path.join("SKILL.md").exists() {
            return Some(dir_path);
        }
    }
    // Check ~/.claude/commands/{name}/ (directory)
    if let Some(cmds_dir) = claude_commands_dir() {
        let dir_path = cmds_dir.join(name);
        if dir_path.is_dir() && dir_path.join("SKILL.md").exists() {
            return Some(dir_path);
        }
        // Check ~/.claude/commands/{name}.md (single file)
        let file_path = cmds_dir.join(format!("{}.md", name));
        if file_path.is_file() {
            return Some(file_path);
        }
    }
    None
}

fn parse_frontmatter(content: &str) -> (HashMap<String, String>, String) {
    if !content.starts_with("---") {
        return (HashMap::new(), content.to_string());
    }
    if let Some(end) = content[4..].find("\n---") {
        let fm_str = &content[4..4 + end];
        let body = &content[4 + end + 4..];
        let mut fm = HashMap::new();
        for line in fm_str.lines() {
            let line = line.trim();
            if let Some((key, val)) = line.split_once(':') {
                let val = val.trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() {
                    fm.insert(key.trim().to_string(), val.to_string());
                }
            }
        }
        (fm, body.trim_start_matches('\n').to_string())
    } else {
        (HashMap::new(), content.to_string())
    }
}

/// List all files in a skill directory (not just .md)
fn list_dir_files(dir: &PathBuf) -> Vec<SkillFileInfo> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                let is_main = filename == "SKILL.md" || filename == "index.md";
                files.push(SkillFileInfo { filename, size, is_main });
            }
        }
    }
    files.sort_by(|a, b| {
        // Main file first, then alphabetical
        b.is_main.cmp(&a.is_main).then(a.filename.cmp(&b.filename))
    });
    files
}

fn scan_skill_directory(dir: &PathBuf, seen: &mut std::collections::HashSet<String>, commands: &mut Vec<ClaudeCommandInfo>) {
    if !dir.exists() { return; }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let skill_md = path.join("SKILL.md");
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                if seen.contains(&name) { continue; }
                // Skip workspace/iteration directories (not skills)
                if name.ends_with("-workspace") { continue; }
                let description = if skill_md.exists() {
                    let content = std::fs::read_to_string(&skill_md).unwrap_or_default();
                    let (fm, _) = parse_frontmatter(&content);
                    fm.get("description").cloned().unwrap_or_default()
                } else {
                    continue; // No SKILL.md = not a valid skill
                };
                seen.insert(name.clone());
                commands.push(ClaudeCommandInfo { name, description, is_directory: true });
            } else if path.extension().map_or(false, |ext| ext == "md") && path.is_file() {
                let name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                if seen.contains(&name) { continue; }
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                let (fm, _) = parse_frontmatter(&content);
                seen.insert(name.clone());
                commands.push(ClaudeCommandInfo {
                    name,
                    description: fm.get("description").cloned().unwrap_or_default(),
                    is_directory: false,
                });
            }
        }
    }
}

#[tauri::command]
pub fn list_claude_commands() -> Result<Vec<ClaudeCommandInfo>, String> {
    let mut commands = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Scan ~/.claude/skills/ first (primary)
    if let Some(skills_dir) = claude_skills_dir() {
        scan_skill_directory(&skills_dir, &mut seen, &mut commands);
    }
    // Then ~/.claude/commands/ (secondary, no duplicates)
    if let Some(cmds_dir) = claude_commands_dir() {
        scan_skill_directory(&cmds_dir, &mut seen, &mut commands);
    }

    commands.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(commands)
}

#[tauri::command]
pub fn read_claude_command(name: String) -> Result<Option<ClaudeCommandFull>, String> {
    let path = match find_skill_path(&name) {
        Some(p) => p,
        None => return Ok(None),
    };

    if path.is_dir() {
        let skill_md = path.join("SKILL.md");
        if !skill_md.exists() { return Ok(None); }
        let content = std::fs::read_to_string(&skill_md).map_err(|e| e.to_string())?;
        let (fm, body) = parse_frontmatter(&content);
        let refs = list_dir_files(&path);
        Ok(Some(ClaudeCommandFull {
            name: fm.get("name").cloned().unwrap_or_else(|| name),
            description: fm.get("description").cloned().unwrap_or_default(),
            instructions: body,
            is_directory: true,
            refs,
        }))
    } else {
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let (fm, body) = parse_frontmatter(&content);
        Ok(Some(ClaudeCommandFull {
            name: fm.get("name").cloned().unwrap_or_else(|| name),
            description: fm.get("description").cloned().unwrap_or_default(),
            instructions: body,
            is_directory: false,
            refs: vec![],
        }))
    }
}

#[tauri::command]
pub fn write_claude_command(
    name: String,
    description: String,
    instructions: String,
) -> Result<(), String> {
    let mut content = String::from("---\n");
    content.push_str(&format!("name: {}\n", name));
    content.push_str(&format!("description: {}\n", description));
    content.push_str("---\n\n");
    content.push_str(&instructions);
    content.push('\n');

    // Write to existing location if skill found
    if let Some(path) = find_skill_path(&name) {
        if path.is_dir() {
            std::fs::write(path.join("SKILL.md"), content).map_err(|e| e.to_string())?;
        } else {
            std::fs::write(&path, content).map_err(|e| e.to_string())?;
        }
    } else {
        // New skill → create in ~/.claude/skills/{name}/SKILL.md
        let skills_dir = claude_skills_dir().ok_or("No home directory")?;
        let skill_dir = skills_dir.join(&name);
        std::fs::create_dir_all(&skill_dir).map_err(|e| e.to_string())?;
        std::fs::write(skill_dir.join("SKILL.md"), content).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn delete_claude_command(name: String) -> Result<(), String> {
    if let Some(path) = find_skill_path(&name) {
        if path.is_dir() {
            std::fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
        } else {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// ── Fork plugin skill to custom ──

#[tauri::command]
pub fn fork_plugin_skill(plugin_path: String, skill_name: String) -> Result<(), String> {
    let src_dir = PathBuf::from(&plugin_path).join("skills").join(&skill_name);
    let dst_dir = claude_skills_dir().ok_or("No home directory")?.join(&skill_name);

    if !src_dir.exists() {
        return Err(format!("Plugin skill not found at: {}", src_dir.display()));
    }

    std::fs::create_dir_all(&dst_dir).map_err(|e| e.to_string())?;

    // Copy all files (md, sh, etc.)
    for entry in std::fs::read_dir(&src_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_file() {
            let filename = path.file_name().unwrap();
            std::fs::copy(&path, dst_dir.join(filename)).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

// ── Reference file management ──

#[tauri::command]
pub fn list_skill_refs(name: String) -> Result<Vec<SkillFileInfo>, String> {
    match find_skill_path(&name) {
        Some(path) if path.is_dir() => Ok(list_dir_files(&path)),
        _ => Ok(vec![]),
    }
}

#[tauri::command]
pub fn read_skill_ref(name: String, filename: String) -> Result<String, String> {
    let skill_path = find_skill_path(&name)
        .ok_or_else(|| format!("Skill '{}' not found", name))?;
    if !skill_path.is_dir() {
        return Err("Skill is not a directory".to_string());
    }
    let path = skill_path.join(&filename);
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn write_skill_ref(name: String, filename: String, content: String) -> Result<(), String> {
    let skill_path = find_skill_path(&name)
        .ok_or_else(|| format!("Skill '{}' not found", name))?;
    if !skill_path.is_dir() {
        return Err("Cannot add ref to single-file skill".to_string());
    }
    std::fs::write(skill_path.join(&filename), &content).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_skill_ref(name: String, filename: String) -> Result<(), String> {
    if filename == "SKILL.md" || filename == "index.md" {
        return Err("Cannot delete main skill file".to_string());
    }
    let skill_path = find_skill_path(&name)
        .ok_or_else(|| format!("Skill '{}' not found", name))?;
    if !skill_path.is_dir() { return Ok(()); }
    let path = skill_path.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Snapshot management ──

fn snapshots_dir(item_type: &str) -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("studio-snapshots").join(item_type))
}

#[derive(Serialize)]
pub struct SnapshotInfo {
    pub filename: String,
    pub timestamp: u64,
    pub size: u64,
}

#[tauri::command]
pub fn snapshot_skill(name: String) -> Result<String, String> {
    let skill_path = find_skill_path(&name)
        .ok_or_else(|| format!("Skill '{}' not found", name))?;

    let content = if skill_path.is_dir() {
        let skill_md = skill_path.join("SKILL.md");
        std::fs::read_to_string(&skill_md).map_err(|e| e.to_string())?
    } else {
        std::fs::read_to_string(&skill_path).map_err(|e| e.to_string())?
    };

    let snap_dir = snapshots_dir("skills").ok_or("No home directory")?;
    let item_dir = snap_dir.join(&name);
    std::fs::create_dir_all(&item_dir).map_err(|e| e.to_string())?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let filename = format!("{}.md", ts);
    let path = item_dir.join(&filename);
    std::fs::write(&path, &content).map_err(|e| e.to_string())?;

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn list_skill_snapshots(name: String) -> Result<Vec<SnapshotInfo>, String> {
    let snap_dir = snapshots_dir("skills").ok_or("No home directory")?;
    let item_dir = snap_dir.join(&name);
    if !item_dir.exists() { return Ok(vec![]); }

    let mut snapshots = Vec::new();
    for entry in std::fs::read_dir(&item_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_file() {
            let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let timestamp = filename.trim_end_matches(".md").parse::<u64>().unwrap_or(0);
            snapshots.push(SnapshotInfo { filename, timestamp, size });
        }
    }
    snapshots.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(snapshots)
}
