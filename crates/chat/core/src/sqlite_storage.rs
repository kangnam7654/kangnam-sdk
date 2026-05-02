//! `Storage` impl backed by rusqlite.
//!
//! Wraps an `Arc<Mutex<Connection>>` and dispatches each call onto
//! `tokio::task::spawn_blocking` so the sync rusqlite API fits the
//! async trait. Trade-off: every op pays one tokio hop, but we avoid
//! the complexity of switching to `rusqlite::Connection` per-task or
//! using a separate worker thread.
//!
//! The mutex is `std::sync::Mutex` (not tokio's) because we never
//! hold it across `.await` — each closure inside `spawn_blocking`
//! grabs the lock, does its SQL, drops it.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::{Result, StorageError};
use crate::storage::{derive_auto_title, now_ts, Storage};
use crate::types::{Conversation, Message, NewMessage, SearchResult};

impl From<rusqlite::Error> for StorageError {
    fn from(e: rusqlite::Error) -> Self {
        match e {
            rusqlite::Error::QueryReturnedNoRows => StorageError::NotFound,
            other => StorageError::Backend(other.to_string()),
        }
    }
}

#[derive(Clone)]
pub struct SqliteStorage {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStorage {
    /// Wrap an existing connection. The caller is responsible for
    /// running [`crate::migrations::run`] beforehand or calling
    /// [`Storage::migrate`] on the returned value.
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Open an in-memory SQLite database. Useful for tests.
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(StorageError::from)?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(StorageError::from)?;
        Ok(Self::new(Arc::new(Mutex::new(conn))))
    }

    /// Borrow the underlying connection mutex. Lets hosts share the
    /// same handle with legacy `conversations::*` free-function
    /// callers during migration.
    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }
}

async fn block_on<F, T>(f: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| StorageError::Backend(format!("join error: {e}")))?
}

#[async_trait]
impl Storage for SqliteStorage {
    async fn migrate(&self) -> Result<()> {
        let conn = self.conn.clone();
        block_on(move || {
            let mut guard = conn
                .lock()
                .map_err(|e| StorageError::Backend(format!("mutex poisoned: {e}")))?;
            crate::migrations::run(&mut guard).map_err(StorageError::from)
        })
        .await
    }

