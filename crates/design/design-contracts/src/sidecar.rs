//! Sidecar IPC protocol — daemon ⇆ desktop ⇆ web wire shapes plus the
//! `--od-stamp-*` CLI-arg stamp + IPC environment variable contract.
//! Mirrors `@open-design/packages/sidecar-proto/src/index.ts`.
//!
//! This module is wire-compatible with the upstream JSON shapes and the
//! `OPEN_DESIGN_SIDECAR_CONTRACT` static. The Node-specific spawn /
//! IPC implementation living in `@open-design/sidecar` is intentionally
//! *not* ported — Rust consumers should bring their own IPC plumbing
//! and use this module purely for the boundary types.

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── App / mode / source enums ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum AppKey {
    Daemon,
    Desktop,
    Web,
}

impl AppKey {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Daemon => "daemon",
            Self::Desktop => "desktop",
            Self::Web => "web",
        }
    }
}

pub const APP_KEYS: &[AppKey] = &[AppKey::Daemon, AppKey::Desktop, AppKey::Web];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum SidecarMode {
    Dev,
    Runtime,
}

impl SidecarMode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Runtime => "runtime",
        }
    }
}

pub const SIDECAR_MODES: &[SidecarMode] = &[SidecarMode::Dev, SidecarMode::Runtime];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum SidecarSource {
    Packaged,
    ToolsDev,
    ToolsPack,
}

impl SidecarSource {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Packaged => "packaged",
            Self::ToolsDev => "tools-dev",
            Self::ToolsPack => "tools-pack",
        }
    }
}

pub const SIDECAR_SOURCES: &[SidecarSource] = &[
    SidecarSource::Packaged,
    SidecarSource::ToolsDev,
    SidecarSource::ToolsPack,
];

// ─── Constants — env vars, CLI flags, defaults, message tags ─────────────

/// Environment-variable names the sidecar protocol consumes/exposes.
/// Mirror of the upstream `SIDECAR_ENV` / `SIDECAR_RUNTIME_ENV` pair.
pub mod env {
    pub const BASE: &str = "OD_SIDECAR_BASE";
    pub const DAEMON_PORT: &str = "OD_PORT";
    pub const IPC_BASE: &str = "OD_SIDECAR_IPC_BASE";
    pub const IPC_PATH: &str = "OD_SIDECAR_IPC_PATH";
    pub const NAMESPACE: &str = "OD_SIDECAR_NAMESPACE";
    pub const SOURCE: &str = "OD_SIDECAR_SOURCE";
    pub const TOOLS_DEV_PARENT_PID: &str = "OD_TOOLS_DEV_PARENT_PID";
    pub const WEB_DIST_DIR: &str = "OD_WEB_DIST_DIR";
    pub const WEB_PORT: &str = "OD_WEB_PORT";
    pub const WEB_TSCONFIG_PATH: &str = "OD_WEB_TSCONFIG_PATH";
}

/// CLI-arg flags forwarded to a spawned sidecar process to stamp it
/// with its app/mode/namespace identity.
pub mod stamp_flags {
    pub const APP: &str = "--od-stamp-app";
    pub const IPC: &str = "--od-stamp-ipc";
    pub const MODE: &str = "--od-stamp-mode";
    pub const NAMESPACE: &str = "--od-stamp-namespace";
    pub const SOURCE: &str = "--od-stamp-source";
}

pub const SIDECAR_STAMP_FIELDS: &[&str] = &["app", "mode", "namespace", "ipc", "source"];

/// Defaults the sidecar runtime falls back to when env / config don't
/// override them.
pub mod defaults {
    pub const HOST: &str = "127.0.0.1";
    pub const IPC_BASE: &str = "/tmp/open-design/ipc";
    pub const NAMESPACE: &str = "default";
    pub const PROJECT_TMP_DIR_NAME: &str = ".tmp";
    pub const WINDOWS_PIPE_PREFIX: &str = "open-design";
}

