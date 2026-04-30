use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Mutex;
use tokio::process::Command;

use crate::adapter::CliAdapter;
use crate::types::{ClaudeEnhancedEvent, McpServerInfo, PluginInfo, UnifiedMessage};

/// Per-content-block accumulator for `tool_use.input` partial-JSON deltas.
/// Anthropic streams the JSON in `input_json_delta` chunks; we buffer them
/// per `content_block_index` and emit a single [`UnifiedMessage::ToolUseInput`]
/// at `content_block_stop` once the full JSON is available.
#[derive(Default, Clone)]
struct ToolUseAccumulator {
    id: String,
    /// Concatenated partial JSON. Parsed at stop time; if it parses we emit a
    /// structured `Value`, otherwise the raw string is wrapped as a one-field
    /// object so callers still get something they can render.
    input_json: String,
}

pub struct ClaudeAdapter {
    mcp_port: u16,
    model: Option<String>,
    /// Index → accumulator. Interior mutability so `parse_line(&self)` keeps
    /// the [`CliAdapter`] trait shape but can stitch streaming events.
    pending_tool_uses: Mutex<HashMap<u64, ToolUseAccumulator>>,
}

impl ClaudeAdapter {
    pub fn new() -> Self {
        Self {
            mcp_port: 3001,
            model: None,
            pending_tool_uses: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_port(port: u16) -> Self {
        Self {
            mcp_port: port,
            model: None,
            pending_tool_uses: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_model(mut self, model: Option<String>) -> Self {
        self.model = model;
        self
    }

    /// Parse a stream_event from Claude Code's NDJSON output
    fn parse_stream_event(
        &self,
        value: &serde_json::Value,
    ) -> Result<Option<UnifiedMessage>, String> {
        let event = value.get("event").ok_or("missing event field")?;
        let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let parent_tool_use_id = value.get("parent_tool_use_id").and_then(|v| v.as_str());
        let block_index = event.get("index").and_then(|v| v.as_u64());

        match event_type {
            "content_block_delta" => {
                let delta = event.get("delta").ok_or("missing delta")?;
                let delta_type = delta.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match delta_type {
                    "text_delta" => {
                        let text = delta.get("text").and_then(|t| t.as_str()).unwrap_or("");
                        if parent_tool_use_id.is_some() {
                            // Text from a subagent — emit as agent progress
                            Ok(Some(UnifiedMessage::AgentProgress {
                                id: parent_tool_use_id.unwrap_or("").to_string(),
                                message: text.to_string(),
                            }))
                        } else {
                            Ok(Some(UnifiedMessage::TextDelta {
                                text: text.to_string(),
                            }))
                        }
                    }
                    "thinking_delta" => {
                        // Anthropic extended-thinking stream chunk. Surface as a
                        // dedicated variant so the UI can show reasoning
                        // separately from final answer text.
                        let text = delta.get("thinking").and_then(|t| t.as_str()).unwrap_or("");
                        Ok(Some(UnifiedMessage::ThinkingDelta {
                            text: text.to_string(),
                        }))
                    }
                    "input_json_delta" => {
                        // Append the partial JSON for the matching content
                        // block. The structured input arrives at
                        // content_block_stop.
                        let partial = delta
                            .get("partial_json")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if let Some(idx) = block_index {
                            if let Ok(mut map) = self.pending_tool_uses.lock() {
                                if let Some(acc) = map.get_mut(&idx) {
                                    acc.input_json.push_str(partial);
                                }
                            }
                        }
                        Ok(None)
                    }
                    _ => Ok(None),
                }
            }
            "content_block_start" => {
                let block = event.get("content_block").ok_or("missing content_block")?;
                let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match block_type {
                    "tool_use" => {
                        let id = block
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let name = block
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        if name == "Agent" || name == "Task" {
                            Ok(Some(UnifiedMessage::AgentStart {
                                id: id.clone(),
                                name: name.clone(),
                                description: String::new(),
                            }))
                        } else {
                            // Begin buffering input_json_delta chunks for this
                            // block index. We still emit ToolUseStart now (with
                            // empty input) so UI can show the call immediately;
                            // the populated input is sent later via
                            // ToolUseInput when content_block_stop arrives.
                            if let Some(idx) = block_index {
                                if let Ok(mut map) = self.pending_tool_uses.lock() {
                                    map.insert(
                                        idx,
                                        ToolUseAccumulator {
                                            id: id.clone(),
                                            input_json: String::new(),
                                        },
                                    );
                                }
                            }
                            Ok(Some(UnifiedMessage::ToolUseStart {
                                id,
                                name,
                                input: serde_json::Value::Object(serde_json::Map::new()),
                            }))
                        }
                    }
                    _ => Ok(None),
                }
            }
            "content_block_stop" => {
                // If we were accumulating input JSON for this block, emit a
                // final ToolUseInput with the parsed payload. Subagent
                // (Agent/Task) blocks are not buffered, so this is a no-op for
                // them.
                if let Some(idx) = block_index {
                    let acc = self
                        .pending_tool_uses
                        .lock()
                        .ok()
                        .and_then(|mut map| map.remove(&idx));
                    if let Some(acc) = acc {
                        if acc.input_json.is_empty() {
                            return Ok(None);
                        }
                        let parsed = serde_json::from_str::<serde_json::Value>(&acc.input_json)
                            .unwrap_or_else(|_| {
                                serde_json::json!({ "_raw_partial_json": acc.input_json })
                            });
                        return Ok(Some(UnifiedMessage::ToolUseInput {
                            id: acc.id,
                            input: parsed,
                        }));
                    }
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Parse an assistant turn message with content blocks
    fn parse_assistant(&self, value: &serde_json::Value) -> Result<Option<UnifiedMessage>, String> {
        let message = match value.get("message") {
            Some(m) => m,
            None => return Ok(None),
        };
        let _content = match message.get("content").and_then(|c| c.as_array()) {
            Some(c) => c,
            None => return Ok(None),
        };

        // Skip assistant turn messages — text is already streamed via
        // stream_event text_delta. Emitting here would cause duplicate display.
        Ok(None)
    }

    /// Parse a user-role message. Anthropic emits these to surface tool
    /// results back into the conversation as `tool_result` content blocks
    /// keyed by `tool_use_id`. Open-design's `claude-stream.js:133-145`
    /// folds them into typed `tool_result` events; we mirror that.
    fn parse_user(&self, value: &serde_json::Value) -> Result<Option<UnifiedMessage>, String> {
        let message = match value.get("message") {
            Some(m) => m,
            None => return Ok(None),
        };
        let content = match message.get("content").and_then(|c| c.as_array()) {
            Some(c) => c,
            None => return Ok(None),
        };

        for block in content {
            let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if block_type != "tool_result" {
                continue;
            }
            let id = block
                .get("tool_use_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let is_error = block
                .get("is_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            // `content` may be a plain string or an array of typed blocks
            // (text/image). Concatenate text segments; ignore image blocks
            // (we don't have a binary channel on UnifiedMessage::ToolResult).
            let output = match block.get("content") {
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(serde_json::Value::Array(parts)) => parts
                    .iter()
                    .filter_map(|p| {
                        if p.get("type").and_then(|t| t.as_str()) == Some("text") {
                            p.get("text").and_then(|t| t.as_str()).map(String::from)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(""),
                _ => String::new(),
            };
            return Ok(Some(UnifiedMessage::ToolResult {
                id,
                output,
                is_error,
            }));
        }
        Ok(None)
    }

    /// Parse a result message (session end)
    fn parse_result(&self, value: &serde_json::Value) -> Result<Option<UnifiedMessage>, String> {
        let is_error = value
            .get("is_error")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_error {
            let message = value
                .get("result")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error")
                .to_string();
            Ok(Some(UnifiedMessage::Error { message }))
        } else {
            Ok(Some(UnifiedMessage::TurnEnd {
                usage: None, // Claude Code reports cost_usd, not raw tokens in result
            }))
        }
    }

    /// Parse a system/init event into SessionMeta
    fn parse_system_init(
        &self,
        value: &serde_json::Value,
    ) -> Result<Option<ClaudeEnhancedEvent>, String> {
        let session_id = value
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let tools = value
            .get("tools")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let skills = value
            .get("skills")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let slash_commands = value
            .get("slash_commands")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let agents = value
            .get("agents")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let plugins = value
            .get("plugins")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|p| PluginInfo {
                        name: p.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        path: p.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mcp_servers = value
            .get("mcp_servers")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|s| McpServerInfo {
                        name: s.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        status: s
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let model = value
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let permission_mode = value
            .get("permissionMode")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let cwd = value
            .get("cwd")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let claude_code_version = value
            .get("claude_code_version")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(Some(ClaudeEnhancedEvent::SessionMeta {
            session_id,
            tools,
            skills,
            slash_commands,
            agents,
            plugins,
            mcp_servers,
            model,
            permission_mode,
            cwd,
            claude_code_version,
        }))
    }

    /// Parse system/task_* events
    fn parse_system_task(
        &self,
        subtype: &str,
        value: &serde_json::Value,
    ) -> Result<Option<ClaudeEnhancedEvent>, String> {
        match subtype {
            "task_started" => Ok(Some(ClaudeEnhancedEvent::TaskStarted {
                task_id: value
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                tool_use_id: value
                    .get("tool_use_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                description: value
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                task_type: value
                    .get("task_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            })),
            "task_progress" => Ok(Some(ClaudeEnhancedEvent::TaskProgress {
                task_id: value
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                description: value
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                usage: value.get("usage").cloned(),
                last_tool_name: value
                    .get("last_tool_name")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            })),
            "task_notification" => Ok(Some(ClaudeEnhancedEvent::TaskNotification {
                task_id: value
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                status: value
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                summary: value
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            })),
            _ => Ok(None),
        }
    }

    /// Parse system/hook_* events
    fn parse_system_hook(
        &self,
        subtype: &str,
        value: &serde_json::Value,
    ) -> Result<Option<ClaudeEnhancedEvent>, String> {
        match subtype {
            "hook_started" => Ok(Some(ClaudeEnhancedEvent::HookStarted {
                hook_id: value
                    .get("hook_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                hook_name: value
                    .get("hook_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                hook_event: value
                    .get("hook_event")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            })),
            "hook_progress" => Ok(Some(ClaudeEnhancedEvent::HookProgress {
                hook_id: value
                    .get("hook_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                stdout: value
                    .get("stdout")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                stderr: value
                    .get("stderr")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            })),
            "hook_response" => Ok(Some(ClaudeEnhancedEvent::HookResponse {
                hook_id: value
                    .get("hook_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            })),
            _ => Ok(None),
        }
    }

    /// Parse a result message into ResultSummary
    fn parse_result_summary(
        &self,
        value: &serde_json::Value,
    ) -> Result<Option<ClaudeEnhancedEvent>, String> {
        let cost_usd = value
            .get("total_cost_usd")
            .and_then(|v| v.as_f64())
            .or_else(|| value.get("cost_usd").and_then(|v| v.as_f64()));
        let usage = value.get("usage").cloned();
        let duration_ms = value.get("duration_ms").and_then(|v| v.as_u64());
        let num_turns = value
            .get("num_turns")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        let model_usage = value.get("modelUsage").cloned();
        let permission_denials = value
            .get("permission_denials")
            .and_then(|v| v.as_array())
            .map(|arr| arr.clone())
            .unwrap_or_default();

        Ok(Some(ClaudeEnhancedEvent::ResultSummary {
            cost_usd,
            usage,
            duration_ms,
            num_turns,
            model_usage,
            permission_denials,
        }))
    }

    /// Parse a rate_limit_event
    fn parse_rate_limit(
        &self,
        value: &serde_json::Value,
    ) -> Result<Option<ClaudeEnhancedEvent>, String> {
        let rate_limit_info = value.get("rate_limit_info");
        let status = rate_limit_info
            .and_then(|v| v.get("status"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let utilization = rate_limit_info
            .and_then(|v| v.get("utilization"))
            .and_then(|v| v.as_f64());
        let rate_limit_type = rate_limit_info
            .and_then(|v| v.get("rateLimitType").or_else(|| v.get("rate_limit_type")))
            .and_then(|v| v.as_str())
            // Fallback: check top-level fields (some versions put it outside rate_limit_info)
            .or_else(|| value.get("rateLimitType").and_then(|v| v.as_str()))
            .or_else(|| value.get("rate_limit_type").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string();

        Ok(Some(ClaudeEnhancedEvent::RateLimit {
            status,
            utilization,
            rate_limit_type,
        }))
    }
}

impl CliAdapter for ClaudeAdapter {
    fn name(&self) -> &str {
        "claude"
    }

    fn command(&self) -> &str {
        "claude"
    }

    fn build_command(&self, working_dir: &Path) -> Command {
        // Use llm-router's resolver so we find `claude` even when launched
        // from a Tauri/Electron GUI context (where $PATH lacks Homebrew etc.).
        let mut cmd = Command::new(kangnam_router::cli_utils::resolve_binary("claude"));
        cmd.env("PATH", kangnam_router::cli_utils::build_path_env());
        cmd.args([
            "-p",
            "--output-format",
            "stream-json",
            "--input-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--include-hook-events",
        ]);
        if let Some(ref model) = self.model {
            cmd.args(["--model", model]);
        }
        // Register our MCP server for permission handling
        let mcp_config = serde_json::json!({
            "mcpServers": {
                "kangnam": {
                    "type": "http",
                    "url": format!("http://localhost:{}/mcp", self.mcp_port)
                }
            }
        });
        cmd.arg("--mcp-config");
        cmd.arg(mcp_config.to_string());
        cmd.arg("--permission-prompt-tool");
        cmd.arg("mcp__kangnam__approve");
        cmd.current_dir(working_dir);
        cmd.stdin(Stdio::piped());
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

        let msg_type = value.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match msg_type {
            "system" => {
                let subtype = value.get("subtype").and_then(|s| s.as_str()).unwrap_or("");
                if subtype == "init" {
                    let session_id = value
                        .get("session_id")
                        .and_then(|s| s.as_str())
                        .unwrap_or("")
                        .to_string();
                    Ok(Some(UnifiedMessage::SessionInit { session_id }))
                } else {
                    // Display text content from system messages (e.g., /context, /cost output)
                    let text = value
                        .get("message")
                        .and_then(|m| m.as_str())
                        .or_else(|| value.get("text").and_then(|t| t.as_str()));
                    if let Some(text) = text {
                        if !text.is_empty() {
                            return Ok(Some(UnifiedMessage::TextDelta {
                                text: text.to_string(),
                            }));
                        }
                    }
                    Ok(None)
                }
            }
            "stream_event" => self.parse_stream_event(&value),
            "assistant" => self.parse_assistant(&value),
            "user" => self.parse_user(&value),
            "result" => self.parse_result(&value),
            _ => Ok(None),
        }
    }

    fn format_user_message(&self, message: &str, session_id: &str) -> Option<String> {
        let msg = serde_json::json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": message
            },
            "session_id": session_id
        });
        Some(format!("{}\n", msg))
    }

    fn supports_persistent_session(&self) -> bool {
        true
    }

    fn version_command(&self) -> Vec<String> {
        vec!["claude".to_string(), "--version".to_string()]
    }

    fn install_command(&self) -> Option<Vec<String>> {
        Some(vec![
            "npm".to_string(),
            "install".to_string(),
            "-g".to_string(),
            "@anthropic-ai/claude-code".to_string(),
        ])
    }

    fn list_skills_command(&self) -> Option<Vec<String>> {
        None
    }

    fn list_agents_command(&self) -> Option<Vec<String>> {
        None
    }

    fn enhanced_features(&self) -> bool {
        true
    }

    fn parse_enhanced(&self, line: &str) -> Result<Option<ClaudeEnhancedEvent>, String> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(None);
        }

        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|e| format!("JSON parse error: {}", e))?;

        let msg_type = value.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match msg_type {
            "system" => {
                let subtype = value.get("subtype").and_then(|s| s.as_str()).unwrap_or("");
                match subtype {
                    "init" => self.parse_system_init(&value),
                    "task_started" | "task_progress" | "task_notification" => {
                        self.parse_system_task(subtype, &value)
                    }
                    "hook_started" | "hook_progress" | "hook_response" => {
                        self.parse_system_hook(subtype, &value)
                    }
                    "status" => Ok(Some(ClaudeEnhancedEvent::StatusUpdate {
                        status: value
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        permission_mode: value
                            .get("permissionMode")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                    })),
                    "compact_boundary" => Ok(Some(ClaudeEnhancedEvent::CompactBoundary)),
                    _ => Ok(None),
                }
            }
            "result" => self.parse_result_summary(&value),
            "rate_limit_event" => self.parse_rate_limit(&value),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::CliAdapter;

    fn adapter() -> ClaudeAdapter {
        ClaudeAdapter::new()
    }

    #[test]
    fn test_parse_enhanced_system_init() {
        let line = r#"{"type":"system","subtype":"init","session_id":"abc-123","tools":["Bash","Read"],"skills":["my-skill"],"slash_commands":["/compact"],"agents":["code-reviewer","frontend-dev"],"plugins":[{"name":"superpowers","path":"/tmp/sp"}],"mcp_servers":[{"name":"server1","status":"connected"}],"model":"claude-sonnet-4-6","permissionMode":"default","cwd":"/tmp","claude_code_version":"2.1.0"}"#;
        let result = adapter().parse_enhanced(line).unwrap().unwrap();
        match result {
            ClaudeEnhancedEvent::SessionMeta {
                session_id,
                tools,
                skills,
                slash_commands,
                agents,
                plugins,
                mcp_servers,
                model,
                permission_mode,
                cwd,
                claude_code_version,
            } => {
                assert_eq!(session_id, "abc-123");
                assert_eq!(tools, vec!["Bash", "Read"]);
                assert_eq!(skills, vec!["my-skill"]);
                assert_eq!(slash_commands, vec!["/compact"]);
                assert_eq!(agents, vec!["code-reviewer", "frontend-dev"]);
                assert_eq!(plugins.len(), 1);
                assert_eq!(plugins[0].name, "superpowers");
                assert_eq!(mcp_servers.len(), 1);
                assert_eq!(mcp_servers[0].name, "server1");
                assert_eq!(mcp_servers[0].status, "connected");
                assert_eq!(model, "claude-sonnet-4-6");
                assert_eq!(permission_mode, "default");
                assert_eq!(cwd, "/tmp");
                assert_eq!(claude_code_version, "2.1.0");
            }
            other => panic!("Expected SessionMeta, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_enhanced_task_started() {
        let line = r#"{"type":"system","subtype":"task_started","task_id":"task-1","tool_use_id":"tu-1","description":"Running tests","task_type":"agent"}"#;
        let result = adapter().parse_enhanced(line).unwrap().unwrap();
        match result {
            ClaudeEnhancedEvent::TaskStarted {
                task_id,
                tool_use_id,
                description,
                task_type,
            } => {
                assert_eq!(task_id, "task-1");
                assert_eq!(tool_use_id, "tu-1");
                assert_eq!(description, "Running tests");
                assert_eq!(task_type, "agent");
            }
            other => panic!("Expected TaskStarted, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_enhanced_task_notification() {
        let line = r#"{"type":"system","subtype":"task_notification","task_id":"task-1","status":"completed","summary":"All tests passed"}"#;
        let result = adapter().parse_enhanced(line).unwrap().unwrap();
        match result {
            ClaudeEnhancedEvent::TaskNotification {
                task_id,
                status,
                summary,
            } => {
                assert_eq!(task_id, "task-1");
                assert_eq!(status, "completed");
                assert_eq!(summary, Some("All tests passed".to_string()));
            }
            other => panic!("Expected TaskNotification, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_enhanced_result_summary() {
        let line = r#"{"type":"result","total_cost_usd":0.042,"usage":{"input_tokens":1000,"output_tokens":500},"duration_ms":12345,"num_turns":3,"modelUsage":{"claude-sonnet-4-6":{"input":800,"output":400}},"permission_denials":[{"tool":"Bash","reason":"user denied"}]}"#;
        let result = adapter().parse_enhanced(line).unwrap().unwrap();
        match result {
            ClaudeEnhancedEvent::ResultSummary {
                cost_usd,
                usage,
                duration_ms,
                num_turns,
                model_usage,
                permission_denials,
            } => {
                assert!((cost_usd.unwrap() - 0.042).abs() < f64::EPSILON);
                assert!(usage.is_some());
                assert_eq!(duration_ms, Some(12345));
                assert_eq!(num_turns, Some(3));
                assert!(model_usage.is_some());
                assert_eq!(permission_denials.len(), 1);
            }
            other => panic!("Expected ResultSummary, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_enhanced_rate_limit() {
        let line = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"throttled","utilization":0.95},"rate_limit_type":"tokens"}"#;
        let result = adapter().parse_enhanced(line).unwrap().unwrap();
        match result {
            ClaudeEnhancedEvent::RateLimit {
                status,
                utilization,
                rate_limit_type,
            } => {
                assert_eq!(status, "throttled");
                assert!((utilization.unwrap() - 0.95).abs() < f64::EPSILON);
                assert_eq!(rate_limit_type, "tokens");
            }
            other => panic!("Expected RateLimit, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_enhanced_compact_boundary() {
        let line = r#"{"type":"system","subtype":"compact_boundary"}"#;
        let result = adapter().parse_enhanced(line).unwrap().unwrap();
        assert!(matches!(result, ClaudeEnhancedEvent::CompactBoundary));
    }

    #[test]
    fn test_parse_enhanced_ignores_stream_event() {
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"hello"}}}"#;
        let result = adapter().parse_enhanced(line).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_line_detects_agent_and_task_names() {
        let adapter = adapter();

        // "Agent" tool name should produce AgentStart
        let agent_line = r#"{"type":"stream_event","event":{"type":"content_block_start","content_block":{"type":"tool_use","id":"tu-agent","name":"Agent"}}}"#;
        let result = adapter.parse_line(agent_line).unwrap().unwrap();
        match &result {
            UnifiedMessage::AgentStart { id, name, .. } => {
                assert_eq!(id, "tu-agent");
                assert_eq!(name, "Agent");
            }
            other => panic!("Expected AgentStart for 'Agent', got {:?}", other),
        }

        // "Task" tool name should also produce AgentStart
        let task_line = r#"{"type":"stream_event","event":{"type":"content_block_start","content_block":{"type":"tool_use","id":"tu-task","name":"Task"}}}"#;
        let result = adapter.parse_line(task_line).unwrap().unwrap();
        match &result {
            UnifiedMessage::AgentStart { id, name, .. } => {
                assert_eq!(id, "tu-task");
                assert_eq!(name, "Task");
            }
            other => panic!("Expected AgentStart for 'Task', got {:?}", other),
        }
    }

    /// Phase 0b parity fix #1: thinking_delta deltas surface as a dedicated
    /// `ThinkingDelta` variant. Previously these were silently dropped by the
    /// `_ => Ok(None)` arm in parse_stream_event.
    #[test]
    fn parity_thinking_delta_emits_thinking_delta_variant() {
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Let me reconsider the approach…"}}}"#;
        let result = adapter().parse_line(line).unwrap().unwrap();
        match result {
            UnifiedMessage::ThinkingDelta { text } => {
                assert_eq!(text, "Let me reconsider the approach…");
            }
            other => panic!("Expected ThinkingDelta, got {:?}", other),
        }
    }

    /// Phase 0b parity fix #2: user-role messages with `tool_use_id` content
    /// blocks emit `ToolResult`. Previously the "user" msg_type fell through
    /// the dispatcher's `_ => Ok(None)` arm.
    #[test]
    fn parity_user_role_tool_result_string_content() {
        let line = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_01","content":"file contents here","is_error":false}]}}"#;
        let result = adapter().parse_line(line).unwrap().unwrap();
        match result {
            UnifiedMessage::ToolResult {
                id,
                output,
                is_error,
            } => {
                assert_eq!(id, "toolu_01");
                assert_eq!(output, "file contents here");
                assert!(!is_error);
            }
            other => panic!("Expected ToolResult, got {:?}", other),
        }
    }

    /// Same as above but content is an array of typed text blocks (Anthropic
    /// emits this shape when a tool's response includes multiple parts or
    /// images). Text segments concatenate; non-text segments are skipped.
    #[test]
    fn parity_user_role_tool_result_array_content() {
        let line = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_02","is_error":true,"content":[{"type":"text","text":"line1"},{"type":"text","text":"\nline2"},{"type":"image","source":{"type":"base64","media_type":"image/png","data":"…"}}]}]}}"#;
        let result = adapter().parse_line(line).unwrap().unwrap();
        match result {
            UnifiedMessage::ToolResult {
                id,
                output,
                is_error,
            } => {
                assert_eq!(id, "toolu_02");
                assert_eq!(output, "line1\nline2");
                assert!(is_error);
            }
            other => panic!("Expected ToolResult, got {:?}", other),
        }
    }

    /// Phase 0b parity fix #3: `input_json_delta` chunks accumulate per
    /// content-block index and emit a final `ToolUseInput` at
    /// content_block_stop. The initial `ToolUseStart` still fires (with
    /// empty input) so consumers see the call begin immediately.
    #[test]
    fn parity_tool_use_input_accumulates_and_emits_at_stop() {
        let adapter = adapter();

        // Start a tool_use block at index 0.
        let start = r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_xyz","name":"Read","input":{}}}}"#;
        let r1 = adapter.parse_line(start).unwrap().unwrap();
        match &r1 {
            UnifiedMessage::ToolUseStart { id, name, input } => {
                assert_eq!(id, "toolu_xyz");
                assert_eq!(name, "Read");
                assert_eq!(*input, serde_json::Value::Object(serde_json::Map::new()));
            }
            other => panic!("Expected ToolUseStart, got {:?}", other),
        }

        // Stream the JSON in 3 fragments. Each parse returns Ok(None).
        let frag = |s: &str| {
            format!(
                r#"{{"type":"stream_event","event":{{"type":"content_block_delta","index":0,"delta":{{"type":"input_json_delta","partial_json":"{}"}}}}}}"#,
                s.replace('"', r#"\""#)
            )
        };
        for partial in [r#"{"file_p"#, r#"ath":"/tm"#, r#"p/x.txt"}"#] {
            let line = frag(partial);
            assert!(adapter.parse_line(&line).unwrap().is_none());
        }

        // content_block_stop emits ToolUseInput with the parsed JSON.
        let stop = r#"{"type":"stream_event","event":{"type":"content_block_stop","index":0}}"#;
        let r2 = adapter.parse_line(stop).unwrap().unwrap();
        match r2 {
            UnifiedMessage::ToolUseInput { id, input } => {
                assert_eq!(id, "toolu_xyz");
                assert_eq!(
                    input,
                    serde_json::json!({"file_path": "/tmp/x.txt"}),
                    "expected accumulated JSON to parse"
                );
            }
            other => panic!("Expected ToolUseInput, got {:?}", other),
        }

        // Subsequent stop with no pending accumulator is a no-op.
        let stop_again = r#"{"type":"stream_event","event":{"type":"content_block_stop","index":0}}"#;
        assert!(adapter.parse_line(stop_again).unwrap().is_none());
    }

    /// Subagent (Agent/Task) tool_use blocks are NOT buffered for input
    /// accumulation — they emit AgentStart immediately and have no
    /// ToolUseInput follow-up. Verify content_block_stop is a no-op here.
    #[test]
    fn parity_tool_use_subagent_skips_input_accumulation() {
        let adapter = adapter();
        let start = r#"{"type":"stream_event","event":{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"tu-task","name":"Task"}}}"#;
        let r1 = adapter.parse_line(start).unwrap().unwrap();
        assert!(matches!(r1, UnifiedMessage::AgentStart { .. }));

        let stop = r#"{"type":"stream_event","event":{"type":"content_block_stop","index":1}}"#;
        assert!(
            adapter.parse_line(stop).unwrap().is_none(),
            "subagent stop should be a no-op (no accumulator entry)"
        );
    }
}
