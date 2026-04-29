use serde::{Deserialize, Serialize};

use crate::Scope;

/// A capability the model can invoke. Either a built-in (Read/Edit/Bash/...),
/// a tool exposed by an MCP server, or a custom tool the consumer registers
/// at runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// JSON Schema describing the tool's input. Stored as opaque JSON to avoid
    /// pulling in a schema crate from `harness-core`.
    #[serde(default)]
    pub parameters: serde_json::Value,
    pub source: ToolSource,
    #[serde(default = "default_scope")]
    pub scope: Scope,
}

fn default_scope() -> Scope {
    Scope::User
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolSource {
    /// Harness-provided tool such as Read, Edit, Bash.
    Builtin(BuiltinTool),
    /// A tool exposed by a connected MCP server.
    Mcp { server: String, tool: String },
    /// A custom tool registered by the consuming application; the harness
    /// only stores metadata, the implementation is provided at runtime.
    Custom { handler: String },
}

/// Enumeration of harness built-ins. Kept open-ended via `Other` so storage
/// roundtrips don't break when a newer version adds tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinTool {
    Read,
    Write,
    Edit,
    Bash,
    Glob,
    Grep,
    WebFetch,
    WebSearch,
    Other(String),
}
