-- harness-session-core Postgres schema. Idempotent — safe to run on every boot.
--
-- Tables are prefixed with `chat_` to avoid collisions with host
-- applications that already use `conversations` / `messages` for
-- unrelated domains. Hosts that want different names should manage
-- their own DDL and skip Storage::migrate().
--
-- Timestamps are stored as BIGINT epoch seconds to match the SQLite
-- backend and the `i64` domain type. Switch to TIMESTAMPTZ in a
-- future migration if needed.

CREATE TABLE IF NOT EXISTS chat_conversations (
    id           TEXT PRIMARY KEY,
    title        TEXT NOT NULL DEFAULT 'New Chat',
    cli_provider TEXT NOT NULL DEFAULT 'claude',
    model        TEXT,
    pinned       INTEGER NOT NULL DEFAULT 0,
    created_at   BIGINT  NOT NULL DEFAULT EXTRACT(EPOCH FROM now())::bigint,
    updated_at   BIGINT  NOT NULL DEFAULT EXTRACT(EPOCH FROM now())::bigint
);

CREATE TABLE IF NOT EXISTS chat_messages (
    id              TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES chat_conversations(id) ON DELETE CASCADE,
    role            TEXT NOT NULL,
    content         TEXT NOT NULL,
    tool_use_id     TEXT,
    tool_name       TEXT,
    tool_args       TEXT,
    token_count     BIGINT,
    attachments     TEXT,
    message_type    TEXT NOT NULL DEFAULT 'text',
    created_at      BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM now())::bigint
);

CREATE INDEX IF NOT EXISTS idx_chat_messages_conv
    ON chat_messages(conversation_id, created_at);
