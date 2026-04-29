use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillReference {
    pub id: String,
    #[serde(rename = "skillId")]
    pub skill_id: String,
    pub name: String,
    pub content: String,
    #[serde(rename = "sortOrder")]
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub instructions: String,
    #[serde(rename = "argumentHint")]
    pub argument_hint: Option<String>,
    pub model: Option<String>,
    #[serde(rename = "userInvocable")]
    pub user_invocable: bool,
    pub references: Vec<SkillReference>,
}

fn get_refs_for_skill(
    conn: &Connection,
    skill_id: &str,
) -> Result<Vec<SkillReference>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, skill_id, name, content, sort_order \
         FROM skill_references WHERE skill_id = ?1 ORDER BY sort_order ASC",
    )?;
    let rows = stmt
        .query_map(params![skill_id], |row| {
            Ok(SkillReference {
                id: row.get(0)?,
                skill_id: row.get(1)?,
                name: row.get(2)?,
                content: row.get(3)?,
                sort_order: row.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn list_skills(conn: &Connection) -> Result<Vec<Skill>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, title, description, content, argument_hint, model, user_invocable \
         FROM prompts ORDER BY sort_order ASC, title ASC",
    )?;
    let skills: Vec<(String, String, String, String, Option<String>, Option<String>, i64)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Batch-load all references
    let mut ref_stmt = conn.prepare(
        "SELECT id, skill_id, name, content, sort_order \
         FROM skill_references ORDER BY sort_order ASC",
    )?;
    let all_refs: Vec<SkillReference> = ref_stmt
        .query_map([], |row| {
            Ok(SkillReference {
                id: row.get(0)?,
                skill_id: row.get(1)?,
                name: row.get(2)?,
                content: row.get(3)?,
                sort_order: row.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    let mut refs_by_skill: std::collections::HashMap<String, Vec<SkillReference>> =
        std::collections::HashMap::new();
    for r in all_refs {
        refs_by_skill.entry(r.skill_id.clone()).or_default().push(r);
    }

    let result = skills
        .into_iter()
        .map(|(id, title, desc, content, hint, model, invocable)| Skill {
            references: refs_by_skill.remove(&id).unwrap_or_default(),
            id,
            name: title,
            description: desc,
            instructions: content,
            argument_hint: hint,
            model,
            user_invocable: invocable == 1,
        })
        .collect();
    Ok(result)
}

pub fn get_skill(conn: &Connection, id: &str) -> Option<Skill> {
    conn.query_row(
        "SELECT id, title, description, content, argument_hint, model, user_invocable \
         FROM prompts WHERE id = ?1",
        params![id],
        |row| {
            let skill_id: String = row.get(0)?;
            Ok(Skill {
                id: skill_id,
                name: row.get(1)?,
                description: row.get(2)?,
                instructions: row.get(3)?,
                argument_hint: row.get(4)?,
                model: row.get(5)?,
                user_invocable: row.get::<_, i64>(6)? == 1,
                references: vec![], // filled below
            })
        },
    )
    .ok()
    .map(|mut s| {
        s.references = get_refs_for_skill(conn, &s.id).unwrap_or_default();
        s
    })
}

pub fn get_skill_instructions(conn: &Connection, id: &str) -> Option<String> {
    let skill = get_skill(conn, id)?;
    let mut text = skill.instructions;
    if !skill.references.is_empty() {
        text.push_str("\n\n---\n\n");
        let refs: Vec<String> = skill
            .references
            .iter()
            .map(|r| format!("## {}\n\n{}", r.name, r.content))
            .collect();
        text.push_str(&refs.join("\n\n---\n\n"));
    }
    Some(text)
}

pub fn create_skill(
    conn: &Connection,
    name: &str,
    description: &str,
    instructions: &str,
    argument_hint: Option<&str>,
    model: Option<&str>,
    user_invocable: Option<bool>,
) -> Result<Skill, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    let invocable = user_invocable.unwrap_or(true);
    conn.execute(
        "INSERT INTO prompts \
         (id, title, description, content, argument_hint, model, user_invocable, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            name,
            description,
            instructions,
            argument_hint,
            model,
            if invocable { 1 } else { 0 },
            now,
            now
        ],
    )?;
    Ok(Skill {
        id,
        name: name.to_string(),
        description: description.to_string(),
        instructions: instructions.to_string(),
        argument_hint: argument_hint.map(|s| s.to_string()),
        model: model.map(|s| s.to_string()),
        user_invocable: invocable,
        references: vec![],
    })
}

pub fn update_skill(
    conn: &Connection,
    id: &str,
    name: &str,
    description: &str,
    instructions: &str,
    argument_hint: Option<&str>,
    model: Option<&str>,
    user_invocable: Option<bool>,
) {
    let now = chrono::Utc::now().timestamp();
    let invocable = user_invocable.unwrap_or(true);
    conn.execute(
        "UPDATE prompts SET title = ?1, description = ?2, content = ?3, \
         argument_hint = ?4, model = ?5, user_invocable = ?6, updated_at = ?7 WHERE id = ?8",
        params![
            name,
            description,
            instructions,
            argument_hint,
            model,
            if invocable { 1 } else { 0 },
            now,
            id
        ],
    )
    .ok();
}

pub fn delete_skill(conn: &Connection, id: &str) {
    conn.execute("DELETE FROM prompts WHERE id = ?1", params![id]).ok();
}

// ── Reference CRUD ──

pub fn add_skill_reference(
    conn: &Connection,
    skill_id: &str,
    name: &str,
    content: &str,
) -> Result<SkillReference, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let sort_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 \
             FROM skill_references WHERE skill_id = ?1",
            params![skill_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO skill_references (id, skill_id, name, content, sort_order) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, skill_id, name, content, sort_order],
    )?;
    Ok(SkillReference {
        id,
        skill_id: skill_id.to_string(),
        name: name.to_string(),
        content: content.to_string(),
        sort_order,
    })
}

pub fn update_skill_reference(conn: &Connection, id: &str, name: &str, content: &str) {
    conn.execute(
        "UPDATE skill_references SET name = ?1, content = ?2 WHERE id = ?3",
        params![name, content, id],
    )
    .ok();
}

pub fn delete_skill_reference(conn: &Connection, id: &str) {
    conn.execute("DELETE FROM skill_references WHERE id = ?1", params![id]).ok();
}

pub fn list_skill_references(conn: &Connection, skill_id: &str) -> Vec<SkillReference> {
    get_refs_for_skill(conn, skill_id).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::run_migrations;

    fn setup_db() -> rusqlite::Connection {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        run_migrations(&mut conn).unwrap();
        conn
    }

    #[test]
    fn test_create_and_list_skills() {
        let conn = setup_db();
        let skill = create_skill(&conn, "test", "desc", "instructions", None, None, None).unwrap();
        assert_eq!(skill.name, "test");
        let skills = list_skills(&conn).unwrap();
        assert!(skills.iter().any(|s| s.name == "test"));
    }

    #[test]
    fn test_get_skill() {
        let conn = setup_db();
        let skill = create_skill(&conn, "getter", "desc", "inst", Some("hint"), Some("gpt-4"), Some(false)).unwrap();
        let found = get_skill(&conn, &skill.id).unwrap();
        assert_eq!(found.name, "getter");
        assert_eq!(found.argument_hint, Some("hint".to_string()));
        assert_eq!(found.model, Some("gpt-4".to_string()));
        assert!(!found.user_invocable);
    }

    #[test]
    fn test_update_skill() {
        let conn = setup_db();
        let skill = create_skill(&conn, "old", "old", "old", None, None, None).unwrap();
        update_skill(&conn, &skill.id, "new", "new desc", "new inst", None, None, None);
        let updated = get_skill(&conn, &skill.id).unwrap();
        assert_eq!(updated.name, "new");
        assert_eq!(updated.description, "new desc");
    }

    #[test]
    fn test_delete_skill() {
        let conn = setup_db();
        let skill = create_skill(&conn, "temp", "d", "i", None, None, None).unwrap();
        delete_skill(&conn, &skill.id);
        assert!(get_skill(&conn, &skill.id).is_none());
    }

    #[test]
    fn test_skill_references() {
        let conn = setup_db();
        let skill = create_skill(&conn, "with-refs", "d", "i", None, None, None).unwrap();

        let r1 = add_skill_reference(&conn, &skill.id, "ref1", "content1").unwrap();
        let r2 = add_skill_reference(&conn, &skill.id, "ref2", "content2").unwrap();

        let refs = list_skill_references(&conn, &skill.id);
        assert_eq!(refs.len(), 2);

        update_skill_reference(&conn, &r1.id, "ref1-updated", "content1-updated");
        let refs = list_skill_references(&conn, &skill.id);
        assert_eq!(refs[0].name, "ref1-updated");

        delete_skill_reference(&conn, &r2.id);
        let refs = list_skill_references(&conn, &skill.id);
        assert_eq!(refs.len(), 1);
    }

    #[test]
    fn test_get_skill_instructions_with_refs() {
        let conn = setup_db();
        let skill = create_skill(&conn, "inst-test", "d", "main instructions", None, None, None).unwrap();
        add_skill_reference(&conn, &skill.id, "API Docs", "api content here").unwrap();

        let instructions = get_skill_instructions(&conn, &skill.id).unwrap();
        assert!(instructions.contains("main instructions"));
        assert!(instructions.contains("## API Docs"));
        assert!(instructions.contains("api content here"));
    }

    #[test]
    fn test_delete_skill_cascades_references() {
        let conn = setup_db();
        let skill = create_skill(&conn, "cascade", "d", "i", None, None, None).unwrap();
        add_skill_reference(&conn, &skill.id, "ref", "content").unwrap();
        delete_skill(&conn, &skill.id);
        let refs = list_skill_references(&conn, &skill.id);
        assert!(refs.is_empty());
    }

    #[test]
    fn test_preset_skills_seeded() {
        let conn = setup_db();
        let skills = list_skills(&conn).unwrap();
        let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"mcp-builder"));
        assert!(names.contains(&"pdf"));
        assert!(names.contains(&"research"));
    }
}
