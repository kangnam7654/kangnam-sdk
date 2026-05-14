//! `Storage` impl backed by sqlx + Postgres.
//!
//! Wraps a `sqlx::PgPool`, which is internally synchronized — the
//! struct is `Clone` and cheap to share. Hosts that already have a
//! pool (lunawave, etc.) can reuse it directly.
//!
//! # Schema
//!
//! [`PostgresStorage::migrate`] creates two tables in the current
//! search_path:
//!
//! - `chat_conversations(id text pk, title text, cli_provider text,
//!   model text, pinned int, created_at bigint, updated_at bigint)`
//! - `chat_messages(id text pk, conversation_id text fk cascade,
//!   role text, content text, tool_use_id text, tool_name text,
//!   tool_args text, token_count bigint, attachments text,
//!   message_type text, created_at bigint)`
//!
//! Names are prefixed with `chat_` so the SDK can drop into hosts
//! whose existing schema already uses `conversations` / `messages`.
//! Hosts that want different names should run their own DDL and
//! skip [`Storage::migrate`].
//!
//! Timestamps stay as `bigint` epoch seconds to match the SQLite
//! schema and the `i64` domain type — no timezone confusion across
//! backends.

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{Result, StorageError};
use crate::storage::{Storage, derive_auto_title, now_ts};
use crate::types::{Conversation, Message, NewMessage, SearchResult};

impl From<sqlx::Error> for StorageError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => StorageError::NotFound,
            other => StorageError::Backend(other.to_string()),
        }
    }
}

#[derive(Clone)]
pub struct PostgresStorage {
    pool: PgPool,
}

impl PostgresStorage {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

const MIGRATION_SQL: &str = include_str!("postgres_migrations.sql");

#[async_trait]
impl Storage for PostgresStorage {
    async fn migrate(&self) -> Result<()> {
        sqlx::raw_sql(MIGRATION_SQL)
            .execute(&self.pool)
            .await
            .map_err(StorageError::from)?;
        Ok(())
    }

