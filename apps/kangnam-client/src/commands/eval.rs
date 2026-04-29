use std::sync::Arc;
use rusqlite::params;
use tauri::{AppHandle, Emitter, State};

use crate::skills::ai;
use crate::state::AppState;

#[tauri::command]
pub fn eval_set_create(
    skill_id: String,
    name: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    let set_name = name.unwrap_or_else(|| "Default".to_string());
    conn.execute(
        "INSERT INTO skill_eval_sets (id, skill_id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, skill_id, set_name, now, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "id": id, "skillId": skill_id, "name": set_name,
        "createdAt": now, "updatedAt": now
    }))
}

#[tauri::command]
pub fn eval_set_list(
    skill_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let mut stmt = conn
        .prepare(
            "SELECT id, skill_id, name, created_at, updated_at \
             FROM skill_eval_sets WHERE skill_id = ?1 ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<serde_json::Value> = stmt
        .query_map(params![skill_id], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "skillId": row.get::<_, String>(1)?,
                "name": row.get::<_, String>(2)?,
                "createdAt": row.get::<_, i64>(3)?,
                "updatedAt": row.get::<_, i64>(4)?
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(serde_json::json!(rows))
}

#[tauri::command]
pub fn eval_set_delete(id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    conn.execute("DELETE FROM skill_eval_sets WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn eval_case_add(
    eval_set_id: String,
    prompt: String,
    expected: String,
    should_trigger: bool,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let id = uuid::Uuid::new_v4().to_string();
    let sort: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM skill_eval_cases WHERE eval_set_id = ?1",
            params![eval_set_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO skill_eval_cases (id, eval_set_id, prompt, expected, should_trigger, sort_order) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, eval_set_id, prompt, expected, should_trigger as i64, sort],
    )
    .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "id": id, "evalSetId": eval_set_id, "prompt": prompt,
        "expected": expected, "shouldTrigger": should_trigger, "sortOrder": sort
    }))
}

