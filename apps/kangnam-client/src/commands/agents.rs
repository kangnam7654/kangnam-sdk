use std::collections::HashMap;
use std::sync::Arc;
use serde::Serialize;
use tauri::State;

use crate::db::agents::{self, Agent};
use crate::state::AppState;

// ── Claude Code file-based agents (~/.claude/agents/) ──

#[derive(Serialize, Clone)]
pub struct AgentFileInfo {
    pub filename: String,
    pub size: u64,
    pub is_main: bool,
}

#[derive(Serialize)]
pub struct ClaudeAgentInfo {
    pub name: String,
    pub description: String,
    pub model: Option<String>,
    pub is_directory: bool,
}

#[derive(Serialize)]
pub struct ClaudeAgentFull {
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub model: Option<String>,
    pub is_directory: bool,
    pub refs: Vec<AgentFileInfo>,
}

fn claude_agents_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("agents"))
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

/// Find the refs subdirectory for an agent (refs/ or references/)
fn agent_refs_dir(agent_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let refs = agent_dir.join("refs");
    if refs.is_dir() { return Some(refs); }
    let references = agent_dir.join("references");
    if references.is_dir() { return Some(references); }
    None
}

/// List all files in an agent's refs directory
fn list_agent_ref_files(agent_dir: &std::path::Path) -> Vec<AgentFileInfo> {
    let mut files = Vec::new();
    if let Some(refs_dir) = agent_refs_dir(agent_dir) {
        if let Ok(entries) = std::fs::read_dir(&refs_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    files.push(AgentFileInfo { filename, size, is_main: false });
                }
            }
        }
    }
    files.sort_by(|a, b| a.filename.cmp(&b.filename));
    files
}

#[tauri::command]
pub fn list_claude_agents() -> Result<Vec<ClaudeAgentInfo>, String> {
    let dir = claude_agents_dir().ok_or("No home directory")?;
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut agents = Vec::new();
    // Only .md files are agents; directories are supplementary (refs)
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "md") && path.is_file() {
            let name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            let (fm, _) = parse_frontmatter(&content);
            let has_refs_dir = dir.join(&name).is_dir();
            agents.push(ClaudeAgentInfo {
                name,
                description: fm.get("description").cloned().unwrap_or_default(),
                model: fm.get("model").cloned(),
                is_directory: has_refs_dir,
            });
        }
    }
    agents.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(agents)
}

#[tauri::command]
pub fn read_claude_agent(name: String) -> Result<Option<ClaudeAgentFull>, String> {
    let dir = claude_agents_dir().ok_or("No home directory")?;
    // Agent definition is always {name}.md
    let path = dir.join(format!("{}.md", name));
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let (fm, body) = parse_frontmatter(&content);
    // Check for companion directory with refs
    let agent_dir = dir.join(&name);
    let has_dir = agent_dir.is_dir();
    let refs = if has_dir { list_agent_ref_files(&agent_dir) } else { vec![] };
    Ok(Some(ClaudeAgentFull {
        name: fm.get("name").cloned().unwrap_or_else(|| name),
        description: fm.get("description").cloned().unwrap_or_default(),
        instructions: body,
        model: fm.get("model").cloned(),
        is_directory: has_dir,
        refs,
    }))
}

/// Read agent description from ~/.claude/agents/{name}.md frontmatter
#[tauri::command]
pub fn read_agent_description(name: String) -> Result<Option<String>, String> {
    let dir = claude_agents_dir().ok_or("No home directory")?;
    let path = dir.join(format!("{}.md", name));
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let (fm, _) = parse_frontmatter(&content);
    Ok(fm.get("description").cloned())
}

