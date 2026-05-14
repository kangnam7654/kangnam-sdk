//! Conversation export formatters.
//!
//! Render a conversation + messages into a portable text format that
//! a user can save, share, or pipe into another tool. Currently
//! supports Markdown and pretty-printed JSON.

use rusqlite::Connection;

use crate::conversations::{self, Conversation, Message};

/// Available export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Markdown,
    Json,
}

impl ExportFormat {
    /// Parse a format key (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "markdown" | "md" => Some(Self::Markdown),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

/// Export a conversation by id, looking up the conversation and its
/// messages from `conn` and rendering to `format`.
///
/// Returns `Err` if the conversation doesn't exist, the messages fail
/// to load, or JSON serialization fails.
pub fn export_conversation(
    conn: &Connection,
    id: &str,
    format: ExportFormat,
) -> Result<String, String> {
    let conv = conversations::get_conversation(conn, id).ok_or("Conversation not found")?;
    let messages = conversations::get_messages(conn, id).map_err(|e| e.to_string())?;
    Ok(match format {
        ExportFormat::Markdown => render_markdown(&conv, &messages),
        ExportFormat::Json => render_json(&conv, &messages)?,
    })
}

/// Render a conversation as Markdown. Each message becomes a labeled
/// section separated by `---` rules.
pub fn render_markdown(conv: &Conversation, messages: &[Message]) -> String {
    let mut md = format!("# {}\n\n", conv.title);
    for msg in messages {
        let role = match msg.role.as_str() {
            "user" => "**User**",
            "assistant" => "**Assistant**",
            "system" => "**System**",
            "tool" => "**Tool**",
            _ => msg.role.as_str(),
        };
        md.push_str(&format!("{role}\n\n{}\n\n---\n\n", msg.content));
    }
    md
}

/// Render a conversation as pretty-printed JSON with `conversation`
/// and `messages` top-level keys.
pub fn render_json(conv: &Conversation, messages: &[Message]) -> Result<String, String> {
    serde_json::to_string_pretty(&serde_json::json!({
        "conversation": conv,
        "messages": messages,
    }))
    .map_err(|_| "Failed to serialize conversation data".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversations::{add_message, create_conversation};
    use crate::migrations;

    fn setup() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        migrations::run(&mut conn).unwrap();
        conn
    }

    #[test]
    fn parse_format_keys() {
        assert_eq!(
            ExportFormat::parse("markdown"),
            Some(ExportFormat::Markdown)
        );
        assert_eq!(ExportFormat::parse("MD"), Some(ExportFormat::Markdown));
        assert_eq!(ExportFormat::parse("json"), Some(ExportFormat::Json));
        assert_eq!(ExportFormat::parse("yaml"), None);
    }

    #[test]
    fn markdown_export_contains_title_and_roles() {
        let conn = setup();
        let conv = create_conversation(&conn, "claude", None).unwrap();
        add_message(&conn, &conv.id, "user", "hi", None, None, None, None, None).unwrap();
        add_message(
            &conn,
            &conv.id,
            "assistant",
            "hello",
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let md = export_conversation(&conn, &conv.id, ExportFormat::Markdown).unwrap();
        assert!(md.contains("# New Chat"));
        assert!(md.contains("**User**"));
        assert!(md.contains("**Assistant**"));
        assert!(md.contains("hi"));
        assert!(md.contains("hello"));
    }

    #[test]
    fn json_export_round_trips() {
        let conn = setup();
        let conv = create_conversation(&conn, "codex", None).unwrap();
        add_message(
            &conn, &conv.id, "user", "test", None, None, None, None, None,
        )
        .unwrap();
        let json = export_conversation(&conn, &conv.id, ExportFormat::Json).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v.get("conversation").is_some());
        assert_eq!(v["messages"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn export_missing_conversation_returns_err() {
        let conn = setup();
        let err = export_conversation(&conn, "nonexistent", ExportFormat::Json).unwrap_err();
        assert!(err.contains("not found"));
    }
}