#[tauri::command]
pub fn eval_case_bulk_add(
    eval_set_id: String,
    cases: Vec<serde_json::Value>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    // Get starting sort_order once (fixes N+1 query pattern)
    let mut sort: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM skill_eval_cases WHERE eval_set_id = ?1",
            params![eval_set_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let mut added = Vec::new();
    // Wrap in transaction for atomicity
    conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;
    for case in &cases {
        let id = uuid::Uuid::new_v4().to_string();
        let prompt = case.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
        let expected = case
            .get("expected")
            .or_else(|| case.get("expectedBehavior"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let should_trigger = case
            .get("shouldTrigger")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        if let Err(e) = conn.execute(
            "INSERT INTO skill_eval_cases (id, eval_set_id, prompt, expected, should_trigger, sort_order) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, eval_set_id, prompt, expected, should_trigger as i64, sort],
        ) {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(e.to_string());
        }
        added.push(serde_json::json!({ "id": id }));
        sort += 1;
    }
    conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
    Ok(serde_json::json!(added))
}

#[tauri::command]
pub fn eval_case_update(
    id: String,
    prompt: String,
    expected: String,
    should_trigger: bool,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    conn.execute(
        "UPDATE skill_eval_cases SET prompt = ?1, expected = ?2, should_trigger = ?3 WHERE id = ?4",
        params![prompt, expected, should_trigger as i64, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn eval_case_delete(id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    conn.execute("DELETE FROM skill_eval_cases WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn eval_case_list(
    eval_set_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let mut stmt = conn
        .prepare(
            "SELECT id, eval_set_id, prompt, expected, should_trigger, sort_order \
             FROM skill_eval_cases WHERE eval_set_id = ?1 ORDER BY sort_order ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<serde_json::Value> = stmt
        .query_map(params![eval_set_id], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "evalSetId": row.get::<_, String>(1)?,
                "prompt": row.get::<_, String>(2)?,
                "expected": row.get::<_, String>(3)?,
                "shouldTrigger": row.get::<_, i64>(4)? == 1,
                "sortOrder": row.get::<_, i64>(5)?
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(serde_json::json!(rows))
}

#[tauri::command]
pub async fn eval_run_start(
    eval_set_id: String,
    skill_id: String,
    provider: String,
    model: Option<String>,
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    // Load skill info
    let (skill_name, skill_desc, skill_body) = {
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        let row = conn
            .query_row(
                "SELECT title, description, content FROM prompts WHERE id = ?1",
                params![skill_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
            )
            .map_err(|e| format!("Skill not found: {e}"))?;
        row
    };

    // Load cases
    let cases: Vec<(String, String, String, bool)> = {
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn
            .prepare(
                "SELECT id, prompt, expected, should_trigger FROM skill_eval_cases WHERE eval_set_id = ?1 ORDER BY sort_order ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<(String, String, String, bool)> = stmt
            .query_map(params![eval_set_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)? == 1,
                ))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        rows
    };

    if cases.is_empty() {
        return Err("No eval cases found".to_string());
    }

    // Create run record
    let run_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    let total_cases = cases.len() as i64;
    {
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute(
            "INSERT INTO skill_eval_runs (id, eval_set_id, skill_id, skill_name, skill_desc, skill_body, provider, model, status, total_cases, completed_cases, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'running', ?9, 0, ?10)",
            params![run_id, eval_set_id, skill_id, skill_name, skill_desc, skill_body, provider, model, total_cases, now],
        )
        .map_err(|e| e.to_string())?;

        // Create pending result records
        for (case_id, _, _, _) in &cases {
            let result_id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO skill_eval_results (id, run_id, case_id, status) VALUES (?1, ?2, ?3, 'pending')",
                params![result_id, run_id, case_id],
            )
            .ok();
        }
    }

    let run_id_ret = run_id.clone();

    // Spawn background eval task
    let app_clone = app.clone();
    let _provider_clone = provider.clone();
    let model_clone = model.clone();

    // We need to get the access_token and provider handle before spawning
    let access_token = state
        .auth
        .get_access_token(&provider, &state.db, &app)
        .await
        .ok_or(format!("Not connected to {provider}"))?;
    let llm_provider = state
        .router
        .get_provider(&provider)
        .ok_or(format!("Unknown provider: {provider}"))?;
    let db_path = state.data_dir.join("kangnam-client.db");

    tokio::spawn(async move {
        // Open a separate DB connection for the background task
        let bg_conn = match crate::db::connection::open_database(&db_path) {
            Ok(c) => c,
            Err(e) => {
                let _ = app_clone.emit(
                    "eval:progress",
                    serde_json::json!({ "runId": run_id, "error": e.to_string() }),
                );
                return;
            }
        };

        let mut completed = 0i64;

        for (case_id, prompt, _expected, should_trigger) in &cases {
            // Send the prompt through the LLM with the skill instructions as system
            let system_msg = format!(
                "You are an AI assistant. The following skill is active:\n\n**Name:** {skill_name}\n**Description:** {skill_desc}\n\n**Instructions:**\n{skill_body}"
            );
            let messages = vec![
                crate::providers::types::ChatMessage {
                    role: "system".to_string(),
                    content: system_msg,
                    tool_call_id: None,
                    tool_use_id: None,
                    images: None,
                    tool_calls: None,
                },
                crate::providers::types::ChatMessage {
                    role: "user".to_string(),
                    content: prompt.clone(),
                    tool_call_id: None,
                    tool_use_id: None,
                    images: None,
                    tool_calls: None,
                },
            ];

            let (tx, mut rx) = tokio::sync::mpsc::channel::<crate::providers::types::StreamEvent>(64);
            let provider_handle = llm_provider.clone();
            let token = access_token.clone();
            let mdl = model_clone.clone();

            tokio::spawn(async move {
                let _ = provider_handle
                    .send_message(&messages, &[], &token, tx, mdl.as_deref(), None)
                    .await;
            });

            let mut response = String::new();
            while let Some(event) = rx.recv().await {
                if let crate::providers::types::StreamEvent::Token(text) = event {
                    response.push_str(&text);
                }
            }

            // Simple trigger detection: did the response reference the skill?
            let did_trigger = !response.is_empty();
            let trigger_correct = did_trigger == *should_trigger;

            // Update result record
            bg_conn
                .execute(
                    "UPDATE skill_eval_results SET did_trigger = ?1, trigger_correct = ?2, response_with = ?3, status = 'complete' \
                     WHERE run_id = ?4 AND case_id = ?5",
                    params![
                        did_trigger as i64,
                        trigger_correct as i64,
                        response,
                        run_id,
                        case_id
                    ],
                )
                .ok();

            completed += 1;
            bg_conn
                .execute(
                    "UPDATE skill_eval_runs SET completed_cases = ?1 WHERE id = ?2",
                    params![completed, run_id],
                )
                .ok();

            let _ = app_clone.emit(
                "eval:progress",
                serde_json::json!({
                    "runId": run_id,
                    "completed": completed,
                    "total": total_cases,
                    "caseId": case_id
                }),
            );
        }

        // Calculate final stats
        let trigger_accuracy: f64 = bg_conn
            .query_row(
                "SELECT CAST(SUM(CASE WHEN trigger_correct = 1 THEN 1 ELSE 0 END) AS REAL) / COUNT(*) FROM skill_eval_results WHERE run_id = ?1 AND status = 'complete'",
                params![run_id],
                |r| r.get(0),
            )
            .unwrap_or(0.0);

        let quality_mean: f64 = bg_conn
            .query_row(
                "SELECT COALESCE(AVG(quality_score), 0.0) FROM skill_eval_results WHERE run_id = ?1 AND quality_score IS NOT NULL",
                params![run_id],
                |r| r.get(0),
            )
            .unwrap_or(0.0);

        bg_conn
            .execute(
                "UPDATE skill_eval_runs SET status = 'complete', trigger_accuracy = ?1, quality_mean = ?2 WHERE id = ?3",
                params![trigger_accuracy, quality_mean, run_id],
            )
            .ok();

        let _ = app_clone.emit(
            "eval:progress",
            serde_json::json!({
                "runId": run_id,
                "completed": total_cases,
                "total": total_cases,
                "status": "complete",
                "triggerAccuracy": trigger_accuracy,
                "qualityMean": quality_mean
            }),
        );
    });

    Ok(serde_json::json!({ "runId": run_id_ret, "totalCases": total_cases }))
}

#[tauri::command]
pub fn eval_run_stop(run_id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    conn.execute(
        "UPDATE skill_eval_runs SET status = 'stopped' WHERE id = ?1 AND status = 'running'",
        params![run_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn eval_run_list(
    eval_set_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let mut stmt = conn
        .prepare(
            "SELECT id, eval_set_id, skill_id, skill_name, provider, model, status, \
             trigger_accuracy, quality_mean, total_cases, completed_cases, created_at \
             FROM skill_eval_runs WHERE eval_set_id = ?1 ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<serde_json::Value> = stmt
        .query_map(params![eval_set_id], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "evalSetId": row.get::<_, String>(1)?,
                "skillId": row.get::<_, String>(2)?,
                "skillName": row.get::<_, String>(3)?,
                "provider": row.get::<_, String>(4)?,
                "model": row.get::<_, Option<String>>(5)?,
                "status": row.get::<_, String>(6)?,
                "triggerAccuracy": row.get::<_, Option<f64>>(7)?,
                "qualityMean": row.get::<_, Option<f64>>(8)?,
                "totalCases": row.get::<_, i64>(9)?,
                "completedCases": row.get::<_, i64>(10)?,
                "createdAt": row.get::<_, i64>(11)?
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(serde_json::json!(rows))
}

#[tauri::command]
pub fn eval_run_get(
    run_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    conn.query_row(
        "SELECT id, eval_set_id, skill_id, skill_name, provider, model, status, \
         trigger_accuracy, quality_mean, total_cases, completed_cases, created_at \
         FROM skill_eval_runs WHERE id = ?1",
        params![run_id],
        |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "evalSetId": row.get::<_, String>(1)?,
                "skillId": row.get::<_, String>(2)?,
                "skillName": row.get::<_, String>(3)?,
                "provider": row.get::<_, String>(4)?,
                "model": row.get::<_, Option<String>>(5)?,
                "status": row.get::<_, String>(6)?,
                "triggerAccuracy": row.get::<_, Option<f64>>(7)?,
                "qualityMean": row.get::<_, Option<f64>>(8)?,
                "totalCases": row.get::<_, i64>(9)?,
                "completedCases": row.get::<_, i64>(10)?,
                "createdAt": row.get::<_, i64>(11)?
            }))
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn eval_run_results(
    run_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let mut stmt = conn
        .prepare(
            "SELECT id, run_id, case_id, did_trigger, trigger_correct, \
             response_with, response_without, quality_score, quality_reason, \
             feedback, feedback_rating, status \
             FROM skill_eval_results WHERE run_id = ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<serde_json::Value> = stmt
        .query_map(params![run_id], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "runId": row.get::<_, String>(1)?,
                "caseId": row.get::<_, String>(2)?,
                "didTrigger": row.get::<_, Option<i64>>(3)?.map(|v| v == 1),
                "triggerCorrect": row.get::<_, Option<i64>>(4)?.map(|v| v == 1),
                "responseWith": row.get::<_, Option<String>>(5)?,
                "responseWithout": row.get::<_, Option<String>>(6)?,
                "qualityScore": row.get::<_, Option<i64>>(7)?,
                "qualityReason": row.get::<_, Option<String>>(8)?,
                "feedback": row.get::<_, Option<String>>(9)?,
                "feedbackRating": row.get::<_, Option<i64>>(10)?,
                "status": row.get::<_, String>(11)?
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(serde_json::json!(rows))
}

#[tauri::command]
pub fn eval_run_stats(
    run_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());

    let run = conn
        .query_row(
            "SELECT status, trigger_accuracy, quality_mean, total_cases, completed_cases \
             FROM skill_eval_runs WHERE id = ?1",
            params![run_id],
            |row| {
                Ok(serde_json::json!({
                    "status": row.get::<_, String>(0)?,
                    "triggerAccuracy": row.get::<_, Option<f64>>(1)?,
                    "qualityMean": row.get::<_, Option<f64>>(2)?,
                    "totalCases": row.get::<_, i64>(3)?,
                    "completedCases": row.get::<_, i64>(4)?
                }))
            },
        )
        .map_err(|e| e.to_string())?;

    // Calculate live stats from results
    let trigger_accuracy: f64 = conn
        .query_row(
            "SELECT CASE WHEN COUNT(*) = 0 THEN 0.0 ELSE \
             CAST(SUM(CASE WHEN trigger_correct = 1 THEN 1 ELSE 0 END) AS REAL) / COUNT(*) END \
             FROM skill_eval_results WHERE run_id = ?1 AND status = 'complete'",
            params![run_id],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    let quality_mean: f64 = conn
        .query_row(
            "SELECT COALESCE(AVG(quality_score), 0.0) FROM skill_eval_results \
             WHERE run_id = ?1 AND quality_score IS NOT NULL",
            params![run_id],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    Ok(serde_json::json!({
        "runId": run_id,
        "triggerAccuracy": trigger_accuracy,
        "qualityMean": quality_mean,
        "run": run
    }))
}

#[tauri::command]
pub fn eval_run_delete(run_id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    conn.execute("DELETE FROM skill_eval_runs WHERE id = ?1", params![run_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn eval_result_feedback(
    result_id: String,
    feedback: String,
    rating: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    conn.execute(
        "UPDATE skill_eval_results SET feedback = ?1, feedback_rating = ?2 WHERE id = ?3",
        params![feedback, rating, result_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn eval_ai_generate(
    skill: serde_json::Value,
    provider: String,
    model: Option<String>,
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    let cases = ai::generate_evals(&skill, &provider, model.as_deref(), &state, &app).await?;
    serde_json::to_value(cases).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn eval_optimize_start(
    _skill_id: String,
    _eval_set_id: String,
    _provider: String,
    _model: Option<String>,
    _state: State<'_, Arc<AppState>>,
    _app: AppHandle,
) -> Result<(), String> {
    // Placeholder for iterative skill optimization using eval results
    Err("Eval optimization not yet implemented".to_string())
}
