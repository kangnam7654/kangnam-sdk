use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

use crate::adapter::CliAdapter;
use crate::types::UnifiedMessage;

pub struct CodexAdapter {
    provider_name: &'static str,
}

impl CodexAdapter {
    pub fn new() -> Self {
        Self {
            provider_name: "codex",
        }
    }

    pub fn subscription() -> Self {
        Self {
            provider_name: "codex_cli",
        }
    }

    fn parse_item_event(
        &self,
        value: &serde_json::Value,
    ) -> Result<Option<UnifiedMessage>, String> {
        let event_type = value.get("type").and_then(|t| t.as_str()).unwrap_or("");

        if !event_type.starts_with("item.") {
            return Ok(None);
        }

        let item = value.get("item").unwrap_or(value);
        let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match item_type {
            "message" => {
                let content = item.get("content").and_then(|c| c.as_str()).unwrap_or("");
                if !content.is_empty() {
                    Ok(Some(UnifiedMessage::TextDelta {
                        text: content.to_string(),
                    }))
                } else {
                    Ok(None)
                }
            }
            "function_call" | "command" => {
                let id = item
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let name = item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("command")
                    .to_string();
                let args = item
                    .get("arguments")
                    .or_else(|| item.get("command"))
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                Ok(Some(UnifiedMessage::ToolUseStart {
                    id,
                    name,
                    input: args,
                }))
            }
            "function_call_output" | "command_output" => {
                let id = item
                    .get("call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let output = item
                    .get("output")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Ok(Some(UnifiedMessage::ToolResult {
                    id,
                    output,
                    is_error: false,
                }))
            }
            "file_change" => {
                let file_path = item.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let diff = item
                    .get("diff")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let id = item
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Ok(Some(UnifiedMessage::PermissionRequest {
                    id,
                    tool: "file_edit".to_string(),
                    description: format!("Edit file: {}", file_path),
                    diff,
                }))
            }
            _ => Ok(None),
        }
    }
}

impl CliAdapter for CodexAdapter {
    fn name(&self) -> &str {
        self.provider_name
    }

    fn command(&self) -> &str {
        "codex"
    }

    fn build_command(&self, working_dir: &Path) -> Command {
        self.build_command_with_options(working_dir, None, None)
    }

    fn build_command_with_options(
        &self,
        working_dir: &Path,
        model: Option<&str>,
        reasoning_effort: Option<&str>,
    ) -> Command {
        // Codex CLI is one-shot: `codex exec --json "<prompt>"`
        // Use llm-router's resolver so we find `codex` even when launched
        // from a Tauri/Electron GUI context (where $PATH lacks Homebrew etc.).
        let mut cmd = Command::new(kangnam_router::cli_utils::resolve_binary("codex"));
        cmd.env("PATH", kangnam_router::cli_utils::build_path_env());
        cmd.arg("exec");
        cmd.arg("--json");
        if let Some(model) = model.map(str::trim).filter(|m| !m.is_empty()) {
            cmd.arg("--model");
            cmd.arg(model);
        }
        if let Some(effort) = normalize_reasoning_effort(reasoning_effort) {
            cmd.arg("-c");
            cmd.arg(format!("model_reasoning_effort=\"{effort}\""));
        }
        cmd.current_dir(working_dir);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd
    }

    fn parse_line(&self, line: &str) -> Result<Option<UnifiedMessage>, String> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(None);
        }

        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|e| format!("JSON parse error: {}", e))?;

        let event_type = value.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match event_type {
            "thread.started" => {
                let id = value
                    .get("thread_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Ok(Some(UnifiedMessage::SessionInit { session_id: id }))
            }
            "turn.completed" => Ok(Some(UnifiedMessage::TurnEnd { usage: None })),
            "turn.failed" | "error" => {
                let message = value
                    .get("error")
                    .or_else(|| value.get("message"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error")
                    .to_string();
                Ok(Some(UnifiedMessage::Error { message }))
            }
            t if t.starts_with("item.") => self.parse_item_event(&value),
            _ => Ok(None),
        }
    }

    fn format_user_message(&self, _message: &str, _session_id: &str) -> Option<String> {
        // Codex does not support stdin messaging — each prompt is a new process
        None
    }

    fn supports_persistent_session(&self) -> bool {
        false
    }

    fn version_command(&self) -> Vec<String> {
        vec!["codex".to_string(), "--version".to_string()]
    }

    fn install_command(&self) -> Option<Vec<String>> {
        Some(vec![
            "npm".to_string(),
            "install".to_string(),
            "-g".to_string(),
            "@openai/codex".to_string(),
        ])
    }

    fn list_skills_command(&self) -> Option<Vec<String>> {
        None
    }

    fn list_agents_command(&self) -> Option<Vec<String>> {
        None
    }

    fn enhanced_features(&self) -> bool {
        false
    }
}

fn normalize_reasoning_effort(effort: Option<&str>) -> Option<String> {
    let effort = effort?.trim();
    is_safe_reasoning_effort(effort).then(|| effort.to_string())
}

fn is_safe_reasoning_effort(effort: &str) -> bool {
    !effort.is_empty()
        && effort
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasoning_effort_accepts_known_values_only() {
        assert_eq!(
            normalize_reasoning_effort(Some("low")).as_deref(),
            Some("low")
        );
        assert_eq!(
            normalize_reasoning_effort(Some("medium")).as_deref(),
            Some("medium")
        );
        assert_eq!(
            normalize_reasoning_effort(Some("high")).as_deref(),
            Some("high")
        );
        assert_eq!(
            normalize_reasoning_effort(Some("xhigh")).as_deref(),
            Some("xhigh")
        );
        assert_eq!(
            normalize_reasoning_effort(Some("future-level")).as_deref(),
            Some("future-level")
        );
        assert_eq!(normalize_reasoning_effort(Some("not valid")), None);
        assert_eq!(normalize_reasoning_effort(Some("bad\"value")), None);
        assert_eq!(normalize_reasoning_effort(None), None);
    }
}
