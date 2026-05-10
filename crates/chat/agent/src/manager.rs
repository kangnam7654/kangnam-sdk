#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Child;
use tokio::sync::Mutex;

use crate::adapter::CliAdapter;
use crate::sink::AgentEventSink;
use crate::types::{CliAuthStatus, CliStatus, UnifiedMessage};

struct CliSession {
    child: Child,
    provider: String,
    working_dir: PathBuf,
    session_id: String,
    model: Option<String>,
    reasoning_effort: Option<String>,
    /// Conversation history for non-persistent CLIs (Codex).
    /// Pairs of (user_message, assistant_response).
    history: Vec<(String, String)>,
    /// Buffer for accumulating current assistant response text.
    current_response: Arc<tokio::sync::Mutex<String>>,
}

pub struct CliManager {
    adapters: HashMap<String, Box<dyn CliAdapter>>,
    sessions: Arc<Mutex<HashMap<String, CliSession>>>,
}

struct AuthProbe {
    status: CliAuthStatus,
    message: Option<String>,
}

impl CliManager {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register_adapter(&mut self, adapter: Box<dyn CliAdapter>) {
        let name = adapter.name().to_string();
        self.adapters.insert(name, adapter);
    }

    fn get_adapter(&self, provider: &str) -> Result<&dyn CliAdapter, String> {
        self.adapters
            .get(provider)
            .map(|a| a.as_ref())
            .ok_or_else(|| format!("Unknown provider: {}", provider))
    }

