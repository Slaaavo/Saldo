use rusqlite::{Connection, Result as SqlResult};
use std::path::Path;

use crate::{error::AppError, migrations};

pub fn initialize_db(db_path: &Path) -> SqlResult<Connection> {
    let conn = Connection::open(db_path)?;
    set_pragmas(&conn)?;
    migrations::run_pending(&conn)?;
    Ok(conn)
}

pub fn set_pragmas(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;",
    )?;
    Ok(())
}

pub fn initialize_in_memory() -> SqlResult<Connection> {
    let conn = Connection::open_in_memory()?;
    set_pragmas(&conn)?;
    crate::migrations::run_pending(&conn)?;
    Ok(conn)
}

/// Flush the WAL to the main database file and truncate it.
pub fn wal_checkpoint(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    Ok(())
}

/// Run `PRAGMA integrity_check` and return an error if the result is not "ok".
pub fn integrity_check(conn: &Connection) -> Result<(), AppError> {
    let result: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(AppError::from)?;
    if result.to_lowercase() != "ok" {
        return Err(AppError {
            code: "INTEGRITY_CHECK_FAILED".into(),
            message: format!("Database integrity check failed: {}", result),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_cleanly() {
        let conn = initialize_in_memory().expect("DB init failed");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
            .expect("query failed");
        assert_eq!(count, 11, "Expected 11 applied migrations");
    }

    #[test]
    fn migrations_are_idempotent() {
        let conn = initialize_in_memory().expect("DB init failed");
        // Run migrations again — should be a no-op
        crate::migrations::run_pending(&conn).expect("Second migration run failed");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
            .expect("query failed");
        assert_eq!(count, 11, "Expected 11 applied migrations after double run");
    }

    #[test]
    fn seed_data_present() {
        let conn = initialize_in_memory().expect("DB init failed");

        let currency_code: String = conn
            .query_row("SELECT code FROM currency WHERE id = 1", [], |row| {
                row.get(0)
            })
            .expect("currency query failed");
        assert_eq!(currency_code, "EUR");

        // Migration 008 deletes "Main Account" when no events exist (fresh DB).
        let account_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM account", [], |row| row.get(0))
            .expect("account count query failed");
        assert_eq!(
            account_count, 0,
            "Expected 0 accounts after migration 008 removes Main Account from empty DB"
        );
    }

    #[test]
    fn wal_checkpoint_succeeds_on_in_memory() {
        let conn = initialize_in_memory().expect("DB init failed");
        // WAL checkpoint is a no-op for in-memory DBs but should not error.
        wal_checkpoint(&conn).expect("wal_checkpoint failed");
    }

    #[test]
    fn integrity_check_passes_on_fresh_db() {
        let conn = initialize_in_memory().expect("DB init failed");
        integrity_check(&conn).expect("integrity_check failed on fresh DB");
    }
}
