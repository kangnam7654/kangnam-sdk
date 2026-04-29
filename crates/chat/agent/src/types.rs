#![allow(dead_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UnifiedMessage {
    /// Streaming text chunk from the assistant
    TextDelta { text: String },

    /// Tool call started (file read, edit, bash, etc.)
    ToolUseStart {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    /// Tool execution result
    ToolResult {
        id: String,
        output: String,
        is_error: bool,
    },

    /// CLI requests permission for a dangerous action
    PermissionRequest {
        id: String,
        tool: String,
        description: String,
        diff: Option<String>,
    },

    /// Subagent spawned
    AgentStart {
        id: String,
        name: String,
        description: String,
    },

    /// Subagent progress update
    AgentProgress { id: String, message: String },

    /// Subagent completed
    AgentEnd { id: String, result: String },

    /// Skill invoked via slash command
    SkillInvoked {
        name: String,
        args: Option<String>,
    },

    /// Assistant turn completed
    TurnEnd { usage: Option<TokenUsage> },

    /// Error from CLI process
    Error { message: String },

    /// Session initialized
    SessionInit { session_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliStatus {
    pub provider: String,
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub authenticated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    pub provider: String,
    pub command: String,
    pub working_dir: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClaudeEnhancedEvent {
    SessionMeta {
        session_id: String,
        tools: Vec<String>,
        skills: Vec<String>,
        slash_commands: Vec<String>,
        agents: Vec<String>,
        plugins: Vec<PluginInfo>,
        mcp_servers: Vec<McpServerInfo>,
        model: String,
        permission_mode: String,
        cwd: String,
        claude_code_version: String,
    },
    TaskStarted {
        task_id: String,
        tool_use_id: String,
        description: String,
        task_type: String,
    },
    TaskProgress {
        task_id: String,
        description: String,
        usage: Option<serde_json::Value>,
        last_tool_name: Option<String>,
    },
    TaskNotification {
        task_id: String,
        status: String,
        summary: Option<String>,
    },
    ResultSummary {
        cost_usd: Option<f64>,
        usage: Option<serde_json::Value>,
        duration_ms: Option<u64>,
        num_turns: Option<u32>,
        model_usage: Option<serde_json::Value>,
        permission_denials: Vec<serde_json::Value>,
    },
    RateLimit {
        status: String,
        utilization: Option<f64>,
        rate_limit_type: String,
    },
    HookStarted {
        hook_id: String,
        hook_name: String,
        hook_event: String,
    },
    HookProgress {
        hook_id: String,
        stdout: Option<String>,
        stderr: Option<String>,
    },
    HookResponse {
        hook_id: String,
    },
    StatusUpdate {
        status: String,
        permission_mode: Option<String>,
    },
    CompactBoundary,
}