    async fn list_conversations(&self) -> Result<Vec<Conversation>> {
        let rows = sqlx::query_as::<_, ConversationRow>(
            "SELECT id, title, cli_provider, model, pinned, created_at, updated_at \
             FROM chat_conversations ORDER BY pinned DESC, updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn create_conversation(
        &self,
        cli_provider: &str,
        model: Option<&str>,
    ) -> Result<Conversation> {
        let id = Uuid::new_v4().to_string();
        let now = now_ts();
        sqlx::query(
            "INSERT INTO chat_conversations \
             (id, title, cli_provider, model, pinned, created_at, updated_at) \
             VALUES ($1, 'New Chat', $2, $3, 0, $4, $4)",
        )
        .bind(&id)
        .bind(cli_provider)
        .bind(model)
        .bind(now)
        .execute(&self.pool)
        .await?;
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

    async fn get_conversation(&self, id: &str) -> Result<Option<Conversation>> {
        let row = sqlx::query_as::<_, ConversationRow>(
            "SELECT id, title, cli_provider, model, pinned, created_at, updated_at \
             FROM chat_conversations WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn conversation_exists(&self, id: &str) -> Result<bool> {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM chat_conversations WHERE id = $1)")
                .bind(id)
                .fetch_one(&self.pool)
                .await?;
        Ok(exists)
    }

    async fn ensure_conversation(&self, id: &str, cli_provider: &str) -> Result<()> {
        let now = now_ts();
        sqlx::query(
            "INSERT INTO chat_conversations \
             (id, title, cli_provider, pinned, created_at, updated_at) \
             VALUES ($1, 'New Chat', $2, 0, $3, $3) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(id)
        .bind(cli_provider)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete_conversation(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM chat_conversations WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete_all_conversations(&self) -> Result<()> {
        sqlx::query("DELETE FROM chat_conversations")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn update_title(&self, id: &str, title: &str) -> Result<()> {
        let now = now_ts();
        sqlx::query("UPDATE chat_conversations SET title = $1, updated_at = $2 WHERE id = $3")
            .bind(title)
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn toggle_pin(&self, id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE chat_conversations SET pinned = CASE WHEN pinned = 0 THEN 1 ELSE 0 END \
             WHERE id = $1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn auto_title_if_needed(&self, conversation_id: &str, user_message: &str) -> Result<()> {
        let Some(new_title) = derive_auto_title(user_message) else {
            return Ok(());
        };
        let now = now_ts();
        // Single-statement guard: only updates rows where title is
        // still the placeholder. Avoids a read-modify-write race.
        sqlx::query(
            "UPDATE chat_conversations \
             SET title = $1, updated_at = $2 \
             WHERE id = $3 AND title = 'New Chat'",
        )
        .bind(&new_title)
        .bind(now)
        .bind(conversation_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_messages(&self, conversation_id: &str) -> Result<Vec<Message>> {
        let rows = sqlx::query_as::<_, MessageRow>(
            "SELECT id, conversation_id, role, content, tool_use_id, tool_name, tool_args, \
             token_count, attachments, created_at \
             FROM chat_messages WHERE conversation_id = $1 ORDER BY created_at ASC",
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn add_message(&self, conversation_id: &str, msg: NewMessage<'_>) -> Result<Message> {
        let id = Uuid::new_v4().to_string();
        let now = now_ts();

        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO chat_messages \
             (id, conversation_id, role, content, tool_use_id, tool_name, tool_args, \
              token_count, attachments, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(&id)
        .bind(conversation_id)
        .bind(msg.role)
        .bind(msg.content)
        .bind(msg.tool_use_id)
        .bind(msg.tool_name)
        .bind(msg.tool_args)
        .bind(msg.token_count)
        .bind(msg.attachments)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        // Best-effort updated_at bump. Mirrors the SQLite path which
        // uses `.ok()` to ignore failures (e.g. orphan FK).
        let _ = sqlx::query("UPDATE chat_conversations SET updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(conversation_id)
            .execute(&mut *tx)
            .await;

        tx.commit().await?;

        Ok(Message {
            id,
            conversation_id: conversation_id.to_string(),
            role: msg.role.to_string(),
            content: msg.content.to_string(),
            tool_use_id: msg.tool_use_id.map(|s| s.to_string()),
            tool_name: msg.tool_name.map(|s| s.to_string()),
            tool_args: msg.tool_args.map(|s| s.to_string()),
            token_count: msg.token_count,
            attachments: msg.attachments.map(|s| s.to_string()),
            created_at: now,
        })
    }

    async fn search_messages(&self, query: &str) -> Result<Vec<SearchResult>> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(vec![]);
        }
        let pattern = format!("%{trimmed}%");
        let rows = sqlx::query_as::<_, SearchRow>(
            "SELECT m.id AS message_id, m.conversation_id, c.title AS conversation_title, \
             m.content, m.role, m.created_at \
             FROM chat_messages m \
             JOIN chat_conversations c ON c.id = m.conversation_id \
             WHERE m.content ILIKE $1 AND m.role IN ('user', 'assistant') \
             ORDER BY m.created_at DESC LIMIT 50",
        )
        .bind(pattern)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}

// -- sqlx FromRow shims --
//
// sqlx::FromRow can't derive directly onto types that other crates
// depend on without tying chat-core's public API to sqlx. So we have
// internal `*Row` structs with `FromRow` and convert to the public
// domain types.

#[derive(sqlx::FromRow)]
struct ConversationRow {
    id: String,
    title: String,
    cli_provider: String,
    model: Option<String>,
    pinned: i32,
    created_at: i64,
    updated_at: i64,
}

impl From<ConversationRow> for Conversation {
    fn from(r: ConversationRow) -> Self {
        Conversation {
            id: r.id,
            title: r.title,
            cli_provider: r.cli_provider,
            model: r.model,
            pinned: r.pinned as i64,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: String,
    conversation_id: String,
    role: String,
    content: String,
    tool_use_id: Option<String>,
    tool_name: Option<String>,
    tool_args: Option<String>,
    token_count: Option<i64>,
    attachments: Option<String>,
    created_at: i64,
}

impl From<MessageRow> for Message {
    fn from(r: MessageRow) -> Self {
        Message {
            id: r.id,
            conversation_id: r.conversation_id,
            role: r.role,
            content: r.content,
            tool_use_id: r.tool_use_id,
            tool_name: r.tool_name,
            tool_args: r.tool_args,
            token_count: r.token_count,
            attachments: r.attachments,
            created_at: r.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct SearchRow {
    message_id: String,
    conversation_id: String,
    conversation_title: String,
    content: String,
    role: String,
    created_at: i64,
}

impl From<SearchRow> for SearchResult {
    fn from(r: SearchRow) -> Self {
        SearchResult {
            message_id: r.message_id,
            conversation_id: r.conversation_id,
            conversation_title: r.conversation_title,
            content: r.content,
            role: r.role,
            created_at: r.created_at,
        }
    }
}
