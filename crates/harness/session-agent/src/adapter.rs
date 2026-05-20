#![allow(dead_code)]

use std::path::Path;
use tokio::process::Command;

use crate::types::{ClaudeEnhancedEvent, UnifiedMessage};

/// Each CLI provider implements this trait to handle its specific JSON format
/// and subprocess configuration.
pub trait CliAdapter: Send + Sync {
    /// Provider name (e.g., "claude", "codex")
    fn name(&self) -> &str;

    /// The CLI binary name to invoke (e.g., "claude", "codex")
    fn command(&self) -> &str;

    /// Build the Command for spawning the CLI process.
    /// For Claude Code: long-running with stdin/stdout pipes.
    /// For Codex CLI: one-shot per prompt via `codex exec`.
    fn build_command(&self, working_dir: &Path) -> Command;

    /// Build a provider command with optional model/runtime controls.
    /// Providers that do not support these controls can use the default
    /// implementation.
    fn build_command_with_options(
        &self,
        working_dir: &Path,
        _model: Option<&str>,
        _reasoning_effort: Option<&str>,
    ) -> Command {
        self.build_command(working_dir)
    }

    /// Parse one line of stdout JSON into a UnifiedMessage.
    /// Returns None if the line should be skipped (e.g., empty or non-JSON).
    fn parse_line(&self, line: &str) -> Result<Option<UnifiedMessage>, String>;

    /// Format a user message for writing to stdin.
    /// Returns None if the CLI does not support stdin messaging (e.g., Codex).
    fn format_user_message(&self, message: &str, session_id: &str) -> Option<String>;

    /// Whether the CLI supports persistent stdin (multi-turn in one process).
    /// Claude Code: true. Codex CLI: false (one process per prompt).
    fn supports_persistent_session(&self) -> bool;

    /// Command to check if CLI is installed (e.g., "claude --version")
    fn version_command(&self) -> Vec<String>;

    /// Command to install the CLI (e.g., ["npm", "install", "-g", "@anthropic-ai/claude-code"])
    fn install_command(&self) -> Option<Vec<String>>;

    /// Command args to list available skills. None if not supported.
    fn list_skills_command(&self) -> Option<Vec<String>>;

    /// Command args to list available agents. None if not supported.
    fn list_agents_command(&self) -> Option<Vec<String>>;

    fn enhanced_features(&self) -> bool {
        false
    }

    fn parse_enhanced(&self, _line: &str) -> Result<Option<ClaudeEnhancedEvent>, String> {
        Ok(None)
    }
}
