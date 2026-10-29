use rusqlite::{Connection, OptionalExtension};

use crate::error::AppError;

include!(concat!(env!("OUT_DIR"), "/migrations_list.rs"));

pub fn run_pending(conn: &Connection) -> Result<(), AppError> {
    apply_pending(conn, MIGRATIONS)
}

/// How a database's applied migrations differ from this binary's list.
#[derive(Debug, PartialEq, Eq)]
pub enum SchemaMismatch {
    /// No `_migrations` table: not a database Saldo has opened.
    NotInitialized,
    /// This binary knows migrations the database lacks.
    Behind { missing: Vec<String> },
    /// The database has migrations this binary does not know: a newer Saldo migrated it. Wins
    /// over `Behind` when both apply.
    Ahead { unknown: Vec<String> },
}

/// Compares the applied migrations with this binary's list. Reads only, never migrates.
pub fn check_schema(conn: &Connection) -> Result<Result<(), SchemaMismatch>, AppError> {
    compare_schema(conn, MIGRATIONS)
}

fn compare_schema(
    conn: &Connection,
    migrations: &[(&str, &str)],
) -> Result<Result<(), SchemaMismatch>, AppError> {
    let initialized: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_migrations')",
        [],
        |row| row.get(0),
    )?;
    if !initialized {
        return Ok(Err(SchemaMismatch::NotInitialized));
    }
    let applied: Vec<String> = conn
        .prepare("SELECT filename FROM _migrations ORDER BY filename")?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let unknown: Vec<String> = applied
        .iter()
        .filter(|name| !migrations.iter().any(|(known, _)| known == name))
        .cloned()
        .collect();
    if !unknown.is_empty() {
        return Ok(Err(SchemaMismatch::Ahead { unknown }));
    }
    let missing: Vec<String> = migrations
        .iter()
        .filter(|(known, _)| !applied.iter().any(|name| name == known))
        .map(|(known, _)| known.to_string())
        .collect();
    if !missing.is_empty() {
        return Ok(Err(SchemaMismatch::Behind { missing }));
    }
    Ok(Ok(()))
}

fn apply_pending(conn: &Connection, migrations: &[(&str, &str)]) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           filename TEXT NOT NULL UNIQUE,
           applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now'))
         );",
    )?;

    for (filename, sql) in migrations {
        let already_applied: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM _migrations WHERE filename = ?1)",
            [filename],
            |row| row.get(0),
        )?;

        if !already_applied {
            apply(conn, filename, sql).map_err(|e| AppError {
                code: "MIGRATION_FAILED".into(),
                message: format!("Migration {filename} failed: {}", e.message),
            })?;
        }
    }
    Ok(())
}

/// SQLite's procedure for schema changes. `foreign_keys` is a no-op inside a transaction, so it
/// is switched off before BEGIN, which also keeps a table rebuild's DROP TABLE from cascading
/// into child rows. `foreign_key_check` then reports any violation the body left.
fn apply(conn: &Connection, filename: &str, sql: &str) -> Result<(), AppError> {
    conn.execute_batch("PRAGMA foreign_keys = OFF; BEGIN;")?;
    let result = apply_in_transaction(conn, filename, sql);
    if result.is_err() {
        let _ = conn.execute_batch("ROLLBACK");
    }
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    result
}

