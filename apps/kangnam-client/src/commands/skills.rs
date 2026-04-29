use std::sync::Arc;
use tauri::State;

use crate::db::skills::{self, Skill, SkillReference};
use crate::state::AppState;

#[tauri::command]
pub fn prompts_list(state: State<'_, Arc<AppState>>) -> Result<Vec<Skill>, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    skills::list_skills(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn prompts_get(id: String, state: State<'_, Arc<AppState>>) -> Result<Option<Skill>, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    Ok(skills::get_skill(&conn, &id))
}

#[tauri::command]
pub fn prompts_get_instructions(
    id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<String>, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    Ok(skills::get_skill_instructions(&conn, &id))
}

#[tauri::command]
pub fn prompts_create(
    name: String,
    description: String,
    instructions: String,
    argument_hint: Option<String>,
    model: Option<String>,
    user_invocable: Option<bool>,
    state: State<'_, Arc<AppState>>,
) -> Result<Skill, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    skills::create_skill(
        &conn,
        &name,
        &description,
        &instructions,
        argument_hint.as_deref(),
        model.as_deref(),
        user_invocable,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn prompts_update(
    id: String,
    name: String,
    description: String,
    instructions: String,
    argument_hint: Option<String>,
    model: Option<String>,
    user_invocable: Option<bool>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    skills::update_skill(
        &conn,
        &id,
        &name,
        &description,
        &instructions,
        argument_hint.as_deref(),
        model.as_deref(),
        user_invocable,
    );
    Ok(())
}

#[tauri::command]
pub fn prompts_delete(id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    skills::delete_skill(&conn, &id);
    Ok(())
}

// ── Reference commands ──

#[tauri::command]
pub fn prompts_ref_list(
    skill_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<SkillReference>, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    Ok(skills::list_skill_references(&conn, &skill_id))
}

#[tauri::command]
pub fn prompts_ref_add(
    skill_id: String,
    name: String,
    content: String,
    state: State<'_, Arc<AppState>>,
) -> Result<SkillReference, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    skills::add_skill_reference(&conn, &skill_id, &name, &content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn prompts_ref_update(
    id: String,
    name: String,
    content: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    skills::update_skill_reference(&conn, &id, &name, &content);
    Ok(())
}

#[tauri::command]
pub fn prompts_ref_delete(id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    skills::delete_skill_reference(&conn, &id);
    Ok(())
}
