//! SQLite-backed [`HarnessStore`] implementation.
//!
//! Resources are stored one-row-per-id with the full record serialized as a
//! JSON blob. This trades column-level queryability for a stable schema that
//! evolves with [`harness_core`] type changes — most reads are list-by-scope
//! or get-by-id, both of which the (id, scope) index covers.

use std::path::Path;
use std::sync::Mutex;

use async_trait::async_trait;
use kangnam_harness_core::{Agent, Hook, HookEvent, Permission, Scope, Skill, Tool};
use rusqlite::{Connection, params};
use serde::{Serialize, de::DeserializeOwned};

use crate::{HarnessStore, Result, StoreError};

/// SQLite-backed store. Cheap to clone if you need to hand out references —
/// wrap the whole struct in `Arc`.
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    /// Open or create a database at the given path and run migrations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    /// Open an in-memory store, useful for tests.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS harness_tools (
                id    TEXT PRIMARY KEY,
                scope TEXT NOT NULL,
                data  TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS harness_skills (
                id    TEXT PRIMARY KEY,
                scope TEXT NOT NULL,
                data  TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS harness_hooks (
                id    TEXT PRIMARY KEY,
                scope TEXT NOT NULL,
                event TEXT NOT NULL,
                data  TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS harness_agents (
                id    TEXT PRIMARY KEY,
                scope TEXT NOT NULL,
                data  TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS harness_permissions (
                id    TEXT PRIMARY KEY,
                scope TEXT NOT NULL,
                data  TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_harness_tools_scope       ON harness_tools(scope);
            CREATE INDEX IF NOT EXISTS idx_harness_skills_scope      ON harness_skills(scope);
            CREATE INDEX IF NOT EXISTS idx_harness_hooks_event_scope ON harness_hooks(event, scope);
            CREATE INDEX IF NOT EXISTS idx_harness_agents_scope      ON harness_agents(scope);
            CREATE INDEX IF NOT EXISTS idx_harness_permissions_scope ON harness_permissions(scope);
            "#,
        )?;
        Ok(())
    }
}

fn scope_str(scope: Scope) -> &'static str {
    match scope {
        Scope::User => "user",
        Scope::Project => "project",
        Scope::Local => "local",
    }
}

fn upsert_blob<T: Serialize>(
    conn: &Connection,
    table: &str,
    id: &str,
    scope: Scope,
    record: &T,
) -> Result<()> {
    let data = serde_json::to_string(record)?;
    let sql = format!(
        "INSERT INTO {table} (id, scope, data) VALUES (?1, ?2, ?3)
         ON CONFLICT(id) DO UPDATE SET scope = excluded.scope, data = excluded.data"
    );
    conn.execute(&sql, params![id, scope_str(scope), data])?;
    Ok(())
}

