//! MCP wire types — server info, tool descriptors, tool-call results,
//! error variants. All shapes mirror the MCP JSON-RPC schema verbatim.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Errors returned by [`super::McpClient`] operations.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    /// Failed to spawn / communicate with the underlying transport
    /// (typically a child process).
    #[error("transport error: {0}")]
    Transport(String),

    /// Transport closed before the response arrived.
    #[error("transport closed before response (request id: {request_id})")]
    Closed { request_id: String },

    /// No response within the configured timeout.
    #[error("request timed out after {seconds}s (method: {method})")]
    Timeout { method: String, seconds: u64 },

    /// Server responded with a malformed or unexpected JSON-RPC frame.
    #[error("protocol error: {0}")]
    Protocol(String),

    /// Server returned a structured JSON-RPC `error` object.
    #[error("MCP server error {code}: {message}")]
    Server {
        code: i64,
        message: String,
        #[allow(dead_code)]
        data: Option<Value>,
    },

    /// (De)serialization failed.
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Information about the server, returned by `initialize`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    /// Optional protocol version the server speaks. The client logs a
    /// `tracing::warn!` when this differs from its own.
    #[serde(default)]
    pub protocol_version: Option<String>,
}

/// One tool advertised by the server's `tools/list` response.
///
/// `input_schema` is an opaque JSON Schema object — the bridge passes
/// it through to the LLM provider as the tool's parameter schema, so
/// the model sees exactly what the MCP server advertised.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTool {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_schema")]
    pub input_schema: Value,
}

fn default_schema() -> Value {
    serde_json::json!({"type": "object"})
}

/// Result of a `tools/call` invocation. The MCP wire shape is an array
/// of typed content blocks (`text`, `image`, `resource`); we collapse
/// it into a flat structure that's easier to feed back to the LLM.
///
/// Deserialise-only — the bridge never sends results back upstream.
#[derive(Debug, Clone, Deserialize)]
pub struct McpToolResult {
    /// Content blocks the server returned. Unknown `type` values are
    /// captured into [`McpToolContent::Other`] so a single malformed
    /// block doesn't fail the whole tool result.
    #[serde(default, deserialize_with = "deserialize_content_lossy")]
    pub content: Vec<McpToolContent>,
    /// `true` when the server flagged the call as failed via
    /// `result.isError == true` (distinct from a JSON-RPC `error`
    /// envelope, which raises [`McpError::Server`] instead).
    #[serde(default, rename = "isError")]
    pub is_error: bool,
}

fn deserialize_content_lossy<'de, D>(d: D) -> Result<Vec<McpToolContent>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: Vec<Value> = Vec::deserialize(d)?;
    Ok(raw
        .into_iter()
        .map(|v| serde_json::from_value(v.clone()).unwrap_or(McpToolContent::Other(v)))
        .collect())
}

impl McpToolResult {
    /// Collapse all `Text`-typed content blocks into a single string.
    /// Image / resource blocks are summarised as `<image:mime>` /
    /// `<resource:uri>` placeholders so the LLM gets a deterministic
    /// payload even on multimodal results.
    pub fn flatten_text(&self) -> String {
        let mut out = String::new();
        for c in &self.content {
            match c {
                McpToolContent::Text { text } => out.push_str(text),
                McpToolContent::Image { mime_type, .. } => {
                    out.push_str(&format!("<image:{mime_type}>"));
                }
                McpToolContent::Resource { uri, .. } => {
                    out.push_str(&format!("<resource:{uri}>"));
                }
                McpToolContent::Other(value) => {
                    out.push_str(&value.to_string());
                }
            }
        }
        out
    }
}

/// One block inside an [`McpToolResult::content`] array.
///
/// `#[non_exhaustive]` so future MCP spec additions (e.g. audio,
/// embeddings) can be added without breaking external `match` blocks.
///
/// Internal `type` tag drives deserialisation. Unknown discriminants
/// land in [`Self::Other`] via the parent's `deserialize_content_lossy`,
/// preserving the raw JSON for callers that want to introspect it.
#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpToolContent {
    Text {
        text: String,
    },
    Image {
        #[serde(rename = "mimeType")]
        mime_type: String,
        data: String,
    },
    Resource {
        uri: String,
        #[serde(default, rename = "mimeType")]
        mime_type: Option<String>,
        #[serde(default)]
        text: Option<String>,
    },
    /// Fallback for content types this client version doesn't model.
    /// Never produced by the internal-tag deserialiser directly — only
    /// by the lossy fallback in `deserialize_content_lossy`.
    #[serde(skip)]
    Other(Value),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_text_concatenates_text_blocks() {
        let r = McpToolResult {
            content: vec![
                McpToolContent::Text {
                    text: "first ".into(),
                },
                McpToolContent::Text {
                    text: "second".into(),
                },
            ],
            is_error: false,
        };
        assert_eq!(r.flatten_text(), "first second");
    }

    #[test]
    fn flatten_text_summarises_image_blocks() {
        let r = McpToolResult {
            content: vec![
                McpToolContent::Text {
                    text: "see: ".into(),
                },
                McpToolContent::Image {
                    mime_type: "image/png".into(),
                    data: "AAAA".into(),
                },
            ],
            is_error: false,
        };
        assert_eq!(r.flatten_text(), "see: <image:image/png>");
    }

    #[test]
    fn mcp_tool_default_schema_when_omitted() {
        let raw = serde_json::json!({"name": "ls"});
        let t: McpTool = serde_json::from_value(raw).unwrap();
        assert_eq!(t.name, "ls");
        assert_eq!(t.input_schema, serde_json::json!({"type": "object"}));
    }

    #[test]
    fn mcp_tool_result_is_error_default_false() {
        let raw = serde_json::json!({"content": [{"type": "text", "text": "ok"}]});
        let r: McpToolResult = serde_json::from_value(raw).unwrap();
        assert!(!r.is_error);
        assert_eq!(r.flatten_text(), "ok");
    }

    #[test]
    fn mcp_tool_result_with_is_error_true() {
        let raw = serde_json::json!({
            "content": [{"type": "text", "text": "boom"}],
            "isError": true
        });
        let r: McpToolResult = serde_json::from_value(raw).unwrap();
        assert!(r.is_error);
    }
}
