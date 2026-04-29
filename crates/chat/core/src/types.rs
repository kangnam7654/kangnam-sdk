//! Pure domain types shared by every storage backend.
//!
//! These structs cross the Storage trait boundary, the JSON-RPC wire,
//! and the export formatters — so they have no DB-specific imports.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub cli_provider: String,
    pub model: Option<String>,
    pub pinned: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub tool_use_id: Option<String>,
    pub tool_name: Option<String>,
    pub tool_args: Option<String>,
    pub token_count: Option<i64>,
    pub attachments: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    #[serde(rename = "messageId")]
    pub message_id: String,
    #[serde(rename = "conversationId")]
    pub conversation_id: String,
    #[serde(rename = "conversationTitle")]
    pub conversation_title: String,
    pub content: String,
    pub role: String,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
}

/// New-message field bundle. Avoids the 8-positional-argument signature
/// of the legacy free function.
#[derive(Debug, Clone, Default)]
pub struct NewMessage<'a> {
    pub role: &'a str,
    pub content: &'a str,
    pub tool_use_id: Option<&'a str>,
    pub tool_name: Option<&'a str>,
    pub tool_args: Option<&'a str>,
    pub token_count: Option<i64>,
    pub attachments: Option<&'a str>,
}

impl<'a> NewMessage<'a> {
    pub fn user(content: &'a str) -> Self {
        Self { role: "user", content, ..Self::default() }
    }
    pub fn assistant(content: &'a str) -> Self {
        Self { role: "assistant", content, ..Self::default() }
    }
}
