use rusqlite::functions::FunctionFlags;
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::{error::AppError, migrations};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub fn initialize_db(db_path: &Path) -> Result<Connection, AppError> {
    let conn = Connection::open(db_path)?;
    configure(&conn)?;
    migrations::run_pending(&conn)?;
    Ok(conn)
}

pub fn open_existing(db_path: &Path) -> Result<Connection, AppError> {
    let flags = OpenFlags::default().difference(OpenFlags::SQLITE_OPEN_CREATE);
    let conn = Connection::open_with_flags(db_path, flags)?;
    configure(&conn)?;
    Ok(conn)
}

/// It moves only when another connection committed, never for this connection's own writes.
pub fn data_version(conn: &Connection) -> Result<i64, AppError> {
    Ok(conn.query_row("PRAGMA data_version", [], |row| row.get(0))?)
}

/// Takes the write lock and runs `f` under it. When `f` fails the lock is released; when it
/// succeeds the lock stays until `conn` is closed, so no other process commits to the file in
/// between.
pub fn lock_until_closed(
    conn: &Connection,
    f: impl FnOnce() -> Result<(), AppError>,
) -> Result<(), AppError> {
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = f();
    if result.is_err() {
        let _ = conn.execute_batch("ROLLBACK");
    }
    result
}

fn configure(conn: &Connection) -> Result<(), AppError> {
    set_pragmas(conn)?;
    register_fold(conn)
}

/// Pragmas that return a row run through `query_row`: `execute_batch` discards the row, so a
/// result reporting failure would pass silently.
fn set_pragmas(conn: &Connection) -> Result<(), AppError> {
    conn.busy_timeout(BUSY_TIMEOUT)?;
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

/// SQL `fold(text)` for searches that ignore case and accents. SQLite's `lower()` folds only
/// A to Z, and Slovak bank texts and supplier names carry diacritics.
fn register_fold(conn: &Connection) -> Result<(), AppError> {
    conn.create_scalar_function(
        "fold",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| Ok(ctx.get::<Option<String>>(0)?.map(|text| fold(&text))),
    )?;
    Ok(())
}

/// Lowercases, then drops combining marks and the diacritics of Latin letters, so `ŠKODA`
/// folds to `skoda`.
fn fold(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        match c {
            '\u{0300}'..='\u{036f}' => {}
            'ß' => folded.push_str("ss"),
            'æ' => folded.push_str("ae"),
            'œ' => folded.push_str("oe"),
            _ => folded.push(latin_base_letter(c)),
        }
    }
    folded
}

fn latin_base_letter(c: char) -> char {
    match c {
        'à'..='å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'ď' | 'đ' => 'd',
        'è'..='ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'ĥ' | 'ħ' => 'h',
        'ì'..='ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => 'i',
        'ĵ' => 'j',
        'ķ' => 'k',
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => 'l',
        'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
        'ò'..='ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => 'o',
        'ŕ' | 'ŗ' | 'ř' => 'r',
        'ś' | 'ŝ' | 'ş' | 'š' => 's',
        'ţ' | 'ť' | 'ŧ' => 't',
        'ù'..='ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'ŵ' => 'w',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        _ => c,
    }
}

pub fn initialize_in_memory() -> Result<Connection, AppError> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    crate::migrations::run_pending(&conn)?;
    Ok(conn)
}

/// Copy the database behind `conn` to `dest`. `VACUUM INTO` writes a consistent snapshot that
/// includes pages still in the WAL, but does not sync its output, so the copy is written and
/// synced under a temporary name and only then renamed: `dest` never exists half-written.
/// `VACUUM INTO` refuses an existing file, so a temporary file left by a crashed copy goes first.
pub fn copy_db(conn: &Connection, dest: &Path) -> Result<(), AppError> {
    let tmp = side_file(dest, ".tmp");
    let _ = std::fs::remove_file(&tmp);
    let copied = vacuum_into(conn, &tmp).and_then(|()| Ok(std::fs::rename(&tmp, dest)?));
    if copied.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    copied
}

fn vacuum_into(conn: &Connection, dest: &Path) -> Result<(), AppError> {
    let dest_str = dest
        .to_str()
        .ok_or_else(|| AppError::from(format!("Unsupported path: {}", dest.display())))?;
    conn.execute("VACUUM INTO ?1", [dest_str])?;
    // Windows needs write access to flush a file.
    std::fs::OpenOptions::new()
        .write(true)
        .open(dest)?
        .sync_all()?;
    Ok(())
}