    async fn list_conversations(&self) -> Result<Vec<Conversation>> {
        let conn = self.conn.clone();
        block_on(move || {
            let guard = conn
                .lock()
                .map_err(|e| StorageError::Backend(format!("mutex poisoned: {e}")))?;
            let mut stmt = guard.prepare(
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
        })
        .await
    }

    async fn create_conversation(
        &self,
        cli_provider: &str,
        model: Option<&str>,
    ) -> Result<Conversation> {
        let provider = cli_provider.to_string();
        let model = model.map(|s| s.to_string());
        let conn = self.conn.clone();
        block_on(move || {
            let guard = conn
                .lock()
                .map_err(|e| StorageError::Backend(format!("mutex poisoned: {e}")))?;
            let id = Uuid::new_v4().to_string();
            let now = now_ts();
            guard.execute(
                "INSERT INTO conversations (id, cli_provider, model, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, provider, model, now, now],
            )?;
            Ok(Conversation {
                id,
                title: "New Chat".to_string(),
                cli_provider: provider,
                model,
                pinned: 0,
                created_at: now,
                updated_at: now,
            })
        })
        .await
    }

    async fn get_conversation(&self, id: &str) -> Result<Option<Conversation>> {
        let id = id.to_string();
        self.with_conn_async(move |conn| {
            match conn.query_row(
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
            ) {
                Ok(c) => Ok(Some(c)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
        .await
    }

    async fn conversation_exists(&self, id: &str) -> Result<bool> {
        let id = id.to_string();
        self.with_conn_async(move |conn| {
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
                params![id],
                |r| r.get::<_, bool>(0),
            )
        })
        .await
    }

    async fn ensure_conversation(&self, id: &str, cli_provider: &str) -> Result<()> {
        let id = id.to_string();
        let provider = cli_provider.to_string();
        self.with_conn_async(move |conn| {
            let now = now_ts();
            conn.execute(
                "INSERT OR IGNORE INTO conversations \
                 (id, title, cli_provider, created_at, updated_at) \
                 VALUES (?1, 'New Chat', ?2, ?3, ?4)",
                params![id, provider, now, now],
            )?;
            Ok(())
        })
        .await
    }

    async fn delete_conversation(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.with_conn_async(move |conn| {
            conn.execute("DELETE FROM conversations WHERE id = ?1", params![id])?;
            Ok(())
        })
        .await
    }

    async fn delete_all_conversations(&self) -> Result<()> {
        self.with_conn_async(move |conn| {
            conn.execute("DELETE FROM conversations", [])?;
            Ok(())
        })
        .await
    }

    async fn update_title(&self, id: &str, title: &str) -> Result<()> {
        let id = id.to_string();
        let title = title.to_string();
        self.with_conn_async(move |conn| {
            let now = now_ts();
            conn.execute(
                "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
                params![title, now, id],
            )?;
            Ok(())
        })
        .await
    }

    async fn toggle_pin(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.with_conn_async(move |conn| {
            conn.execute(
                "UPDATE conversations SET pinned = CASE WHEN pinned = 0 THEN 1 ELSE 0 END \
                 WHERE id = ?1",
                params![id],
            )?;
            Ok(())
        })
        .await
    }

    async fn auto_title_if_needed(
        &self,
        conversation_id: &str,
        user_message: &str,
    ) -> Result<()> {
        let derived = derive_auto_title(user_message);
        let conversation_id = conversation_id.to_string();
        self.with_conn_async(move |conn| {
            let title: Option<String> = conn
                .query_row(
                    "SELECT title FROM conversations WHERE id = ?1",
                    params![conversation_id],
                    |r| r.get(0),
                )
                .ok();
            let needs_update = matches!(title.as_deref(), Some("New Chat"));
            if needs_update {
                if let Some(new_title) = derived {
                    let now = now_ts();
                    conn.execute(
                        "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
                        params![new_title, now, conversation_id],
                    )?;
                }
            }
            Ok(())
        })
        .await
    }

    async fn get_messages(&self, conversation_id: &str) -> Result<Vec<Message>> {
        let conversation_id = conversation_id.to_string();
        self.with_conn_async(move |conn| {
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
        })
        .await
    }

    async fn add_message(
        &self,
        conversation_id: &str,
        msg: NewMessage<'_>,
    ) -> Result<Message> {
        // Owned copies — the closure outlives `'_`.
        let conversation_id = conversation_id.to_string();
        let role = msg.role.to_string();
        let content = msg.content.to_string();
        let tool_use_id = msg.tool_use_id.map(|s| s.to_string());
        let tool_name = msg.tool_name.map(|s| s.to_string());
        let tool_args = msg.tool_args.map(|s| s.to_string());
        let token_count = msg.token_count;
        let attachments = msg.attachments.map(|s| s.to_string());

        self.with_conn_async(move |conn| {
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
            let _ = conn.execute(
                "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
                params![now, conversation_id],
            );
            Ok(Message {
                id,
                conversation_id,
                role,
                content,
                tool_use_id,
                tool_name,
                tool_args,
                token_count,
                attachments,
                created_at: now,
            })
        })
        .await
    }

    async fn search_messages(&self, query: &str) -> Result<Vec<SearchResult>> {
        let trimmed = query.trim().to_string();
        if trimmed.is_empty() {
            return Ok(vec![]);
        }
        self.with_conn_async(move |conn| {
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
        })
        .await
    }
}

impl SqliteStorage {
    /// Spawn a blocking task that holds the connection lock for the
    /// duration of one rusqlite call. The closure receives a borrowed
    /// `&Connection` so it can use `conn.execute` / `conn.query_row`
    /// directly without re-locking.
    async fn with_conn_async<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> std::result::Result<T, rusqlite::Error> + Send + 'static,
        T: Send + 'static,
    {
        let conn = self.conn.clone();
        block_on(move || {
            let guard = conn
                .lock()
                .map_err(|e| StorageError::Backend(format!("mutex poisoned: {e}")))?;
            f(&guard).map_err(StorageError::from)
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn fresh() -> SqliteStorage {
        let s = SqliteStorage::in_memory().unwrap();
        s.migrate().await.unwrap();
        s
    }

    #[tokio::test]
    async fn create_and_list() {
        let s = fresh().await;
        let conv = s.create_conversation("codex", Some("gpt-4")).await.unwrap();
        assert_eq!(conv.title, "New Chat");
        let list = s.list_conversations().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, conv.id);
    }

    #[tokio::test]
    async fn add_and_fetch_messages() {
        let s = fresh().await;
        let conv = s.create_conversation("codex", None).await.unwrap();
        s.add_message(&conv.id, NewMessage::user("hello"))
            .await
            .unwrap();
        let msgs = s.get_messages(&conv.id).await.unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].content, "hello");
    }

    #[tokio::test]
    async fn ensure_is_idempotent() {
        let s = fresh().await;
        s.ensure_conversation("conv-1", "claude").await.unwrap();
        s.ensure_conversation("conv-1", "claude").await.unwrap();
        assert!(s.conversation_exists("conv-1").await.unwrap());
        assert_eq!(s.list_conversations().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn auto_title_picks_first_line() {
        let s = fresh().await;
        s.ensure_conversation("c1", "claude").await.unwrap();
        s.auto_title_if_needed("c1", "Hello there\nsecond line")
            .await
            .unwrap();
        let conv = s.get_conversation("c1").await.unwrap().unwrap();
        assert_eq!(conv.title, "Hello there");
    }

    #[tokio::test]
    async fn auto_title_skips_already_titled() {
        let s = fresh().await;
        s.ensure_conversation("c1", "claude").await.unwrap();
        s.update_title("c1", "Custom").await.unwrap();
        s.auto_title_if_needed("c1", "Should not overwrite")
            .await
            .unwrap();
        let conv = s.get_conversation("c1").await.unwrap().unwrap();
        assert_eq!(conv.title, "Custom");
    }

    #[tokio::test]
    async fn search_finds_user_text() {
        let s = fresh().await;
        let conv = s.create_conversation("codex", None).await.unwrap();
        s.add_message(&conv.id, NewMessage::user("Hello world"))
            .await
            .unwrap();
        assert_eq!(s.search_messages("Hello").await.unwrap().len(), 1);
        assert_eq!(s.search_messages("nope").await.unwrap().len(), 0);
        assert_eq!(s.search_messages("  ").await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn delete_cascades() {
        let s = fresh().await;
        let conv = s.create_conversation("codex", None).await.unwrap();
        s.add_message(&conv.id, NewMessage::user("x")).await.unwrap();
        s.delete_conversation(&conv.id).await.unwrap();
        assert_eq!(s.list_conversations().await.unwrap().len(), 0);
        assert_eq!(s.get_messages(&conv.id).await.unwrap().len(), 0);
    }
}
