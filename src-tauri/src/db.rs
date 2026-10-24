use rusqlite::Connection;
use std::path::Path;

use crate::{error::AppError, migrations};

pub fn initialize_db(db_path: &Path) -> Result<Connection, AppError> {
    let conn = Connection::open(db_path)?;
    set_pragmas(&conn)?;
    migrations::run_pending(&conn)?;
    Ok(conn)
}

/// Pragmas that return a row run through `query_row`: `execute_batch` discards the row, so a
/// result reporting failure would pass silently.
fn set_pragmas(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    let journal_mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    // An in-memory database has no WAL and reports "memory".
    if journal_mode != "wal" && journal_mode != "memory" {
        return Err(AppError {
            code: "DB_ERROR".into(),
            message: format!(
                "Could not switch the database to WAL mode (it stayed in '{journal_mode}')"
            ),
        });
    }
    conn.execute_batch("PRAGMA synchronous = NORMAL;")?;
    Ok(())
}

pub fn initialize_in_memory() -> Result<Connection, AppError> {
    let conn = Connection::open_in_memory()?;
    set_pragmas(&conn)?;
    crate::migrations::run_pending(&conn)?;
    Ok(conn)
}

/// Flush the WAL to the main database file and truncate it. Fails when another connection's
/// open read or write keeps the checkpoint from finishing.
pub fn wal_checkpoint(conn: &Connection) -> Result<(), AppError> {
    let busy: i64 = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))?;
    if busy != 0 {
        return Err(AppError {
            code: "DB_BUSY".into(),
            message: "Another connection is using the database, so its write-ahead log could not be flushed".into(),
        });
    }
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
    fn seed_data_present() {
        let conn = initialize_in_memory().expect("DB init failed");

        let currency_code: String = conn
            .query_row("SELECT code FROM currency WHERE id = 1", [], |row| {
                row.get(0)
            })
            .expect("currency query failed");
        assert_eq!(currency_code, "EUR");

        // Migration 008 deletes "Main Account" when no events exist (fresh DB).
        // Migration 024 creates 2 hidden default taxable accounts for the default person.
        let account_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM account", [], |row| row.get(0))
            .expect("account count query failed");
        assert_eq!(
            account_count, 2,
            "Expected 2 accounts: migration 008 removes Main Account, migration 024 adds default taxable accounts for the default person"
        );
    }

    #[test]
    fn wal_checkpoint_succeeds_on_in_memory() {
        let conn = initialize_in_memory().expect("DB init failed");
        // WAL checkpoint is a no-op for in-memory DBs but should not error.
        wal_checkpoint(&conn).expect("wal_checkpoint failed");
    }

    #[test]
    fn wal_checkpoint_fails_while_a_reader_blocks_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("saldo.db");
        let conn = initialize_db(&path).expect("DB init failed");
        conn.busy_timeout(std::time::Duration::ZERO).unwrap();
        let reader = Connection::open(&path).unwrap();
        reader
            .execute_batch("BEGIN; SELECT COUNT(*) FROM currency;")
            .unwrap();
        conn.execute(
            "INSERT INTO app_setting (key, value) VALUES ('theme', 'dark')",
            [],
        )
        .unwrap();

        assert_eq!(wal_checkpoint(&conn).unwrap_err().code, "DB_BUSY");
    }

    #[test]
    fn integrity_check_passes_on_fresh_db() {
        let conn = initialize_in_memory().expect("DB init failed");
        integrity_check(&conn).expect("integrity_check failed on fresh DB");
    }

    #[test]
    fn migrations_apply_cleanly() {
        let conn = initialize_in_memory().expect("DB init failed");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
            .expect("_migrations count query failed");
        assert_eq!(count, 34, "Expected 34 migrations to be applied");
    }

    #[test]
    fn migrations_are_idempotent() {
        let conn = initialize_in_memory().expect("DB init failed");
        crate::migrations::run_pending(&conn).expect("second run_pending failed");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
            .expect("_migrations count query failed");
        assert_eq!(
            count, 34,
            "Running migrations twice must not duplicate rows"
        );
    }
}
