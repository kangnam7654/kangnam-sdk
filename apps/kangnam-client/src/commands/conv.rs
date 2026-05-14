use std::sync::Arc;
use tauri::State;

use crate::db::conversations::{self, Conversation, Message, SearchResult};
use crate::state::AppState;
use kangnam_chat::core::export::{ExportFormat, export_conversation};

#[tauri::command]
pub fn conv_list(state: State<'_, Arc<AppState>>) -> Result<Vec<Conversation>, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    conversations::list_conversations(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn conv_create(
    provider: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Conversation, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    conversations::create_conversation(&conn, &provider, None).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn conv_delete(id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    conversations::delete_conversation(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn conv_get_messages(
    id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<Message>, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    conversations::get_messages(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn conv_update_title(
    id: String,
    title: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    conversations::update_title(&conn, &id, &title).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn conv_toggle_pin(id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    conversations::toggle_pin(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn conv_delete_all(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    conversations::delete_all_conversations(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn conv_export(
    id: String,
    format: String,
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let fmt = ExportFormat::parse(&format).ok_or_else(|| format!("Unknown format: {format}"))?;
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    export_conversation(&conn, &id, fmt)
}

#[tauri::command]
pub fn conv_search(
    query: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<SearchResult>, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    conversations::search_messages(&conn, &query).map_err(|e| e.to_string())
}
