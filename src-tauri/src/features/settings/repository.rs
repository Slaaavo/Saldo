use crate::error::AppError;
use rusqlite::{params, Connection, OptionalExtension};

/// Read a single app_setting value by key. Returns None if the key does not exist.
pub fn get_app_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM app_setting WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
}

/// Upsert an app_setting key-value pair (insert or replace existing value).
pub fn set_app_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO app_setting (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// Return all account IDs currently excluded from bulk balance updates.
pub fn fetch_bulk_update_exclusions(conn: &Connection) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT account_id FROM bulk_update_exclusion")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect()
}

/// Replace the full exclusion set with the supplied account IDs.
/// Runs inside a savepoint so the delete+insert is atomic.
pub fn replace_bulk_update_exclusions(
    conn: &Connection,
    account_ids: &[i64],
) -> Result<(), AppError> {
    crate::shared::with_savepoint_app(conn, || {
        conn.execute("DELETE FROM bulk_update_exclusion", [])
            .map_err(AppError::from)?;
        for &id in account_ids {
            conn.execute(
                "INSERT INTO bulk_update_exclusion (account_id) VALUES (?1)",
                params![id],
            )
            .map_err(AppError::from)?;
        }
        Ok(())
    })
}
