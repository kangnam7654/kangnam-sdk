use std::sync::Arc;
use tauri::{AppHandle, State};

use crate::skills::ai;
use crate::state::AppState;

#[tauri::command]
pub async fn prompts_ai_generate(
    user_request: String,
    provider: String,
    model: Option<String>,
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    let result =
        ai::generate_skill(&user_request, &provider, model.as_deref(), &state, &app).await?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn prompts_ai_improve(
    current_skill: serde_json::Value,
    feedback: String,
    provider: String,
    model: Option<String>,
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    let result =
        ai::improve_skill(&current_skill, &feedback, &provider, model.as_deref(), &state, &app)
            .await?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn prompts_ai_generate_ref(
    skill_instructions: String,
    user_request: String,
    provider: String,
    model: Option<String>,
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    let result = ai::generate_reference(
        &skill_instructions,
        &user_request,
        &provider,
        model.as_deref(),
        &state,
        &app,
    )
    .await?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn prompts_ai_generate_evals(
    skill: serde_json::Value,
    provider: String,
    model: Option<String>,
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    let result =
        ai::generate_evals(&skill, &provider, model.as_deref(), &state, &app).await?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn prompts_ai_grade(
    skill: serde_json::Value,
    criteria: Vec<String>,
    provider: String,
    model: Option<String>,
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    ai::grade_skill(&skill, &criteria, &provider, model.as_deref(), &state, &app).await
}

#[tauri::command]
pub async fn prompts_ai_compare(
    skill_a: serde_json::Value,
    skill_b: serde_json::Value,
    provider: String,
    model: Option<String>,
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    ai::compare_skills(&skill_a, &skill_b, &provider, model.as_deref(), &state, &app).await
}

#[tauri::command]
pub async fn prompts_ai_analyze(
    comparison_result: serde_json::Value,
    winner_skill: serde_json::Value,
    loser_skill: serde_json::Value,
    provider: String,
    model: Option<String>,
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    ai::analyze_comparison(
        &comparison_result,
        &winner_skill,
        &loser_skill,
        &provider,
        model.as_deref(),
        &state,
        &app,
    )
    .await
}
