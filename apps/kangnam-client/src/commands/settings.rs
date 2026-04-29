use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub theme: String,
    #[serde(rename = "defaultProvider")]
    pub default_provider: String,
    #[serde(rename = "fontSize")]
    pub font_size: u32,
    #[serde(rename = "sendOnEnter")]
    pub send_on_enter: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            default_provider: "codex".to_string(),
            font_size: 14,
            send_on_enter: true,
        }
    }
}

fn settings_path(_state: &AppState) -> PathBuf {
    #[cfg(target_os = "macos")]
    let base = dirs::config_dir().unwrap_or_default().join("kangnam-client");
    #[cfg(target_os = "windows")]
    let base = dirs::data_dir().unwrap_or_default().join("kangnam-client");
    #[cfg(target_os = "linux")]
    let base = dirs::config_dir().unwrap_or_default().join("kangnam-client");
    base.join("settings.json")
}

fn load_settings(state: &AppState) -> Settings {
    let path = settings_path(state);
    match fs::read_to_string(&path) {
        Ok(raw) => {
            let defaults = Settings::default();
            let mut merged: serde_json::Value =
                serde_json::to_value(&defaults).unwrap_or_default();
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let (Some(base), Some(overlay)) = (merged.as_object_mut(), parsed.as_object()) {
                    for (k, v) in overlay {
                        base.insert(k.clone(), v.clone());
                    }
                }
            }
            serde_json::from_value(merged).unwrap_or_default()
        }
        Err(_) => Settings::default(),
    }
}

fn save_settings(state: &AppState, settings: &Settings) -> Result<(), String> {
    let path = settings_path(state);
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn settings_get(state: State<'_, Arc<AppState>>) -> Result<Settings, String> {
    Ok(load_settings(&state))
}

#[tauri::command]
pub fn settings_set(
    state: State<'_, Arc<AppState>>,
    partial: serde_json::Value,
) -> Result<Settings, String> {
    let current = load_settings(&state);
    let mut current_val = serde_json::to_value(&current).map_err(|e| e.to_string())?;

    if let (Some(base), Some(overlay)) = (current_val.as_object_mut(), partial.as_object()) {
        for (k, v) in overlay {
            base.insert(k.clone(), v.clone());
        }
    }

    let updated: Settings = serde_json::from_value(current_val).map_err(|e| e.to_string())?;
    save_settings(&state, &updated)?;
    Ok(updated)
}