fn list_blobs<T: DeserializeOwned>(
    conn: &Connection,
    table: &str,
    scope: Option<Scope>,
) -> Result<Vec<T>> {
    let (sql, scope_param) = match scope {
        Some(s) => (
            format!("SELECT data FROM {table} WHERE scope = ?1 ORDER BY id"),
            Some(scope_str(s)),
        ),
        None => (format!("SELECT data FROM {table} ORDER BY id"), None),
    };
    let mut stmt = conn.prepare(&sql)?;
    let rows = if let Some(s) = scope_param {
        stmt.query_map(params![s], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?
    } else {
        stmt.query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    rows.into_iter()
        .map(|s| serde_json::from_str::<T>(&s).map_err(StoreError::from))
        .collect()
}

fn get_blob<T: DeserializeOwned>(conn: &Connection, table: &str, id: &str) -> Result<Option<T>> {
    let sql = format!("SELECT data FROM {table} WHERE id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        let data: String = row.get(0)?;
        Ok(Some(serde_json::from_str(&data)?))
    } else {
        Ok(None)
    }
}

fn delete_row(conn: &Connection, table: &str, id: &str) -> Result<()> {
    let sql = format!("DELETE FROM {table} WHERE id = ?1");
    conn.execute(&sql, params![id])?;
    Ok(())
}

#[async_trait]
impl HarnessStore for SqliteStore {
    // ── Tools ──
    async fn list_tools(&self, scope: Option<Scope>) -> Result<Vec<Tool>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        list_blobs(&conn, "harness_tools", scope)
    }
    async fn get_tool(&self, id: &str) -> Result<Option<Tool>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        get_blob(&conn, "harness_tools", id)
    }
    async fn upsert_tool(&self, tool: Tool) -> Result<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        upsert_blob(&conn, "harness_tools", &tool.id, tool.scope, &tool)
    }
    async fn delete_tool(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        delete_row(&conn, "harness_tools", id)
    }

    // ── Skills ──
    async fn list_skills(&self, scope: Option<Scope>) -> Result<Vec<Skill>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        list_blobs(&conn, "harness_skills", scope)
    }
    async fn get_skill(&self, id: &str) -> Result<Option<Skill>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        get_blob(&conn, "harness_skills", id)
    }
    async fn upsert_skill(&self, skill: Skill) -> Result<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        upsert_blob(&conn, "harness_skills", &skill.id, skill.scope, &skill)
    }
    async fn delete_skill(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        delete_row(&conn, "harness_skills", id)
    }

    // ── Hooks ──
    async fn list_hooks(
        &self,
        event: Option<HookEvent>,
        scope: Option<Scope>,
    ) -> Result<Vec<Hook>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let event_str = event
            .map(|e| serde_json::to_value(e))
            .transpose()?
            .and_then(|v| v.as_str().map(str::to_owned));
        let scope_param = scope.map(scope_str);

        let (sql, params_vec): (String, Vec<Box<dyn rusqlite::ToSql>>) =
            match (&event_str, scope_param) {
                (Some(e), Some(s)) => (
                    "SELECT data FROM harness_hooks WHERE event = ?1 AND scope = ?2 ORDER BY id"
                        .into(),
                    vec![Box::new(e.clone()), Box::new(s.to_string())],
                ),
                (Some(e), None) => (
                    "SELECT data FROM harness_hooks WHERE event = ?1 ORDER BY id".into(),
                    vec![Box::new(e.clone())],
                ),
                (None, Some(s)) => (
                    "SELECT data FROM harness_hooks WHERE scope = ?1 ORDER BY id".into(),
                    vec![Box::new(s.to_string())],
                ),
                (None, None) => ("SELECT data FROM harness_hooks ORDER BY id".into(), vec![]),
            };

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params_vec
            .iter()
            .map(|p| p.as_ref() as &dyn rusqlite::ToSql)
            .collect();
        let rows: Vec<String> = stmt
            .query_map(param_refs.as_slice(), |row| row.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        rows.into_iter()
            .map(|s| serde_json::from_str(&s).map_err(StoreError::from))
            .collect()
    }
    async fn get_hook(&self, id: &str) -> Result<Option<Hook>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        get_blob(&conn, "harness_hooks", id)
    }
    async fn upsert_hook(&self, hook: Hook) -> Result<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let event_str = serde_json::to_value(hook.event)?
            .as_str()
            .ok_or_else(|| StoreError::Other("HookEvent did not serialize to string".into()))?
            .to_owned();
        let data = serde_json::to_string(&hook)?;
        conn.execute(
            "INSERT INTO harness_hooks (id, scope, event, data) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET scope = excluded.scope, event = excluded.event, data = excluded.data",
            params![hook.id, scope_str(hook.scope), event_str, data],
        )?;
        Ok(())
    }
    async fn delete_hook(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        delete_row(&conn, "harness_hooks", id)
    }

    // ── Agents ──
    async fn list_agents(&self, scope: Option<Scope>) -> Result<Vec<Agent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        list_blobs(&conn, "harness_agents", scope)
    }
    async fn get_agent(&self, id: &str) -> Result<Option<Agent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        get_blob(&conn, "harness_agents", id)
    }
    async fn upsert_agent(&self, agent: Agent) -> Result<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        upsert_blob(&conn, "harness_agents", &agent.id, agent.scope, &agent)
    }
    async fn delete_agent(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        delete_row(&conn, "harness_agents", id)
    }

    // ── Permissions ──
    async fn list_permissions(&self, scope: Option<Scope>) -> Result<Vec<Permission>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        list_blobs(&conn, "harness_permissions", scope)
    }
    async fn upsert_permission(&self, permission: Permission) -> Result<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        upsert_blob(
            &conn,
            "harness_permissions",
            &permission.id,
            permission.scope,
            &permission,
        )
    }
    async fn delete_permission(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        delete_row(&conn, "harness_permissions", id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kangnam_harness_core::{BuiltinTool, ToolSource};

    #[tokio::test]
    async fn roundtrip_tool() {
        let store = SqliteStore::open_in_memory().unwrap();
        let tool = Tool {
            id: "read".into(),
            name: "Read".into(),
            description: "Read a file".into(),
            parameters: serde_json::json!({}),
            source: ToolSource::Builtin(BuiltinTool::Read),
            scope: Scope::User,
        };
        store.upsert_tool(tool.clone()).await.unwrap();

        let fetched = store.get_tool("read").await.unwrap().unwrap();
        assert_eq!(fetched.name, "Read");

        let listed = store.list_tools(Some(Scope::User)).await.unwrap();
        assert_eq!(listed.len(), 1);

        store.delete_tool("read").await.unwrap();
        assert!(store.get_tool("read").await.unwrap().is_none());
    }
}
