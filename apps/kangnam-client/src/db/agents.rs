#![allow(dead_code)]

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub model: Option<String>,
    #[serde(rename = "allowedTools")]
    pub allowed_tools: Option<Vec<String>>,
    #[serde(rename = "maxTurns")]
    pub max_turns: i64,
    #[serde(rename = "sortOrder")]
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRun {
    pub id: String,
    #[serde(rename = "agentId")]
    pub agent_id: String,
    #[serde(rename = "conversationId")]
    pub conversation_id: String,
    pub task: String,
    pub result: Option<String>,
    #[serde(rename = "modelUsed")]
    pub model_used: Option<String>,
    pub status: String,
    #[serde(rename = "startedAt")]
    pub started_at: i64,
    #[serde(rename = "completedAt")]
    pub completed_at: Option<i64>,
}

pub fn list_agents(conn: &Connection) -> Result<Vec<Agent>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, instructions, model, allowed_tools, max_turns, sort_order \
         FROM agents ORDER BY sort_order ASC, name ASC",
    )?;
    let agents = stmt
        .query_map([], |row| {
            let tools_json: Option<String> = row.get(5)?;
            let allowed_tools: Option<Vec<String>> = tools_json
                .and_then(|s| serde_json::from_str(&s).ok());
            Ok(Agent {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                instructions: row.get(3)?,
                model: row.get(4)?,
                allowed_tools,
                max_turns: row.get(6)?,
                sort_order: row.get(7)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(agents)
}

pub fn get_agent(conn: &Connection, id: &str) -> Option<Agent> {
    conn.query_row(
        "SELECT id, name, description, instructions, model, allowed_tools, max_turns, sort_order \
         FROM agents WHERE id = ?1",
        params![id],
        |row| {
            let tools_json: Option<String> = row.get(5)?;
            let allowed_tools: Option<Vec<String>> = tools_json
                .and_then(|s| serde_json::from_str(&s).ok());
            Ok(Agent {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                instructions: row.get(3)?,
                model: row.get(4)?,
                allowed_tools,
                max_turns: row.get(6)?,
                sort_order: row.get(7)?,
            })
        },
    )
    .ok()
}

pub fn create_agent(
    conn: &Connection,
    name: &str,
    description: &str,
    instructions: &str,
    model: Option<&str>,
    allowed_tools: Option<Vec<String>>,
    max_turns: i64,
) -> Result<Agent, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    let tools_json = allowed_tools.as_ref().map(|t| serde_json::to_string(t).unwrap_or_default());
    conn.execute(
        "INSERT INTO agents (id, name, description, instructions, model, allowed_tools, max_turns, sort_order, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9)",
        params![id, name, description, instructions, model, tools_json, max_turns, now, now],
    )?;
    Ok(Agent {
        id,
        name: name.to_string(),
        description: description.to_string(),
        instructions: instructions.to_string(),
        model: model.map(|s| s.to_string()),
        allowed_tools,
        max_turns,
        sort_order: 0,
    })
}

pub fn update_agent(
    conn: &Connection,
    id: &str,
    name: &str,
    description: &str,
    instructions: &str,
    model: Option<&str>,
    allowed_tools: Option<Vec<String>>,
    max_turns: i64,
) -> Result<(), rusqlite::Error> {
    let now = chrono::Utc::now().timestamp();
    let tools_json = allowed_tools.as_ref().map(|t| serde_json::to_string(t).unwrap_or_default());
    conn.execute(
        "UPDATE agents SET name = ?1, description = ?2, instructions = ?3, \
         model = ?4, allowed_tools = ?5, max_turns = ?6, updated_at = ?7 WHERE id = ?8",
        params![name, description, instructions, model, tools_json, max_turns, now, id],
    )?;
    Ok(())
}

pub fn delete_agent(conn: &Connection, id: &str) {
    conn.execute("DELETE FROM agents WHERE id = ?1", params![id]).ok();
}

// ── Agent Runs ──

pub fn create_agent_run(
    conn: &Connection,
    agent_id: &str,
    conversation_id: &str,
    task: &str,
) -> Result<AgentRun, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO agent_runs (id, agent_id, conversation_id, task, status, started_at) \
         VALUES (?1, ?2, ?3, ?4, 'running', ?5)",
        params![id, agent_id, conversation_id, task, now],
    )?;
    Ok(AgentRun {
        id,
        agent_id: agent_id.to_string(),
        conversation_id: conversation_id.to_string(),
        task: task.to_string(),
        result: None,
        model_used: None,
        status: "running".to_string(),
        started_at: now,
        completed_at: None,
    })
}

