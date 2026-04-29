//! Legacy rusqlite free-function API — kept for chat-rpc / chat-server
//! / kangnam-client compatibility while they migrate to the
//! [`crate::Storage`] trait. New code should depend on the trait
//! instead; the [`SqliteStorage`](crate::sqlite_storage::SqliteStorage)
//! impl wraps these same SQL statements.

#![allow(dead_code)]

use rusqlite::{params, Connection};
use uuid::Uuid;

// Re-export the domain types from the shared module so the legacy
// path `kangnam_chat_core::conversations::Conversation` continues to resolve.
pub use crate::types::{Conversation, Message, SearchResult};

fn now_ts() -> i64 {
    crate::storage::now_ts()
}

pub fn list_conversations(conn: &Connection) -> Result<Vec<Conversation>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, title, cli_provider, model, pinned, created_at, updated_at \
         FROM conversations ORDER BY pinned DESC, updated_at DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                title: row.get(1)?,
                cli_provider: row.get(2)?,
                model: row.get(3)?,
                pinned: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn create_conversation(
    conn: &Connection,
    cli_provider: &str,
    model: Option<&str>,
) -> Result<Conversation, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = now_ts();
    conn.execute(
        "INSERT INTO conversations (id, cli_provider, model, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, cli_provider, model, now, now],
    )?;
    Ok(Conversation {
        id,
        title: "New Chat".to_string(),
        cli_provider: cli_provider.to_string(),
        model: model.map(|s| s.to_string()),
        pinned: 0,
        created_at: now,
        updated_at: now,
    })
}

pub fn delete_conversation(conn: &Connection, id: &str) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM conversations WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn delete_all_conversations(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM conversations", [])?;
    Ok(())
}