/// Delete a database file and its `-wal` and `-shm` side files. Returns the deletions that
/// failed; a file that does not exist counts as deleted.
pub fn remove_db_files(db_path: &Path) -> Vec<String> {
    ["", "-wal", "-shm"]
        .into_iter()
        .map(|suffix| side_file(db_path, suffix))
        .filter_map(|path| match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                Some(format!("{}: {e}", path.display()))
            }
            _ => None,
        })
        .collect()
}

fn side_file(db_path: &Path, suffix: &str) -> PathBuf {
    let mut name = db_path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
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
    fn fold_ignores_case_and_accents_in_sql() {
        let conn = initialize_in_memory().unwrap();
        let folded = |text: &str| -> Option<String> {
            conn.query_row("SELECT fold(?1)", [text], |row| row.get(0))
                .unwrap()
        };

        assert_eq!(
            folded("ŠKODA AUTO a.s.").as_deref(),
            Some("skoda auto a.s.")
        );
        assert_eq!(folded("čez").as_deref(), Some("cez"));
        assert_eq!(
            folded("ĽUBOŠ Ďurčík ÔŔ").as_deref(),
            Some("lubos durcik or")
        );
        assert_eq!(folded("S\u{030C}KODA").as_deref(), Some("skoda"));
        assert_eq!(folded("Straße").as_deref(), Some("strasse"));
        let null: Option<String> = conn
            .query_row("SELECT fold(NULL)", [], |row| row.get(0))
            .unwrap();
        assert_eq!(null, None);
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
        assert_eq!(count, 37, "Expected 37 migrations to be applied");
    }

    #[test]
    fn migrations_are_idempotent() {
        let conn = initialize_in_memory().expect("DB init failed");
        crate::migrations::run_pending(&conn).expect("second run_pending failed");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
            .expect("_migrations count query failed");
        assert_eq!(
            count, 37,
            "Running migrations twice must not duplicate rows"
        );
    }

    #[test]
    fn open_existing_never_creates_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("saldo.db");

        assert!(open_existing(&path).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn open_existing_opens_without_migrating() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("saldo.db");
        drop(Connection::open(&path).unwrap());

        let conn = open_existing(&path).unwrap();

        let migrations_table: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = '_migrations')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!migrations_table);
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(journal_mode, "wal");
    }

    #[test]
    fn data_version_moves_only_for_another_connections_commit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("saldo.db");
        let app = initialize_db(&path).unwrap();
        let mcp = open_existing(&path).unwrap();
        let write = |conn: &Connection, theme: &str| {
            conn.execute(
                "INSERT INTO app_setting (key, value) VALUES ('theme', ?1)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                [theme],
            )
            .unwrap()
        };
        let start = data_version(&app).unwrap();

        write(&app, "dark");
        let after_own_write = data_version(&app).unwrap();
        write(&mcp, "light");
        let after_other_write = data_version(&app).unwrap();

        assert_eq!(after_own_write, start);
        assert_ne!(after_other_write, after_own_write);
    }

    #[test]
    fn a_second_writer_waits_for_the_first_instead_of_failing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("saldo.db");
        let app = initialize_db(&path).unwrap();
        let other = initialize_db(&path).unwrap();
        app.execute_batch("BEGIN IMMEDIATE").unwrap();

        let writer =
            std::thread::spawn(move || other.execute_batch("BEGIN IMMEDIATE; COMMIT;").is_ok());
        std::thread::sleep(Duration::from_millis(200));
        app.execute_batch("COMMIT").unwrap();

        assert!(
            writer.join().unwrap(),
            "the second writer got the lock once it was free"
        );
    }

    #[test]
    fn copy_db_replaces_a_temporary_file_left_by_a_crashed_copy() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("saldo.db");
        std::fs::write(dir.path().join("saldo.db.tmp"), b"half-written").unwrap();
        let conn = initialize_in_memory().expect("DB init failed");

        copy_db(&conn, &dest).expect("copy over a stale temporary file failed");

        let copy = Connection::open(&dest).unwrap();
        integrity_check(&copy).expect("copy failed its integrity check");
        assert!(!dir.path().join("saldo.db.tmp").exists());
    }
}