    pub async fn start_session(
        &self,
        provider: &str,
        working_dir: &Path,
        session_id: &str,
        sink: Arc<dyn AgentEventSink>,
        model: Option<String>,
    ) -> Result<(), String> {
        let adapter = self.get_adapter(provider)?;
        let mut cmd = if provider == "claude" {
            let claude_adapter =
                crate::adapters::claude::ClaudeAdapter::new().with_model(model.clone());
            claude_adapter.build_command(working_dir)
        } else {
            adapter.build_command_with_options(working_dir, model.as_deref(), None)
        };
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn {}: {}", provider, e))?;

        let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
        let sessions = self.sessions.clone();
        let sid = session_id.to_string();
        let is_claude = provider == "claude";

        {
            let mut sessions_lock = sessions.lock().await;
            sessions_lock.insert(
                session_id.to_string(),
                CliSession {
                    child,
                    provider: provider.to_string(),
                    working_dir: working_dir.to_path_buf(),
                    session_id: session_id.to_string(),
                    model,
                    reasoning_effort: None,
                    history: Vec::new(),
                    current_response: Arc::new(tokio::sync::Mutex::new(String::new())),
                },
            );
        }

        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let parsed = if is_claude {
                    crate::adapters::claude::ClaudeAdapter::new().parse_line(&line)
                } else {
                    crate::adapters::codex::CodexAdapter::new().parse_line(&line)
                };

                match parsed {
                    Ok(Some(msg)) => {
                        // For non-error result messages with text, emit text before TurnEnd
                        if is_claude {
                            if let UnifiedMessage::TurnEnd { .. } = &msg {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                                    if v.get("type").and_then(|t| t.as_str()) == Some("result") {
                                        if !v
                                            .get("is_error")
                                            .and_then(|b| b.as_bool())
                                            .unwrap_or(false)
                                        {
                                            if let Some(text) =
                                                v.get("result").and_then(|t| t.as_str())
                                            {
                                                if !text.is_empty() {
                                                    sink.emit_message(UnifiedMessage::TextDelta {
                                                        text: text.to_string(),
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        sink.emit_message(msg);
                    }
                    Ok(None) => {}
                    Err(e) => {
                        sink.emit_message(UnifiedMessage::Error {
                            message: format!("Parse error: {}", e),
                        });
                    }
                }

                // Enhanced events (Claude only)
                if is_claude {
                    let enhanced =
                        crate::adapters::claude::ClaudeAdapter::new().parse_enhanced(&line);
                    if let Ok(Some(event)) = enhanced {
                        sink.emit_enhanced(event);
                    }
                }
            }

            sink.emit_message(UnifiedMessage::TurnEnd { usage: None });

            let mut sessions_lock = sessions.lock().await;
            sessions_lock.remove(&sid);
        });

        Ok(())
    }

    pub async fn send_message(
        &self,
        session_id: &str,
        message: &str,
        sink: Arc<dyn AgentEventSink>,
    ) -> Result<(), String> {
        let mut sessions_lock = self.sessions.lock().await;

        if let Some(session) = sessions_lock.get_mut(session_id) {
            let adapter = self.get_adapter(&session.provider.clone())?;

            if let Some(effort) = parse_effort_command(message) {
                session.reasoning_effort = Some(effort);
                return Ok(());
            }

            if adapter.supports_persistent_session() {
                if let Some(formatted) =
                    adapter.format_user_message(message, &session.session_id.clone())
                {
                    if let Some(stdin) = session.child.stdin.as_mut() {
                        stdin
                            .write_all(formatted.as_bytes())
                            .await
                            .map_err(|e| format!("stdin write error: {}", e))?;
                        stdin
                            .flush()
                            .await
                            .map_err(|e| format!("stdin flush error: {}", e))?;
                    }
                }
                Ok(())
            } else {
                // Non-persistent (Codex): build prompt with history context
                let provider = session.provider.clone();
                let working_dir = session.working_dir.clone();
                let model = session.model.clone();
                let reasoning_effort = session.reasoning_effort.clone();
                let history = session.history.clone();

                // Build contextual prompt from history
                let mut prompt = String::new();
                if !history.is_empty() {
                    prompt.push_str("Previous conversation:\n");
                    for (user_msg, assistant_msg) in &history {
                        prompt.push_str(&format!(
                            "User: {}\nAssistant: {}\n\n",
                            user_msg, assistant_msg
                        ));
                    }
                    prompt.push_str("Now respond to:\n");
                }
                prompt.push_str(message);

                let current_response = session.current_response.clone();
                let user_msg = message.to_string();
                drop(sessions_lock);

                self.start_codex_exec_with_history(
                    &provider,
                    &working_dir,
                    session_id,
                    &prompt,
                    &user_msg,
                    model.as_deref(),
                    reasoning_effort.as_deref(),
                    current_response,
                    sink,
                )
                .await
            }
        } else {
            Err(format!("Session not found: {}", session_id))
        }
    }

    async fn start_codex_exec_with_history(
        &self,
        provider: &str,
        working_dir: &Path,
        session_id: &str,
        prompt: &str,
        user_msg: &str,
        model: Option<&str>,
        reasoning_effort: Option<&str>,
        current_response: Arc<tokio::sync::Mutex<String>>,
        sink: Arc<dyn AgentEventSink>,
    ) -> Result<(), String> {
        let adapter = self.get_adapter(provider)?;
        let mut cmd = adapter.build_command_with_options(working_dir, model, reasoning_effort);
        cmd.arg(prompt);

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn codex: {}", e))?;
        let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
        let sessions = self.sessions.clone();
        let sid = session_id.to_string();
        let user_msg_owned = user_msg.to_string();
        let _response_collector = current_response.clone();

        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut response_text = String::new();

            while let Ok(Some(line)) = lines.next_line().await {
                let parsed = crate::adapters::codex::CodexAdapter::new().parse_line(&line);
                match parsed {
                    Ok(Some(ref msg)) => {
                        // Collect text for history
                        if let UnifiedMessage::TextDelta { text } = msg {
                            response_text.push_str(text);
                        }
                        sink.emit_message(msg.clone());
                    }
                    Ok(None) => {}
                    Err(e) => {
                        sink.emit_message(UnifiedMessage::Error {
                            message: format!("Parse error: {}", e),
                        });
                    }
                }
            }

            sink.emit_message(UnifiedMessage::TurnEnd { usage: None });

            // Save response to history for multi-turn
            if !response_text.is_empty() {
                let mut sessions_lock = sessions.lock().await;
                if let Some(session) = sessions_lock.get_mut(&sid) {
                    session.history.push((user_msg_owned, response_text));
                }
            }

            let mut sessions_lock = sessions.lock().await;
            sessions_lock.remove(&sid);
        });

        Ok(())
    }

    pub async fn stop_session(&self, session_id: &str) -> Result<(), String> {
        let mut sessions_lock = self.sessions.lock().await;
        if let Some(mut session) = sessions_lock.remove(session_id) {
            let _ = session.child.kill().await;
        }
        Ok(())
    }

    pub async fn check_installed(&self, provider: &str) -> Result<CliStatus, String> {
        let adapter = self.get_adapter(provider)?;
        let version_cmd = adapter.version_command();
        let auth_source = auth_source_for_adapter(adapter).to_string();

        if version_cmd.is_empty() {
            return Ok(CliStatus {
                provider: provider.to_string(),
                installed: false,
                version: None,
                path: None,
                authenticated: false,
                auth_status: CliAuthStatus::Unknown,
                auth_source,
                auth_message: None,
            });
        }

        let output = tokio::process::Command::new(kangnam_router::cli_utils::resolve_binary(
            &version_cmd[0],
        ))
        .env("PATH", kangnam_router::cli_utils::build_path_env())
        .args(&version_cmd[1..])
        .output()
        .await;

        match output {
            Ok(out) if out.status.success() => {
                let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let which_output = tokio::process::Command::new("which")
                    .env("PATH", kangnam_router::cli_utils::build_path_env())
                    .arg(adapter.command())
                    .output()
                    .await;
                let path = which_output
                    .ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
                let auth = probe_cli_auth(adapter).await;

                Ok(CliStatus {
                    provider: provider.to_string(),
                    installed: true,
                    version: Some(version),
                    path,
                    authenticated: auth.status == CliAuthStatus::Ready,
                    auth_status: auth.status,
                    auth_source,
                    auth_message: auth.message,
                })
            }
            _ => Ok(CliStatus {
                provider: provider.to_string(),
                installed: false,
                version: None,
                path: None,
                authenticated: false,
                auth_status: CliAuthStatus::Unknown,
                auth_source,
                auth_message: None,
            }),
        }
    }

    pub async fn install_cli(&self, provider: &str) -> Result<(), String> {
        let adapter = self.get_adapter(provider)?;
        let install_cmd = adapter
            .install_command()
            .ok_or_else(|| format!("No install command for {}", provider))?;

        if install_cmd.is_empty() {
            return Err("Empty install command".to_string());
        }

        let output = tokio::process::Command::new(kangnam_router::cli_utils::resolve_binary(
            &install_cmd[0],
        ))
        .env("PATH", kangnam_router::cli_utils::build_path_env())
        .args(&install_cmd[1..])
        .output()
        .await
        .map_err(|e| format!("Install failed: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Install failed: {}", stderr))
        }
    }
}

impl Default for CliManager {
    fn default() -> Self {
        Self::new()
    }
}

fn auth_source_for_adapter(adapter: &dyn CliAdapter) -> &'static str {
    match adapter.command() {
        "codex" | "claude" => "local_cli_login",
        _ => "unknown",
    }
}

fn parse_effort_command(message: &str) -> Option<String> {
    let effort = message.trim().strip_prefix("/effort ")?.trim();
    is_safe_reasoning_effort(effort).then(|| effort.to_string())
}

fn is_safe_reasoning_effort(effort: &str) -> bool {
    !effort.is_empty()
        && effort
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

async fn probe_cli_auth(adapter: &dyn CliAdapter) -> AuthProbe {
    match adapter.command() {
        "codex" => probe_codex_auth().await,
        "claude" => AuthProbe {
            status: CliAuthStatus::Unknown,
            message: Some("Claude CLI auth probing is not implemented yet".to_string()),
        },
        _ => AuthProbe {
            status: CliAuthStatus::NotApplicable,
            message: None,
        },
    }
}

async fn probe_codex_auth() -> AuthProbe {
    let output = tokio::process::Command::new(kangnam_router::cli_utils::resolve_binary("codex"))
        .env("PATH", kangnam_router::cli_utils::build_path_env())
        .args(["login", "status"])
        .output()
        .await;

    match output {
        Ok(out) => classify_codex_login_status(
            out.status.success(),
            &String::from_utf8_lossy(&out.stdout),
            &String::from_utf8_lossy(&out.stderr),
        ),
        Err(e) => AuthProbe {
            status: CliAuthStatus::Failed,
            message: Some(format!("failed to run `codex login status`: {e}")),
        },
    }
}

fn classify_codex_login_status(success: bool, stdout: &str, stderr: &str) -> AuthProbe {
    let combined = format!("{stdout}\n{stderr}");
    let normalized = combined.to_lowercase();
    let cleaned = combined
        .lines()
        .filter(|line| !line.to_lowercase().contains("could not update path"))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let message = if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    };

    if normalized.contains("rate limit") || normalized.contains("rate_limited") {
        return AuthProbe {
            status: CliAuthStatus::RateLimited,
            message,
        };
    }

    if normalized.contains("not logged in")
        || normalized.contains("not authenticated")
        || normalized.contains("login required")
        || normalized.contains("please login")
    {
        return AuthProbe {
            status: CliAuthStatus::NotLoggedIn,
            message,
        };
    }

    if success && normalized.contains("logged in") {
        return AuthProbe {
            status: CliAuthStatus::Ready,
            message,
        };
    }

    AuthProbe {
        status: if success {
            CliAuthStatus::Unknown
        } else {
            CliAuthStatus::Failed
        },
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_login_status_logged_in_is_ready() {
        let probe = classify_codex_login_status(true, "Logged in using ChatGPT\n", "");
        assert_eq!(probe.status, CliAuthStatus::Ready);
        assert_eq!(probe.message.as_deref(), Some("Logged in using ChatGPT"));
    }

    #[test]
    fn codex_login_status_not_logged_in_is_not_logged_in() {
        let probe = classify_codex_login_status(false, "Not logged in\n", "");
        assert_eq!(probe.status, CliAuthStatus::NotLoggedIn);
    }

    #[test]
    fn codex_login_status_filters_path_warning() {
        let probe = classify_codex_login_status(
            true,
            "WARNING: proceeding, even though we could not update PATH: Operation not permitted\nLogged in using ChatGPT\n",
            "",
        );
        assert_eq!(probe.status, CliAuthStatus::Ready);
        assert_eq!(probe.message.as_deref(), Some("Logged in using ChatGPT"));
    }

    #[test]
    fn effort_command_accepts_only_known_levels() {
        assert_eq!(parse_effort_command("/effort low").as_deref(), Some("low"));
        assert_eq!(
            parse_effort_command(" /effort medium ").as_deref(),
            Some("medium")
        );
        assert_eq!(parse_effort_command("/effort high").as_deref(), Some("high"));
        assert_eq!(
            parse_effort_command("/effort xhigh").as_deref(),
            Some("xhigh")
        );
        assert_eq!(
            parse_effort_command("/effort future-level").as_deref(),
            Some("future-level")
        );
        assert_eq!(parse_effort_command("/effort bad value"), None);
        assert_eq!(parse_effort_command("/effort bad\"value"), None);
    }
}