pub fn get_conversation(conn: &Connection, id: &str) -> Option<Conversation> {
    conn.query_row(
        "SELECT id, title, cli_provider, model, pinned, created_at, updated_at \
         FROM conversations WHERE id = ?1",
        params![id],
        |row| {
            Ok(Conversation {
                id: row.get(0)?,
                title: row.get(1)?,
                cli_provider: row.get(2)?,
                model: row.get(3)?,
                pinned: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        },
    )
    .ok()
}

pub fn update_title(conn: &Connection, id: &str, title: &str) -> Result<(), rusqlite::Error> {
    let now = now_ts();
    conn.execute(
        "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
        params![title, now, id],
    )?;
    Ok(())
}

pub fn toggle_pin(conn: &Connection, id: &str) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE conversations SET pinned = CASE WHEN pinned = 0 THEN 1 ELSE 0 END WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn auto_title_if_needed(
    conn: &Connection,
    conversation_id: &str,
    user_message: &str,
) -> Result<(), rusqlite::Error> {
    if let Some(conv) = get_conversation(conn, conversation_id) {
        if conv.title != "New Chat" {
            return Ok(());
        }
        if let Some(title) = crate::storage::derive_auto_title(user_message) {
            update_title(conn, conversation_id, &title)?;
        }
    }
    Ok(())
}

pub fn get_messages(
    conn: &Connection,
    conversation_id: &str,
) -> Result<Vec<Message>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, role, content, tool_use_id, tool_name, tool_args, \
         token_count, attachments, created_at \
         FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map(params![conversation_id], |row| {
            Ok(Message {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                tool_use_id: row.get(4)?,
                tool_name: row.get(5)?,
                tool_args: row.get(6)?,
                token_count: row.get(7)?,
                attachments: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn add_message(
    conn: &Connection,
    conversation_id: &str,
    role: &str,
    content: &str,
    tool_use_id: Option<&str>,
    token_count: Option<i64>,
    attachments: Option<&str>,
    tool_name: Option<&str>,
    tool_args: Option<&str>,
) -> Result<Message, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = now_ts();
    conn.execute(
        "INSERT INTO messages \
         (id, conversation_id, role, content, tool_use_id, tool_name, tool_args, \
          token_count, attachments, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            id,
            conversation_id,
            role,
            content,
            tool_use_id,
            tool_name,
            tool_args,
            token_count,
            attachments,
            now
        ],
    )?;

    conn.execute(
        "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
        params![now, conversation_id],
    )
    .ok();

    Ok(Message {
        id,
        conversation_id: conversation_id.to_string(),
        role: role.to_string(),
        content: content.to_string(),
        tool_use_id: tool_use_id.map(|s| s.to_string()),
        tool_name: tool_name.map(|s| s.to_string()),
        tool_args: tool_args.map(|s| s.to_string()),
        token_count,
        attachments: attachments.map(|s| s.to_string()),
        created_at: now,
    })
}

pub fn search_messages(
    conn: &Connection,
    query: &str,
) -> Result<Vec<SearchResult>, rusqlite::Error> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(vec![]);
    }
    let pattern = format!("%{trimmed}%");
    let mut stmt = conn.prepare(
        "SELECT m.id, m.conversation_id, c.title, m.content, m.role, m.created_at \
         FROM messages m JOIN conversations c ON c.id = m.conversation_id \
         WHERE m.content LIKE ?1 AND m.role IN ('user', 'assistant') \
         ORDER BY m.created_at DESC LIMIT 50",
    )?;
    let rows = stmt
        .query_map(params![pattern], |row| {
            Ok(SearchResult {
                message_id: row.get(0)?,
                conversation_id: row.get(1)?,
                conversation_title: row.get(2)?,
                content: row.get(3)?,
                role: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;

    fn setup_test_db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        migrations::run(&mut conn).unwrap();
        conn
    }

    #[test]
    fn test_create_and_list() {
        let conn = setup_test_db();
        let conv = create_conversation(&conn, "codex", Some("gpt-4")).unwrap();
        assert_eq!(conv.cli_provider, "codex");
        assert_eq!(conv.title, "New Chat");
        let list = list_conversations(&conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, conv.id);
    }

    #[test]
    fn test_messages() {
        let conn = setup_test_db();
        let conv = create_conversation(&conn, "codex", None).unwrap();
        add_message(&conn, &conv.id, "user", "Hello", None, None, None, None, None).unwrap();
        let msgs = get_messages(&conn, &conv.id).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].content, "Hello");
    }

    #[test]
    fn test_delete_cascades() {
        let conn = setup_test_db();
        let conv = create_conversation(&conn, "codex", None).unwrap();
        add_message(&conn, &conv.id, "user", "test", None, None, None, None, None).unwrap();
        delete_conversation(&conn, &conv.id).unwrap();
        assert_eq!(list_conversations(&conn).unwrap().len(), 0);
        assert_eq!(get_messages(&conn, &conv.id).unwrap().len(), 0);
    }

    #[test]
    fn test_toggle_pin() {
        let conn = setup_test_db();
        let conv = create_conversation(&conn, "codex", None).unwrap();
        toggle_pin(&conn, &conv.id).unwrap();
        assert_eq!(get_conversation(&conn, &conv.id).unwrap().pinned, 1);
        toggle_pin(&conn, &conv.id).unwrap();
        assert_eq!(get_conversation(&conn, &conv.id).unwrap().pinned, 0);
    }

    #[test]
    fn test_search() {
        let conn = setup_test_db();
        let conv = create_conversation(&conn, "codex", None).unwrap();
        add_message(&conn, &conv.id, "user", "Hello world", None, None, None, None, None).unwrap();
        assert_eq!(search_messages(&conn, "Hello").unwrap().len(), 1);
        assert_eq!(search_messages(&conn, "nope").unwrap().len(), 0);
        assert_eq!(search_messages(&conn, "  ").unwrap().len(), 0);
    }

    #[test]
    fn test_update_title() {
        let conn = setup_test_db();
        let conv = create_conversation(&conn, "codex", None).unwrap();
        update_title(&conn, &conv.id, "New Title").unwrap();
        assert_eq!(get_conversation(&conn, &conv.id).unwrap().title, "New Title");
    }

    #[test]
    fn test_delete_all() {
        let conn = setup_test_db();
        create_conversation(&conn, "codex", None).unwrap();
        create_conversation(&conn, "gemini", None).unwrap();
        delete_all_conversations(&conn).unwrap();
        assert_eq!(list_conversations(&conn).unwrap().len(), 0);
    }
}
