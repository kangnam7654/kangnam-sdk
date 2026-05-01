//! Tauri commands wrapping `design-artifact::export`.
//!
//! Phase 5c-17: HTML pass-through + Markdown.
//! Phase 6a-02: HTML asset inlining + ZIP archive.
//!
//! PDF lives entirely in the webview side (`window.print()` on the
//! iframe → OS dialog).

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[tauri::command]
pub fn artifact_export_html(body: String) -> Result<String, String> {
    Ok(design_artifact::export_html(&body))
}

#[tauri::command]
pub fn artifact_export_markdown(body: String) -> Result<String, String> {
    Ok(design_artifact::export_markdown(&body))
}

/// One asset to inline / archive — frontend supplies bytes as a base64
/// string so the JSON-RPC bridge can carry binary payloads.
#[derive(Deserialize)]
pub struct InputAsset {
    pub path: String,
    pub mime: String,
    /// Base64-encoded raw bytes.
    pub bytes_b64: String,
}

fn decode_assets(input: Vec<InputAsset>) -> Result<Vec<design_artifact::Asset>, String> {
    use base64::Engine as _;
    let mut out = Vec::with_capacity(input.len());
    for a in input {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&a.bytes_b64)
            .map_err(|e| format!("base64 decode {}: {}", a.path, e))?;
        out.push(design_artifact::Asset {
            path: a.path,
            mime: a.mime,
            bytes,
        });
    }
    Ok(out)
}

#[tauri::command]
pub fn artifact_export_html_inline(
    body: String,
    assets: Vec<InputAsset>,
) -> Result<String, String> {
    let decoded = decode_assets(assets)?;
    Ok(design_artifact::export_html_inline(&body, &decoded))
}

/// Build a ZIP archive and return base64-encoded bytes the renderer
/// can drop into a Blob.
#[tauri::command]
pub fn artifact_export_zip(html: String, assets: Vec<InputAsset>) -> Result<String, String> {
    use base64::Engine as _;
    let decoded = decode_assets(assets)?;
    let bytes = design_artifact::export_zip(&html, &decoded)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

/// Helper that walks an artifact directory + reads file bytes for the
/// renderer to feed into the inline / zip exports without re-bundling
/// asset transport logic in TS. Restricted to `working_dir` (path
/// traversal guard).
#[tauri::command]
pub fn artifact_collect_assets(
    working_dir: String,
    rel_paths: Vec<String>,
) -> Result<Vec<InputAsset>, String> {
    use base64::Engine as _;
    let root = PathBuf::from(&working_dir);
    let canon_root = root
        .canonicalize()
        .map_err(|e| format!("canonicalize root: {e}"))?;
    let mut out = Vec::with_capacity(rel_paths.len());
    for rel in rel_paths {
        let path = root.join(&rel);
        let canon = path
            .canonicalize()
            .map_err(|e| format!("canonicalize {rel}: {e}"))?;
        if !canon.starts_with(&canon_root) {
            return Err(format!("path escapes working_dir: {rel}"));
        }
        let bytes = std::fs::read(&canon).map_err(|e| format!("read {rel}: {e}"))?;
        let mime = mime_from_path(&canon).to_string();
        out.push(InputAsset {
            path: rel,
            mime,
            bytes_b64: base64::engine::general_purpose::STANDARD.encode(&bytes),
        });
    }
    Ok(out)
}

fn mime_from_path(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "json" => "application/json",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "html" | "htm" => "text/html",
        _ => "application/octet-stream",
    }
}