pub fn complete_agent_run(
    conn: &Connection,
    run_id: &str,
    result: &str,
    model_used: Option<&str>,
) -> Result<(), rusqlite::Error> {
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "UPDATE agent_runs SET result = ?1, model_used = ?2, status = 'completed', completed_at = ?3 WHERE id = ?4",
        params![result, model_used, now, run_id],
    )?;
    Ok(())
}

pub fn fail_agent_run(
    conn: &Connection,
    run_id: &str,
    error: &str,
) -> Result<(), rusqlite::Error> {
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "UPDATE agent_runs SET result = ?1, status = 'failed', completed_at = ?2 WHERE id = ?3",
        params![error, now, run_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::run_migrations;

    fn setup_db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        run_migrations(&mut conn).unwrap();
        conn
    }

    #[test]
    fn test_create_and_list_agents() {
        let conn = setup_db();
        let agent = create_agent(&conn, "test-agent", "desc", "instructions", None, None, 10).unwrap();
        assert_eq!(agent.name, "test-agent");
        assert_eq!(agent.max_turns, 10);

        let agents = list_agents(&conn).unwrap();
        // Includes preset agents + our new one
        assert!(agents.iter().any(|a| a.name == "test-agent"));
    }

    #[test]
    fn test_get_agent() {
        let conn = setup_db();
        let agent = create_agent(&conn, "finder", "finds things", "find it", Some("gpt-4"), None, 5).unwrap();
        let found = get_agent(&conn, &agent.id).unwrap();
        assert_eq!(found.name, "finder");
        assert_eq!(found.model, Some("gpt-4".to_string()));
        assert_eq!(found.max_turns, 5);
    }

    #[test]
    fn test_update_agent() {
        let conn = setup_db();
        let agent = create_agent(&conn, "old", "old desc", "old inst", None, None, 10).unwrap();
        update_agent(&conn, &agent.id, "new", "new desc", "new inst", Some("claude"), None, 20).unwrap();
        let updated = get_agent(&conn, &agent.id).unwrap();
        assert_eq!(updated.name, "new");
        assert_eq!(updated.model, Some("claude".to_string()));
        assert_eq!(updated.max_turns, 20);
    }

    #[test]
    fn test_delete_agent() {
        let conn = setup_db();
        let agent = create_agent(&conn, "temp", "temp", "temp", None, None, 10).unwrap();
        delete_agent(&conn, &agent.id);
        assert!(get_agent(&conn, &agent.id).is_none());
    }

    #[test]
    fn test_agent_with_allowed_tools() {
        let conn = setup_db();
        let tools = Some(vec!["read".to_string(), "write".to_string()]);
        let agent = create_agent(&conn, "limited", "desc", "inst", None, tools, 10).unwrap();
        let found = get_agent(&conn, &agent.id).unwrap();
        assert_eq!(found.allowed_tools, Some(vec!["read".to_string(), "write".to_string()]));
    }

    #[test]
    fn test_agent_run_lifecycle() {
        let conn = setup_db();
        // Need a conversation for FK
        conn.execute(
            "INSERT INTO conversations (id, title, cli_provider) VALUES ('conv-1', 'Test', 'test')",
            [],
        ).unwrap();

        let agent = create_agent(&conn, "runner", "desc", "inst", None, None, 10).unwrap();
        let run = create_agent_run(&conn, &agent.id, "conv-1", "do something").unwrap();
        assert_eq!(run.status, "running");

        complete_agent_run(&conn, &run.id, "done!", Some("gpt-4")).unwrap();
        // Verify via raw query
        let status: String = conn.query_row(
            "SELECT status FROM agent_runs WHERE id = ?1",
            params![run.id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(status, "completed");
    }

    #[test]
    fn test_agent_run_fail() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO conversations (id, title, cli_provider) VALUES ('conv-2', 'Test', 'test')",
            [],
        ).unwrap();

        let agent = create_agent(&conn, "failer", "desc", "inst", None, None, 10).unwrap();
        let run = create_agent_run(&conn, &agent.id, "conv-2", "will fail").unwrap();
        fail_agent_run(&conn, &run.id, "something broke").unwrap();

        let status: String = conn.query_row(
            "SELECT status FROM agent_runs WHERE id = ?1",
            params![run.id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(status, "failed");
    }

    #[test]
    fn test_preset_agents_seeded() {
        let conn = setup_db();
        let agents = list_agents(&conn).unwrap();
        let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"code-reviewer"));
        assert!(names.contains(&"researcher"));
        assert!(names.contains(&"translator"));
    }
}
