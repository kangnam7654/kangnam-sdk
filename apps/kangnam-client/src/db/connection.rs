use rusqlite::{Connection, Result};
use std::path::Path;

/// Open a database connection with WAL mode, foreign keys, and busy timeout enabled
pub fn open_database(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "busy_timeout", "5000")?;
    Ok(conn)
}