fn apply_in_transaction(conn: &Connection, filename: &str, sql: &str) -> Result<(), AppError> {
    conn.execute_batch(sql)?;
    let violation: Option<(String, String)> = conn
        .query_row("PRAGMA foreign_key_check", [], |row| {
            Ok((row.get(0)?, row.get(2)?))
        })
        .optional()?;
    if let Some((table, parent)) = violation {
        return Err(AppError {
            code: "FOREIGN_KEY_VIOLATION".into(),
            message: format!("it left a row in {table} that references a missing {parent} row"),
        });
    }
    conn.execute("INSERT INTO _migrations (filename) VALUES (?1)", [filename])?;
    conn.execute_batch("COMMIT")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fk_enabled_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        conn
    }

    fn table_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [name],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn applied(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT filename FROM _migrations ORDER BY id")
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    const FIRST: (&str, &str) = ("001_first.sql", "CREATE TABLE first (id INTEGER);");
    const SECOND: (&str, &str) = ("002_second.sql", "CREATE TABLE second (id INTEGER);");

    #[test]
    fn a_database_on_the_same_migrations_matches() {
        let conn = fk_enabled_conn();
        apply_pending(&conn, &[FIRST, SECOND]).unwrap();

        assert_eq!(compare_schema(&conn, &[FIRST, SECOND]).unwrap(), Ok(()));
    }

    #[test]
    fn a_database_missing_migrations_is_behind() {
        let conn = fk_enabled_conn();
        apply_pending(&conn, &[FIRST]).unwrap();

        assert_eq!(
            compare_schema(&conn, &[FIRST, SECOND]).unwrap(),
            Err(SchemaMismatch::Behind {
                missing: vec!["002_second.sql".into()]
            })
        );
        assert!(!table_exists(&conn, "second"), "the check never migrates");
    }

    #[test]
    fn a_database_with_unknown_migrations_is_ahead() {
        let conn = fk_enabled_conn();
        apply_pending(&conn, &[FIRST, SECOND]).unwrap();

        assert_eq!(
            compare_schema(&conn, &[FIRST]).unwrap(),
            Err(SchemaMismatch::Ahead {
                unknown: vec!["002_second.sql".into()]
            })
        );
    }

    #[test]
    fn a_database_saldo_never_opened_is_not_initialized() {
        let conn = fk_enabled_conn();

        assert_eq!(
            compare_schema(&conn, &[FIRST]).unwrap(),
            Err(SchemaMismatch::NotInitialized)
        );
    }

    #[test]
    fn a_migrated_database_matches_this_build() {
        let conn = crate::db::initialize_in_memory().unwrap();

        assert_eq!(check_schema(&conn).unwrap(), Ok(()));
    }

    #[test]
    fn a_failing_migration_leaves_no_partial_schema() {
        let conn = fk_enabled_conn();
        let migrations = [
            (
                "001_first.sql",
                "CREATE TABLE first (id INTEGER PRIMARY KEY);",
            ),
            (
                "002_broken.sql",
                "CREATE TABLE second (id INTEGER PRIMARY KEY);
                 INSERT INTO missing VALUES (1);",
            ),
        ];

        let err = apply_pending(&conn, &migrations).unwrap_err();

        assert!(table_exists(&conn, "first"));
        assert!(!table_exists(&conn, "second"));
        assert_eq!(applied(&conn), ["001_first.sql"]);
        assert!(conn.is_autocommit());
        assert!(err.message.contains("002_broken.sql"), "{}", err.message);
    }

    #[test]
    fn a_migration_that_leaves_an_orphan_row_fails() {
        let conn = fk_enabled_conn();
        let migrations = [
            (
                "001_tables.sql",
                "CREATE TABLE parent (id INTEGER PRIMARY KEY);
                 CREATE TABLE child (
                   id INTEGER PRIMARY KEY,
                   parent_id INTEGER NOT NULL REFERENCES parent (id)
                 );",
            ),
            (
                "002_orphan.sql",
                "INSERT INTO child (id, parent_id) VALUES (1, 99);",
            ),
        ];

        let err = apply_pending(&conn, &migrations).unwrap_err();

        assert!(err.message.contains("002_orphan.sql"), "{}", err.message);
        let children: i64 = conn
            .query_row("SELECT COUNT(*) FROM child", [], |row| row.get(0))
            .unwrap();
        assert_eq!(children, 0);
        assert_eq!(applied(&conn), ["001_tables.sql"]);
        let foreign_keys: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys, 1);
    }
}