/// Message-type tags carried on `SidecarMessage.type`.
pub mod messages {
    pub const CLICK: &str = "click";
    pub const CONSOLE: &str = "console";
    pub const EVAL: &str = "eval";
    pub const SCREENSHOT: &str = "screenshot";
    pub const SHUTDOWN: &str = "shutdown";
    pub const STATUS: &str = "status";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Error)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum SidecarErrorCode {
    #[serde(rename = "SIDECAR_INVALID_MESSAGE")]
    #[error("SIDECAR_INVALID_MESSAGE")]
    InvalidMessage,
    #[serde(rename = "SIDECAR_UNKNOWN_MESSAGE")]
    #[error("SIDECAR_UNKNOWN_MESSAGE")]
    UnknownMessage,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("{code}: {message}")]
pub struct SidecarContractError {
    pub code: SidecarErrorCode,
    pub message: String,
}

impl SidecarContractError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: SidecarErrorCode::InvalidMessage,
            message: message.into(),
        }
    }

    pub fn unknown(message: impl Into<String>) -> Self {
        Self {
            code: SidecarErrorCode::UnknownMessage,
            message: message.into(),
        }
    }
}

// ─── Stamp shape ────────────────────────────────────────────────────────

/// `--od-stamp-*` CLI arg payload — what every spawned sidecar process
/// sees about itself. Validated via [`SidecarStamp::validate`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SidecarStamp {
    pub app: AppKey,
    pub ipc: String,
    pub mode: SidecarMode,
    pub namespace: String,
    pub source: SidecarSource,
}

impl SidecarStamp {
    /// Returns `Err` when the IPC path or namespace don't pass the
    /// upstream `normalizeIpcPath` / `normalizeNamespace` checks.
    pub fn validate(&self) -> Result<(), SidecarContractError> {
        normalize_ipc_path(&self.ipc)?;
        normalize_namespace(&self.namespace)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SidecarStampCriteria {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<AppKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ipc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<SidecarMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SidecarSource>,
}

// ─── Service / desktop runtime state ────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ServiceRuntimeState {
    Idle,
    Running,
    Starting,
    Stopped,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum DesktopRuntimeState {
    Idle,
    Running,
    Unknown,
}

// ─── Status snapshots ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DaemonStatusSnapshot {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub pid: Option<Option<u32>>,
    pub state: ServiceRuntimeState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    pub url: Option<String>,
}

/// Same shape as [`DaemonStatusSnapshot`] — alias to mirror the upstream
/// `WebStatusSnapshot` type.
pub type WebStatusSnapshot = DaemonStatusSnapshot;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DesktopStatusSnapshot {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub pid: Option<Option<u32>>,
    pub state: DesktopRuntimeState,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_helpers::double_option"
    )]
    pub title: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_visible: Option<bool>,
}

