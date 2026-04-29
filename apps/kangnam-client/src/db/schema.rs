use rusqlite::{Connection, Result};

/// Run all database migrations — mirrors src/main/db/database.ts DDL exactly
pub fn run_migrations(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;

    // ── Core tables ──

    tx.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS conversations (
            id           TEXT PRIMARY KEY,
            title        TEXT NOT NULL DEFAULT 'New Chat',
            cli_provider TEXT NOT NULL DEFAULT 'claude',
            model        TEXT,
            created_at   INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
            updated_at   INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE TABLE IF NOT EXISTS messages (
            id              TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            role            TEXT NOT NULL,
            content         TEXT NOT NULL,
            tool_use_id     TEXT,
            token_count     INTEGER,
            created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_messages_conv
            ON messages(conversation_id, created_at);
        ",
    )?;

    // Migration: add pinned column
    let _ = tx.execute_batch("ALTER TABLE conversations ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0");

    // Migration: add attachments column to messages
    let _ = tx.execute_batch("ALTER TABLE messages ADD COLUMN attachments TEXT");

    // Migration: add tool_name and tool_args columns to messages
    let _ = tx.execute_batch("ALTER TABLE messages ADD COLUMN tool_name TEXT");
    let _ = tx.execute_batch("ALTER TABLE messages ADD COLUMN tool_args TEXT");

    // Migration: add message_type column
    let _ = tx.execute_batch("ALTER TABLE messages ADD COLUMN message_type TEXT NOT NULL DEFAULT 'text'");

    // Migration: rename provider → cli_provider in conversations
    // SQLite does not support RENAME COLUMN before 3.25.0; use ADD + UPDATE pattern
    let _ = tx.execute_batch("ALTER TABLE conversations ADD COLUMN cli_provider TEXT NOT NULL DEFAULT 'claude'");
    let _ = tx.execute_batch("UPDATE conversations SET cli_provider = provider WHERE cli_provider = 'claude' AND provider != ''");
    // Migration: drop legacy provider column (NOT NULL without default blocks new inserts)
    let _ = tx.execute_batch("ALTER TABLE conversations DROP COLUMN provider");

    // ── MCP servers ──

    tx.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS mcp_servers (
            name     TEXT PRIMARY KEY,
            type     TEXT NOT NULL,
            command  TEXT,
            args     TEXT,
            url      TEXT,
            env      TEXT,
            headers  TEXT,
            enabled  INTEGER NOT NULL DEFAULT 1
        );
        ",
    )?;

    // ── Auth tokens ──

    tx.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS auth_tokens (
            provider      TEXT PRIMARY KEY,
            access_token  TEXT NOT NULL,
            refresh_token TEXT,
            expires_at    INTEGER,
            metadata      TEXT
        );
        ",
    )?;

    // ── Prompts (Skills) ──

    tx.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS prompts (
            id          TEXT PRIMARY KEY,
            title       TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            content     TEXT NOT NULL,
            icon        TEXT NOT NULL DEFAULT 'default',
            sort_order  INTEGER NOT NULL DEFAULT 0,
            created_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
            updated_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );
        ",
    )?;

    // Migration: add columns to prompts
    let _ = tx.execute_batch("ALTER TABLE prompts ADD COLUMN description TEXT NOT NULL DEFAULT ''");
    let _ = tx.execute_batch("ALTER TABLE prompts ADD COLUMN argument_hint TEXT");
    let _ = tx.execute_batch("ALTER TABLE prompts ADD COLUMN model TEXT");
    let _ = tx.execute_batch("ALTER TABLE prompts ADD COLUMN user_invocable INTEGER NOT NULL DEFAULT 1");

    // ── Skill references ──

    tx.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS skill_references (
            id          TEXT PRIMARY KEY,
            skill_id    TEXT NOT NULL REFERENCES prompts(id) ON DELETE CASCADE,
            name        TEXT NOT NULL,
            content     TEXT NOT NULL,
            sort_order  INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_skill_refs_skill
            ON skill_references(skill_id, sort_order);
        ",
    )?;

    // ── Agents ──

    tx.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS agents (
            id              TEXT PRIMARY KEY,
            name            TEXT NOT NULL,
            description     TEXT NOT NULL DEFAULT '',
            instructions    TEXT NOT NULL,
            model           TEXT,
            allowed_tools   TEXT CHECK(allowed_tools IS NULL OR json_valid(allowed_tools)),
            max_turns       INTEGER NOT NULL DEFAULT 10,
            sort_order      INTEGER NOT NULL DEFAULT 0,
            created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
            updated_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE TABLE IF NOT EXISTS agent_runs (
            id              TEXT PRIMARY KEY,
            agent_id        TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
            conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            task            TEXT NOT NULL,
            result          TEXT,
            model_used      TEXT,
            status          TEXT NOT NULL DEFAULT 'running' CHECK(status IN ('running', 'completed', 'failed', 'cancelled')),
            started_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
            completed_at    INTEGER
        );

        CREATE INDEX IF NOT EXISTS idx_agent_runs_conv
            ON agent_runs(conversation_id);
        CREATE INDEX IF NOT EXISTS idx_agent_runs_agent
            ON agent_runs(agent_id);
        ",
    )?;

    // ── Eval tables ──

    tx.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS skill_eval_sets (
            id          TEXT PRIMARY KEY,
            skill_id    TEXT NOT NULL REFERENCES prompts(id) ON DELETE CASCADE,
            name        TEXT NOT NULL DEFAULT 'Default',
            created_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
            updated_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_eval_sets_skill
            ON skill_eval_sets(skill_id);

        CREATE TABLE IF NOT EXISTS skill_eval_cases (
            id            TEXT PRIMARY KEY,
            eval_set_id   TEXT NOT NULL REFERENCES skill_eval_sets(id) ON DELETE CASCADE,
            prompt        TEXT NOT NULL,
            expected      TEXT NOT NULL DEFAULT '',
            should_trigger INTEGER NOT NULL DEFAULT 1,
            sort_order    INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_eval_cases_set
            ON skill_eval_cases(eval_set_id, sort_order);

        CREATE TABLE IF NOT EXISTS skill_eval_runs (
            id                TEXT PRIMARY KEY,
            eval_set_id       TEXT NOT NULL REFERENCES skill_eval_sets(id) ON DELETE CASCADE,
            skill_id          TEXT NOT NULL,
            skill_name        TEXT NOT NULL,
            skill_desc        TEXT NOT NULL DEFAULT '',
            skill_body        TEXT NOT NULL DEFAULT '',
            provider          TEXT NOT NULL,
            model             TEXT,
            status            TEXT NOT NULL DEFAULT 'running',
            trigger_accuracy  REAL,
            quality_mean      REAL,
            quality_stddev    REAL,
            total_cases       INTEGER NOT NULL DEFAULT 0,
            completed_cases   INTEGER NOT NULL DEFAULT 0,
            created_at        INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_eval_runs_set
            ON skill_eval_runs(eval_set_id, created_at DESC);

        CREATE TABLE IF NOT EXISTS skill_eval_results (
            id                TEXT PRIMARY KEY,
            run_id            TEXT NOT NULL REFERENCES skill_eval_runs(id) ON DELETE CASCADE,
            case_id           TEXT NOT NULL REFERENCES skill_eval_cases(id) ON DELETE CASCADE,
            did_trigger       INTEGER,
            trigger_correct   INTEGER,
            response_with     TEXT,
            response_without  TEXT,
            quality_score     INTEGER,
            quality_reason    TEXT,
            feedback          TEXT,
            feedback_rating   INTEGER,
            status            TEXT NOT NULL DEFAULT 'pending'
        );

        CREATE INDEX IF NOT EXISTS idx_eval_results_run
            ON skill_eval_results(run_id);
        ",
    )?;

    // ── updated_at triggers ──

    tx.execute_batch(
        "
        CREATE TRIGGER IF NOT EXISTS trg_conversations_updated
            AFTER UPDATE ON conversations
            BEGIN UPDATE conversations SET updated_at = strftime('%s','now') WHERE id = NEW.id; END;

        CREATE TRIGGER IF NOT EXISTS trg_prompts_updated
            AFTER UPDATE ON prompts
            BEGIN UPDATE prompts SET updated_at = strftime('%s','now') WHERE id = NEW.id; END;

        CREATE TRIGGER IF NOT EXISTS trg_agents_updated
            AFTER UPDATE ON agents
            BEGIN UPDATE agents SET updated_at = strftime('%s','now') WHERE id = NEW.id; END;
        ",
    )?;

    tx.commit()?;

    // Seed preset skills (wrapped in transaction for performance)
    seed_preset_skills(conn);

    // Seed preset agents
    seed_preset_agents(conn);

    Ok(())
}

/// Seed preset skills from embedded JSON data (wrapped in transaction for performance)
fn seed_preset_skills(conn: &Connection) {
    let json_data = include_str!("../../data/preset-skills.json");
    let presets: Vec<serde_json::Value> = match serde_json::from_str(json_data) {
        Ok(v) => v,
        Err(_) => return,
    };

    let now = chrono::Utc::now().timestamp();
    let _ = conn.execute_batch("BEGIN");

    // Collect valid preset IDs for cleanup
    let preset_ids: Vec<&str> = presets
        .iter()
        .filter_map(|s| s.get("id").and_then(|v| v.as_str()))
        .filter(|id| !id.is_empty())
        .collect();

    // Remove stale builtin skills no longer in presets
    if !preset_ids.is_empty() {
        let placeholders: Vec<String> = preset_ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
        let sql = format!(
            "DELETE FROM prompts WHERE id LIKE 'builtin-%' AND id NOT IN ({})",
            placeholders.join(", ")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> = preset_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        conn.execute(&sql, params.as_slice()).ok();
    }

    for skill in &presets {
        let id = skill.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if id.is_empty() {
            continue;
        }

        // Check if skill already exists
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM prompts WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;

        if !exists {
            let name = skill.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let description = skill.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let instructions = skill.get("instructions").and_then(|v| v.as_str()).unwrap_or("");
            let sort_order = skill.get("sortOrder").and_then(|v| v.as_i64()).unwrap_or(0);

            conn.execute(
                "INSERT INTO prompts (id, title, description, content, argument_hint, model, user_invocable, sort_order, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, NULL, NULL, 1, ?5, ?6, ?7)",
                rusqlite::params![id, name, description, instructions, sort_order, now, now],
            ).ok();
        }

        // Seed/update references
        if let Some(refs) = skill.get("refs").and_then(|v| v.as_array()) {
            for r in refs {
                let ref_id = r.get("id").and_then(|v| v.as_str()).unwrap_or("");
                if ref_id.is_empty() {
                    continue;
                }
                let ref_name = r.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let ref_content = r.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let ref_sort = r.get("sortOrder").and_then(|v| v.as_i64()).unwrap_or(0);

                let ref_exists: bool = conn
                    .query_row(
                        "SELECT COUNT(*) FROM skill_references WHERE id = ?1",
                        rusqlite::params![ref_id],
                        |row| row.get::<_, i64>(0),
                    )
                    .unwrap_or(0)
                    > 0;

                if ref_exists {
                    conn.execute(
                        "UPDATE skill_references SET name = ?1, content = ?2 WHERE id = ?3",
                        rusqlite::params![ref_name, ref_content, ref_id],
                    ).ok();
                } else {
                    conn.execute(
                        "INSERT INTO skill_references (id, skill_id, name, content, sort_order) VALUES (?1, ?2, ?3, ?4, ?5)",
                        rusqlite::params![ref_id, id, ref_name, ref_content, ref_sort],
                    ).ok();
                }
            }
        }
    }
    let _ = conn.execute_batch("COMMIT");
}

/// Seed preset agents from embedded JSON data (wrapped in transaction for performance)
fn seed_preset_agents(conn: &Connection) {
    let json_data = include_str!("../../data/preset-agents.json");
    let presets: Vec<serde_json::Value> = match serde_json::from_str(json_data) {
        Ok(v) => v,
        Err(_) => return,
    };

    let now = chrono::Utc::now().timestamp();
    let _ = conn.execute_batch("BEGIN");

    // Collect valid preset IDs for cleanup
    let preset_ids: Vec<&str> = presets
        .iter()
        .filter_map(|a| a.get("id").and_then(|v| v.as_str()))
        .filter(|id| !id.is_empty())
        .collect();

    // Remove stale builtin agents no longer in presets
    if !preset_ids.is_empty() {
        let placeholders: Vec<String> = preset_ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
        let sql = format!(
            "DELETE FROM agents WHERE id LIKE 'builtin-%' AND id NOT IN ({})",
            placeholders.join(", ")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> = preset_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        conn.execute(&sql, params.as_slice()).ok();
    }

    for agent in &presets {
        let id = agent.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if id.is_empty() {
            continue;
        }

        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM agents WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;

        if !exists {
            let name = agent.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let description = agent.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let instructions = agent.get("instructions").and_then(|v| v.as_str()).unwrap_or("");
            let model = agent.get("model").and_then(|v| v.as_str());
            let allowed_tools = agent.get("allowedTools").and_then(|v| serde_json::to_string(v).ok());
            let max_turns = agent.get("maxTurns").and_then(|v| v.as_i64()).unwrap_or(10);
            let sort_order = agent.get("sortOrder").and_then(|v| v.as_i64()).unwrap_or(0);

            conn.execute(
                "INSERT INTO agents (id, name, description, instructions, model, allowed_tools, max_turns, sort_order, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![id, name, description, instructions, model, allowed_tools, max_turns, sort_order, now, now],
            ).ok();
        }
    }
    let _ = conn.execute_batch("COMMIT");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_run_twice() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        run_migrations(&mut conn).unwrap();
        // Running twice should be idempotent
        run_migrations(&mut conn).unwrap();
    }

    #[test]
    fn test_tables_exist() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        run_migrations(&mut conn).unwrap();

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"conversations".to_string()));
        assert!(tables.contains(&"messages".to_string()));
        assert!(tables.contains(&"prompts".to_string()));
        assert!(tables.contains(&"auth_tokens".to_string()));
        assert!(tables.contains(&"mcp_servers".to_string()));
        assert!(tables.contains(&"skill_references".to_string()));
        assert!(tables.contains(&"skill_eval_sets".to_string()));
        assert!(tables.contains(&"skill_eval_cases".to_string()));
        assert!(tables.contains(&"skill_eval_runs".to_string()));
        assert!(tables.contains(&"skill_eval_results".to_string()));
        assert!(tables.contains(&"agents".to_string()));
        assert!(tables.contains(&"agent_runs".to_string()));
    }
}
