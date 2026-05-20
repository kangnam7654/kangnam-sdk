//! Minimal idempotent DDL for the chat tables.
//!
//! Run this once at app startup. Safe to call repeatedly — every statement is
//! `IF NOT EXISTS`. If a host application defines a superset schema, calling
//! this is harmless because the host's `CREATE TABLE IF NOT EXISTS` will
//! short-circuit on already-created tables.

use rusqlite::{Connection, Result};

pub fn run(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;

    tx.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS conversations (
            id           TEXT PRIMARY KEY,
            title        TEXT NOT NULL DEFAULT 'New Chat',
            cli_provider TEXT NOT NULL DEFAULT 'claude',
            model        TEXT,
            pinned       INTEGER NOT NULL DEFAULT 0,
            created_at   INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
            updated_at   INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE TABLE IF NOT EXISTS messages (
            id              TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            role            TEXT NOT NULL,
            content         TEXT NOT NULL,
            tool_use_id     TEXT,
            tool_name       TEXT,
            tool_args       TEXT,
            token_count     INTEGER,
            attachments     TEXT,
            message_type    TEXT NOT NULL DEFAULT 'text',
            created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_messages_conv
            ON messages(conversation_id, created_at);

        CREATE TRIGGER IF NOT EXISTS trg_conversations_updated
            AFTER UPDATE ON conversations
            BEGIN UPDATE conversations SET updated_at = strftime('%s','now') WHERE id = NEW.id; END;
        ",
    )?;

    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_is_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        run(&mut conn).unwrap();
        run(&mut conn).unwrap();
    }
}
