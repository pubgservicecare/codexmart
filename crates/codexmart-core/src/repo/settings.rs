use rusqlite::{params, Connection};
use serde_json::{Map, Value};

use crate::error::AppResult;

/// All stored settings as a JSON object (values are JSON-encoded).
pub fn load_all(conn: &Connection) -> AppResult<Map<String, Value>> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut map = Map::new();
    for row in rows {
        let (key, raw) = row?;
        if let Ok(v) = serde_json::from_str::<Value>(&raw) {
            map.insert(key, v);
        }
    }
    Ok(map)
}

pub fn save_all(conn: &Connection, values: &Map<String, Value>, now: &str) -> AppResult<()> {
    for (key, value) in values {
        conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value.to_string(), now],
        )?;
    }
    Ok(())
}