// ─── Desktop input / result shapes ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopEvalInput {
    pub expression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DesktopEvalResult {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopScreenshotInput {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopScreenshotResult {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopConsoleEntry {
    pub level: String,
    pub text: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopConsoleResult {
    pub entries: Vec<DesktopConsoleEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopClickInput {
    pub selector: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopClickResult {
    pub clicked: bool,
    pub found: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShutdownResult {
    /// Locked to `true` upstream.
    pub accepted: ShutdownAccepted,
}

crate::locked_true!(
    /// `accepted: true` literal on `ShutdownResult` — see [`crate::locked_true`].
    pub struct ShutdownAccepted;,
    field_name = "accepted"
);

// ─── Message envelopes ──────────────────────────────────────────────────

/// Shared `{ type: "status" }` and `{ type: "shutdown" }` no-payload
/// messages, accepted by every sidecar app.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
#[non_exhaustive]
pub enum SharedSidecarMessage {
    Status,
    Shutdown,
}

/// Daemon-app sidecar accepts only the shared messages.
pub type DaemonSidecarMessage = SharedSidecarMessage;

/// Web-app sidecar accepts only the shared messages.
pub type WebSidecarMessage = SharedSidecarMessage;

/// Desktop-app sidecar accepts shared messages plus eval / screenshot /
/// console / click introspection messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
#[non_exhaustive]
pub enum DesktopSidecarMessage {
    Status,
    Shutdown,
    Console,
    Eval { input: DesktopEvalInput },
    Screenshot { input: DesktopScreenshotInput },
    Click { input: DesktopClickInput },
}

// ─── Validators ─────────────────────────────────────────────────────────

/// Mirrors `normalizeNamespace(value)` upstream — alphanumeric with
/// `._-`, max 128 chars, no path separators, no leading/trailing
/// whitespace.
pub fn normalize_namespace(value: &str) -> Result<String, SidecarContractError> {
    if value.trim() != value {
        return Err(SidecarContractError::invalid(
            "namespace must not contain leading or trailing whitespace",
        ));
    }
    if value.is_empty() {
        return Err(SidecarContractError::invalid("namespace must not be empty"));
    }
    if value.contains('/') || value.contains('\\') {
        return Err(SidecarContractError::invalid(format!(
            "namespace must not contain path separators: {value}"
        )));
    }
    if value.len() > 128 {
        return Err(SidecarContractError::invalid(format!(
            "namespace too long: {value}"
        )));
    }
    let mut chars = value.chars();
    let first = chars
        .next()
        .ok_or_else(|| SidecarContractError::invalid("namespace must not be empty"))?;
    if !first.is_ascii_alphanumeric() {
        return Err(SidecarContractError::invalid(format!(
            "namespace contains unsupported characters: {value}"
        )));
    }
    for c in chars {
        if !(c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-') {
            return Err(SidecarContractError::invalid(format!(
                "namespace contains unsupported characters: {value}"
            )));
        }
    }
    Ok(value.to_string())
}

/// True for Windows named-pipe paths beginning with `\\.\pipe\`.
pub fn is_windows_named_pipe_path(value: &str) -> bool {
    value.starts_with("\\\\.\\pipe\\")
}

/// Mirrors `normalizeIpcPath(ipc)` upstream — must be absolute (Unix
/// `/...`, Windows drive letter, or named pipe), no whitespace, no
/// null bytes.
pub fn normalize_ipc_path(value: &str) -> Result<String, SidecarContractError> {
    if value.is_empty() {
        return Err(SidecarContractError::invalid(
            "sidecar ipc path must not be empty",
        ));
    }
    if value.trim() != value {
        return Err(SidecarContractError::invalid(
            "sidecar ipc path must not contain leading or trailing whitespace",
        ));
    }
    if value.contains('\0') {
        return Err(SidecarContractError::invalid(
            "sidecar ipc path must not contain null bytes",
        ));
    }
    if is_windows_named_pipe_path(value) {
        return Ok(value.to_string());
    }
    let mut chars = value.chars();
    let first = chars.next();
    if first == Some('/') {
        return Ok(value.to_string());
    }
    // Windows drive letter "C:\..." or "C:/...".
    if let Some(c) = first {
        if c.is_ascii_alphabetic() {
            let second = chars.next();
            let third = chars.next();
            if second == Some(':') && (third == Some('\\') || third == Some('/')) {
                return Ok(value.to_string());
            }
        }
    }
    Err(SidecarContractError::invalid(format!(
        "sidecar ipc path must be absolute: {value}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_key_lowercase_round_trip() {
        for (slug, key) in [
            ("daemon", AppKey::Daemon),
            ("desktop", AppKey::Desktop),
            ("web", AppKey::Web),
        ] {
            let q = format!("\"{slug}\"");
            let parsed: AppKey = serde_json::from_str(&q).unwrap();
            assert_eq!(parsed, key);
            assert_eq!(key.as_str(), slug);
        }
    }

    #[test]
    fn sidecar_source_kebab_case() {
        assert_eq!(
            serde_json::to_string(&SidecarSource::ToolsDev).unwrap(),
            "\"tools-dev\""
        );
        let s: SidecarSource = serde_json::from_str("\"tools-pack\"").unwrap();
        assert_eq!(s, SidecarSource::ToolsPack);
    }

    #[test]
    fn sidecar_mode_lowercase() {
        let m: SidecarMode = serde_json::from_str("\"runtime\"").unwrap();
        assert_eq!(m, SidecarMode::Runtime);
    }

    #[test]
    fn env_constants_match_upstream() {
        assert_eq!(env::BASE, "OD_SIDECAR_BASE");
        assert_eq!(env::DAEMON_PORT, "OD_PORT");
        assert_eq!(env::IPC_PATH, "OD_SIDECAR_IPC_PATH");
        assert_eq!(env::WEB_TSCONFIG_PATH, "OD_WEB_TSCONFIG_PATH");
    }

    #[test]
    fn stamp_flag_constants_match_upstream() {
        assert_eq!(stamp_flags::APP, "--od-stamp-app");
        assert_eq!(stamp_flags::IPC, "--od-stamp-ipc");
        assert_eq!(stamp_flags::SOURCE, "--od-stamp-source");
    }

    #[test]
    fn defaults_match_upstream() {
        assert_eq!(defaults::HOST, "127.0.0.1");
        assert_eq!(defaults::IPC_BASE, "/tmp/open-design/ipc");
        assert_eq!(defaults::NAMESPACE, "default");
    }

    #[test]
    fn message_constants_match_upstream() {
        assert_eq!(messages::CLICK, "click");
        assert_eq!(messages::CONSOLE, "console");
        assert_eq!(messages::EVAL, "eval");
        assert_eq!(messages::SCREENSHOT, "screenshot");
        assert_eq!(messages::SHUTDOWN, "shutdown");
        assert_eq!(messages::STATUS, "status");
    }

    #[test]
    fn error_code_screaming_snake() {
        assert_eq!(
            serde_json::to_string(&SidecarErrorCode::InvalidMessage).unwrap(),
            "\"SIDECAR_INVALID_MESSAGE\""
        );
        let e: SidecarErrorCode = serde_json::from_str("\"SIDECAR_UNKNOWN_MESSAGE\"").unwrap();
        assert_eq!(e, SidecarErrorCode::UnknownMessage);
    }

    #[test]
    fn stamp_round_trip_and_validate() {
        let s = SidecarStamp {
            app: AppKey::Daemon,
            ipc: "/tmp/open-design/ipc/default.sock".into(),
            mode: SidecarMode::Runtime,
            namespace: "default".into(),
            source: SidecarSource::Packaged,
        };
        assert!(s.validate().is_ok());
        let j = serde_json::to_string(&s).unwrap();
        assert!(j.contains("\"app\":\"daemon\""));
        assert!(j.contains("\"mode\":\"runtime\""));
        assert!(j.contains("\"source\":\"packaged\""));
        let back: SidecarStamp = serde_json::from_str(&j).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn stamp_validate_rejects_relative_ipc() {
        let s = SidecarStamp {
            app: AppKey::Web,
            ipc: "relative/path".into(),
            mode: SidecarMode::Dev,
            namespace: "default".into(),
            source: SidecarSource::ToolsDev,
        };
        let err = s.validate().unwrap_err();
        assert_eq!(err.code, SidecarErrorCode::InvalidMessage);
    }

    #[test]
    fn normalize_namespace_accepts_default() {
        assert_eq!(normalize_namespace("default").unwrap(), "default");
        assert_eq!(
            normalize_namespace("acme.dev_proj-1").unwrap(),
            "acme.dev_proj-1"
        );
    }

    #[test]
    fn normalize_namespace_rejects_invalid() {
        assert!(normalize_namespace("").is_err());
        assert!(normalize_namespace(" leading").is_err());
        assert!(normalize_namespace("trailing ").is_err());
        assert!(normalize_namespace("a/b").is_err());
        assert!(normalize_namespace("a\\b").is_err());
        assert!(normalize_namespace("-leading-dash").is_err());
        assert!(normalize_namespace("name!").is_err());
    }

    #[test]
    fn normalize_ipc_path_accepts_unix_and_windows() {
        assert!(normalize_ipc_path("/tmp/x.sock").is_ok());
        assert!(normalize_ipc_path("C:\\foo\\bar").is_ok());
        assert!(normalize_ipc_path("C:/foo/bar").is_ok());
        assert!(normalize_ipc_path("\\\\.\\pipe\\open-design.default").is_ok());
    }

    #[test]
    fn normalize_ipc_path_rejects_relative_and_bad_chars() {
        assert!(normalize_ipc_path("").is_err());
        assert!(normalize_ipc_path("relative/path").is_err());
        assert!(normalize_ipc_path(" /leading-space").is_err());
        assert!(normalize_ipc_path("/has\0null").is_err());
    }

    #[test]
    fn shared_sidecar_message_round_trip() {
        let m = SharedSidecarMessage::Status;
        let s = serde_json::to_string(&m).unwrap();
        assert_eq!(s, r#"{"type":"status"}"#);
        let m2: SharedSidecarMessage = serde_json::from_str(r#"{"type":"shutdown"}"#).unwrap();
        assert_eq!(m2, SharedSidecarMessage::Shutdown);
    }

    #[test]
    fn desktop_sidecar_message_with_payloads() {
        let m = DesktopSidecarMessage::Eval {
            input: DesktopEvalInput {
                expression: "1+1".into(),
            },
        };
        let s = serde_json::to_string(&m).unwrap();
        assert!(s.contains("\"type\":\"eval\""));
        assert!(s.contains("\"expression\":\"1+1\""));
        let back: DesktopSidecarMessage = serde_json::from_str(&s).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn desktop_sidecar_message_payload_less_console() {
        let m = DesktopSidecarMessage::Console;
        let s = serde_json::to_string(&m).unwrap();
        assert_eq!(s, r#"{"type":"console"}"#);
    }

    #[test]
    fn shutdown_result_locked_to_true() {
        let r = ShutdownResult {
            accepted: ShutdownAccepted,
        };
        let s = serde_json::to_string(&r).unwrap();
        assert_eq!(s, r#"{"accepted":true}"#);
        let bad = r#"{"accepted":false}"#;
        let err = serde_json::from_str::<ShutdownResult>(bad).unwrap_err();
        assert!(err.to_string().contains("accepted must be true"));
    }

    #[test]
    fn daemon_status_snapshot_camel_case_with_pid_null() {
        let s = DaemonStatusSnapshot {
            pid: Some(None),
            state: ServiceRuntimeState::Stopped,
            updated_at: Some("2026-05-06T00:00:00Z".into()),
            url: None,
        };
        let j = serde_json::to_string(&s).unwrap();
        assert!(j.contains("\"pid\":null"));
        assert!(j.contains("\"state\":\"stopped\""));
        assert!(j.contains("\"updatedAt\""));
        assert!(j.contains("\"url\":null"));
    }

    #[test]
    fn desktop_status_snapshot_window_visible_round_trip() {
        let s = DesktopStatusSnapshot {
            pid: Some(Some(1234)),
            state: DesktopRuntimeState::Running,
            title: Some(Some("My Window".into())),
            updated_at: None,
            url: Some("https://localhost".into()),
            window_visible: Some(true),
        };
        let j = serde_json::to_string(&s).unwrap();
        assert!(j.contains("\"pid\":1234"));
        assert!(j.contains("\"state\":\"running\""));
        assert!(j.contains("\"title\":\"My Window\""));
        assert!(j.contains("\"windowVisible\":true"));
    }

    #[test]
    fn desktop_eval_result_round_trip() {
        let r = DesktopEvalResult {
            ok: true,
            error: None,
            value: Some(serde_json::json!(2)),
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"ok\":true"));
        assert!(s.contains("\"value\":2"));
        assert!(!s.contains("error"));
    }

    #[test]
    fn stamp_fields_const_matches_upstream() {
        assert_eq!(
            SIDECAR_STAMP_FIELDS,
            &["app", "mode", "namespace", "ipc", "source"]
        );
    }

    #[test]
    fn app_keys_const_lists_three() {
        assert_eq!(APP_KEYS.len(), 3);
        assert_eq!(SIDECAR_MODES.len(), 2);
        assert_eq!(SIDECAR_SOURCES.len(), 3);
    }
}
