//! Tauri commands wrapping `design-artifact::export` (Phase 5c-17).
//!
//! Renderer chips invoke these to materialize an artifact body into
//! HTML / Markdown bytes ready for download. PDF lives entirely in
//! the webview side (`window.print()` on the iframe → OS dialog).
//! ZIP / multi-file bundle ships in Phase 6a.

#[tauri::command]
pub fn artifact_export_html(body: String) -> Result<String, String> {
    Ok(design_artifact::export_html(&body))
}

#[tauri::command]
pub fn artifact_export_markdown(body: String) -> Result<String, String> {
    Ok(design_artifact::export_markdown(&body))
}