#[tauri::command]
pub fn write_claude_agent(
    name: String,
    description: String,
    instructions: String,
    model: Option<String>,
) -> Result<(), String> {
    let dir = claude_agents_dir().ok_or("No home directory")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let mut content = String::from("---\n");
    content.push_str(&format!("name: {}\n", name));
    content.push_str(&format!("description: {}\n", description));
    if let Some(ref m) = model {
        content.push_str(&format!("model: {}\n", m));
    }
    content.push_str("---\n\n");
    content.push_str(&instructions);
    content.push('\n');

    // Always write to {name}.md (the agent definition file)
    let path = dir.join(format!("{}.md", name));
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_claude_agent(name: String) -> Result<(), String> {
    let dir = claude_agents_dir().ok_or("No home directory")?;
    // Delete the .md definition file
    let path = dir.join(format!("{}.md", name));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    // Also delete companion directory (refs) if exists
    let agent_dir = dir.join(&name);
    if agent_dir.is_dir() {
        std::fs::remove_dir_all(&agent_dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Agent reference file management ──

#[tauri::command]
pub fn list_agent_refs(name: String) -> Result<Vec<AgentFileInfo>, String> {
    let dir = claude_agents_dir().ok_or("No home directory")?;
    let agent_dir = dir.join(&name);
    Ok(list_agent_ref_files(&agent_dir))
}

#[tauri::command]
pub fn read_agent_ref(name: String, filename: String) -> Result<String, String> {
    let dir = claude_agents_dir().ok_or("No home directory")?;
    let agent_dir = dir.join(&name);
    // Find in refs/ or references/ subdirectory
    let refs_dir = agent_refs_dir(&agent_dir)
        .ok_or_else(|| format!("No refs directory for agent '{}'", name))?;
    let path = refs_dir.join(&filename);
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn write_agent_ref(name: String, filename: String, content: String) -> Result<(), String> {
    let dir = claude_agents_dir().ok_or("No home directory")?;
    let agent_dir = dir.join(&name);
    // Use existing refs dir, or create refs/ by default
    let refs_dir = agent_refs_dir(&agent_dir)
        .unwrap_or_else(|| agent_dir.join("refs"));
    std::fs::create_dir_all(&refs_dir).map_err(|e| e.to_string())?;
    std::fs::write(refs_dir.join(&filename), &content).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_agent_ref(name: String, filename: String) -> Result<(), String> {
    let dir = claude_agents_dir().ok_or("No home directory")?;
    let agent_dir = dir.join(&name);
    let refs_dir = agent_refs_dir(&agent_dir)
        .ok_or_else(|| format!("No refs directory for agent '{}'", name))?;
    let path = refs_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Snapshot management ──

#[derive(Serialize)]
pub struct AgentSnapshotInfo {
    pub filename: String,
    pub timestamp: u64,
    pub size: u64,
}

fn agent_snapshots_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("studio-snapshots").join("agents"))
}

#[tauri::command]
pub fn snapshot_agent(name: String) -> Result<String, String> {
    let dir = claude_agents_dir().ok_or("No home directory")?;
    let path = dir.join(format!("{}.md", name));
    if !path.exists() {
        return Err(format!("Agent '{}' not found", name));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;

    let snap_dir = agent_snapshots_dir().ok_or("No home directory")?;
    let item_dir = snap_dir.join(&name);
    std::fs::create_dir_all(&item_dir).map_err(|e| e.to_string())?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let filename = format!("{}.md", ts);
    let snap_path = item_dir.join(&filename);
    std::fs::write(&snap_path, &content).map_err(|e| e.to_string())?;

    Ok(snap_path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn list_agent_snapshots(name: String) -> Result<Vec<AgentSnapshotInfo>, String> {
    let snap_dir = agent_snapshots_dir().ok_or("No home directory")?;
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
            snapshots.push(AgentSnapshotInfo { filename, timestamp, size });
        }
    }
    snapshots.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(snapshots)
}

// ── DB-based agents (legacy, kept for compatibility) ──

#[tauri::command]
pub fn agents_list(state: State<'_, Arc<AppState>>) -> Result<Vec<Agent>, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    agents::list_agents(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn agents_get(id: String, state: State<'_, Arc<AppState>>) -> Result<Option<Agent>, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    Ok(agents::get_agent(&conn, &id))
}

#[tauri::command]
pub fn agents_create(
    name: String,
    description: String,
    instructions: String,
    model: Option<String>,
    allowed_tools: Option<Vec<String>>,
    max_turns: Option<i64>,
    state: State<'_, Arc<AppState>>,
) -> Result<Agent, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    agents::create_agent(
        &conn, &name, &description, &instructions,
        model.as_deref(), allowed_tools, max_turns.unwrap_or(10),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn agents_update(
    id: String,
    name: String,
    description: String,
    instructions: String,
    model: Option<String>,
    allowed_tools: Option<Vec<String>>,
    max_turns: Option<i64>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    agents::update_agent(
        &conn, &id, &name, &description, &instructions,
        model.as_deref(), allowed_tools, max_turns.unwrap_or(10),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn agents_delete(id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    agents::delete_agent(&conn, &id);
    Ok(())
}
