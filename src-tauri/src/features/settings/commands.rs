use std::sync::atomic::Ordering;

use crate::error::AppError;
use crate::AppState;
use tauri::State;

use super::models::{DbLocationInfo, PickDbFolderResult};
use super::repository;

const SETTING_ALLOWLIST: &[&str] = &["oxr_app_id", "theme"];

fn validate_setting_key(key: &str) -> Result<(), AppError> {
    if SETTING_ALLOWLIST.contains(&key) {
        Ok(())
    } else {
        Err(AppError {
            code: "VALIDATION".into(),
            message: format!(
                "Unknown setting key '{}'. Allowed: {:?}",
                key, SETTING_ALLOWLIST
            ),
        })
    }
}

#[tauri::command]
pub fn get_app_setting(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<String>, AppError> {
    validate_setting_key(&key)?;
    let conn = state.conn()?;
    let value = repository::get_app_setting(&conn, &key)?;
    Ok(value)
}

#[tauri::command]
pub fn set_app_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), AppError> {
    validate_setting_key(&key)?;
    let conn = state.conn()?;
    repository::set_app_setting(&conn, &key, &value)?;
    Ok(())
}

/// Switch the active database to an ephemeral in-memory demo database seeded with
/// realistic sample data. Carries over the user's API key and colour theme.
/// No-op if demo mode is already active.
#[tauri::command]
pub fn enter_demo_mode(state: State<'_, AppState>) -> Result<(), AppError> {
    enter_demo(&state)
}

fn enter_demo(state: &AppState) -> Result<(), AppError> {
    if state.demo_mode.load(Ordering::SeqCst) {
        return Ok(());
    }

    // Read settings from the persistent DB (currently the active connection).
    let oxr_app_id = {
        let conn = state.conn()?;
        repository::get_app_setting(&conn, "oxr_app_id")?
    };
    let theme = {
        let conn = state.conn()?;
        repository::get_app_setting(&conn, "theme")?
    };

    // Create and seed the in-memory demo database.
    let demo_conn = crate::db::initialize_in_memory()?;
    crate::demo::seed_demo_data(&demo_conn, oxr_app_id, theme)?;

    // Swap: stash persistent connection, promote demo connection.
    // Lock ordering: db first, then persistent_db.
    {
        let mut db = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
        let mut pdb = state
            .persistent_db
            .lock()
            .map_err(|e| AppError::from(e.to_string()))?;
        let old_conn = state.swap_conn(&mut db, demo_conn);
        *pdb = Some(old_conn);
    }

    state.demo_mode.store(true, Ordering::SeqCst);
    Ok(())
}

/// Restore the persistent database and drop the in-memory demo database.
/// All demo data is discarded. No-op if demo mode is not active.
#[tauri::command]
pub fn exit_demo_mode(state: State<'_, AppState>) -> Result<(), AppError> {
    exit_demo(&state)
}

fn exit_demo(state: &AppState) -> Result<(), AppError> {
    if !state.demo_mode.load(Ordering::SeqCst) {
        return Ok(());
    }

    // Reverse swap: restore persistent connection, drop demo connection.
    // Lock ordering: db first, then persistent_db.
    {
        let mut db = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
        let mut pdb = state
            .persistent_db
            .lock()
            .map_err(|e| AppError::from(e.to_string()))?;
        let persistent = pdb.take().ok_or_else(|| AppError {
            code: "STATE_ERROR".into(),
            message: "No persistent connection stashed — state is inconsistent".into(),
        })?;
        // Old in-memory connection is dropped here, discarding all demo data.
        let _demo = state.swap_conn(&mut db, persistent);
    }

    state.demo_mode.store(false, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn get_db_data_version(state: State<'_, AppState>) -> Result<i64, AppError> {
    let conn = state.conn()?;
    crate::db::data_version(&conn)
}

/// Returns `true` when the app is currently running against the ephemeral demo database.
#[tauri::command]
pub fn is_demo_mode(state: State<'_, AppState>) -> Result<bool, AppError> {
    Ok(state.demo_mode.load(Ordering::SeqCst))
}

/// Returns information about the current database location.
#[tauri::command]
pub fn get_db_location(state: State<'_, AppState>) -> Result<DbLocationInfo, AppError> {
    let db_path_guard = state
        .db_path
        .lock()
        .map_err(|e| AppError::from(e.to_string()))?;
    let current_path = db_path_guard.to_string_lossy().to_string();
    let default_path = state.app_data_dir.join(crate::config::DB_FILENAME);
    let is_default = *db_path_guard == default_path;
    let is_demo_mode = state.demo_mode.load(Ordering::SeqCst);
    let fallback_warning = state.fallback_warning.load(Ordering::SeqCst);
    Ok(DbLocationInfo {
        current_path,
        is_default,
        is_demo_mode,
        fallback_warning,
    })
}

/// Opens a native folder-picker dialog and returns the selected folder with a flag
/// indicating whether `saldo.db` already exists there. Returns `None` if the
/// user cancelled the dialog.
///
/// Must be `async` so Tauri runs it off the main thread; `blocking_pick_folder()`
/// dispatches to the main thread internally and would deadlock on macOS if the
/// command itself already occupied the main thread.
#[tauri::command]
pub async fn pick_db_folder(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<PickDbFolderResult>, AppError> {
    if state.demo_mode.load(Ordering::SeqCst) {
        return Err(AppError {
            code: "DEMO_MODE".into(),
            message: "Cannot change database location while demo mode is active.".into(),
        });
    }
    use tauri_plugin_dialog::DialogExt;
    let folder = app.dialog().file().blocking_pick_folder();
    match folder {
        None => Ok(None),
        Some(file_path) => {
            let folder_str = file_path.to_string();
            let path = std::path::PathBuf::from(&folder_str);
            let db_file_path = path.join(crate::config::DB_FILENAME);
            let db_exists = db_file_path.exists();
            Ok(Some(PickDbFolderResult {
                folder: folder_str,
                db_exists,
            }))
        }
    }
}

enum ConfigAction {
    WriteCustom,
    ClearToDefault,
}

fn relocate_db(
    state: &AppState,
    target_folder: &std::path::Path,
    action: &str,
    config_action: ConfigAction,
) -> Result<(), AppError> {
    if state.demo_mode.load(Ordering::SeqCst) {
        return Err(AppError {
            code: "DEMO_MODE".into(),
            message: "Cannot change database location while demo mode is active.".into(),
        });
    }

    if !matches!(action, "switch" | "move" | "fresh") {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: format!(
                "Invalid action '{}'. Expected 'switch', 'move', or 'fresh'.",
                action
            ),
        });
    }

    // Held until the swap, so no write lands on the old database after it was copied.
    let mut db = state.conn()?;
    let mut db_path = state
        .db_path
        .lock()
        .map_err(|e| AppError::from(e.to_string()))?;

    // Same-folder check: compare canonicalized folder paths.
    let current_folder = db_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| db_path.clone());
    if let (Ok(t), Ok(c)) = (target_folder.canonicalize(), current_folder.canonicalize()) {
        if t == c {
            return Ok(());
        }
    }

    let new_db_path = target_folder.join(crate::config::DB_FILENAME);
    let creates_file = !new_db_path.exists();
    let prepared = if action == "move" {
        crate::db::copy_db(&db, &new_db_path)
    } else {
        Ok(())
    }
    .and_then(|()| open_checked(&new_db_path))
    .and_then(|conn| {
        write_config(&state.app_data_dir, target_folder, config_action)?;
        Ok(conn)
    });
    let new_conn = match prepared {
        Ok(conn) => conn,
        Err(e) => {
            if creates_file {
                crate::db::remove_db_files(&new_db_path);
            }
            return Err(e);
        }
    };

    let old_conn = state.swap_conn(&mut db, new_conn);
    let old_path = std::mem::replace(&mut *db_path, new_db_path);
    drop(db_path);
    drop(db);
    state.fallback_warning.store(false, Ordering::SeqCst);

    if action == "move" {
        // Windows refuses to delete a file that is still open.
        drop(old_conn);
        // The move itself succeeded and the app now runs on the new file, so a leftover old
        // file is logged rather than reported as a failure.
        for failure in crate::db::remove_db_files(&old_path) {
            eprintln!("[relocate_db] could not delete the old database file {failure}");
        }
    }
    Ok(())
}

fn open_checked(db_path: &std::path::Path) -> Result<rusqlite::Connection, AppError> {
    let conn = crate::db::initialize_db(db_path)?;
    crate::db::integrity_check(&conn)?;
    Ok(conn)
}

fn write_config(
    app_data_dir: &std::path::Path,
    target_folder: &std::path::Path,
    config_action: ConfigAction,
) -> Result<(), AppError> {
    match config_action {
        ConfigAction::WriteCustom => crate::config::write_db_location(app_data_dir, target_folder),
        ConfigAction::ClearToDefault => crate::config::clear_db_location(app_data_dir),
    }
}

/// Changes the active database to the given folder using the specified action:
/// - `"switch"` — open an existing DB at the target location
/// - `"move"`   — copy the live DB into the target → open and integrity-check the copy →
///   write the config → swap → delete the old files
/// - `"fresh"`  — create a brand-new DB at the target location (migrations applied)
///
/// Returns an error if demo mode is active, the folder is not writable, or the
/// target is already the current location.
#[tauri::command]
pub fn change_db_location(
    state: State<'_, AppState>,
    folder: String,
    action: String,
) -> Result<(), AppError> {
    let target_folder = std::path::PathBuf::from(&folder);

    // Writable check: create and immediately delete a temp file.
    let temp_path = target_folder.join(".write-test-tmp");
    std::fs::write(&temp_path, b"").map_err(|e| AppError {
        code: "NOT_WRITABLE".into(),
        message: format!("Target folder is not writable: {}", e),
    })?;
    let _ = std::fs::remove_file(&temp_path);

    relocate_db(&state, &target_folder, &action, ConfigAction::WriteCustom)
}

/// Resets the database location back to the default `app_data_dir`. Accepts the same
/// `action` values as `change_db_location`. Clears `db-location.json` after a
/// successful swap.
#[tauri::command]
pub fn reset_db_location(state: State<'_, AppState>, action: String) -> Result<(), AppError> {
    let target_folder = state.app_data_dir.clone();
    relocate_db(
        &state,
        &target_folder,
        &action,
        ConfigAction::ClearToDefault,
    )
}

/// Returns `true` if `saldo.db` already exists at the default `app_data_dir`.
/// Used by the frontend to decide which dialog to show for "Reset to default".
#[tauri::command]
pub fn check_default_db(state: State<'_, AppState>) -> Result<bool, AppError> {
    let default_path = state.app_data_dir.join(crate::config::DB_FILENAME);
    Ok(default_path.exists())
}

#[tauri::command]
pub fn get_bulk_update_exclusions(state: State<'_, AppState>) -> Result<Vec<i64>, AppError> {
    let conn = state.conn()?;
    Ok(repository::fetch_bulk_update_exclusions(&conn)?)
}

#[tauri::command]
pub fn set_bulk_update_exclusions(
    state: State<'_, AppState>,
    account_ids: Vec<i64>,
) -> Result<(), AppError> {
    let conn = state.conn()?;
    repository::replace_bulk_update_exclusions(&conn, &account_ids)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;

    fn test_state(root: &std::path::Path) -> AppState {
        let app_dir = root.join("app");
        std::fs::create_dir_all(&app_dir).unwrap();
        let db_path = app_dir.join(config::DB_FILENAME);
        let conn = crate::db::initialize_db(&db_path).expect("DB init failed");
        AppState::new(conn, app_dir, db_path, false)
    }

    fn file_names(folder: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(folder)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    /// Writes `marker` through the app's connection and reads it back from `db_path` directly,
    /// so it passes only when the app is still writing to that file.
    fn assert_app_writes_to(state: &AppState, db_path: &std::path::Path, marker: &str) {
        repository::set_app_setting(&state.conn().unwrap(), "theme", marker).unwrap();
        let direct = rusqlite::Connection::open(db_path).unwrap();
        assert_eq!(
            repository::get_app_setting(&direct, "theme")
                .unwrap()
                .as_deref(),
            Some(marker)
        );
    }

    #[test]
    fn a_failed_config_write_keeps_the_old_database_live() {
        for action in ["switch", "move", "fresh"] {
            let dir = tempfile::tempdir().unwrap();
            let state = test_state(dir.path());
            let old_path = state.db_path.lock().unwrap().clone();
            let target = dir.path().join("custom");
            std::fs::create_dir_all(&target).unwrap();
            if action == "switch" {
                drop(crate::db::initialize_db(&target.join(config::DB_FILENAME)).unwrap());
            }
            std::fs::create_dir(state.app_data_dir.join("db-location.json")).unwrap();

            let result = relocate_db(&state, &target, action, ConfigAction::WriteCustom);

            assert!(result.is_err(), "{action}");
            assert_eq!(*state.db_path.lock().unwrap(), old_path, "{action}");
            assert_app_writes_to(&state, &old_path, action);
            assert_eq!(config::read_db_location(&state.app_data_dir), None);
            assert!(state.app_data_dir.join("db-location.json").is_dir());
            assert!(!state.app_data_dir.join("db-location.json.tmp").exists());
            let expected_left: &[&str] = if action == "switch" {
                &["saldo.db"]
            } else {
                &[]
            };
            assert_eq!(file_names(&target), expected_left, "{action}");
        }
    }

    #[test]
    fn a_failed_copy_leaves_no_file_and_keeps_the_old_database_live() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let old_path = state.db_path.lock().unwrap().clone();
        let target = dir.path().join("custom");
        std::fs::create_dir_all(target.join("saldo.db.tmp")).unwrap();

        let result = relocate_db(&state, &target, "move", ConfigAction::WriteCustom);

        assert!(result.is_err());
        assert_eq!(file_names(&target), ["saldo.db.tmp"]);
        assert_eq!(*state.db_path.lock().unwrap(), old_path);
        assert_eq!(config::read_db_location(&state.app_data_dir), None);
        assert_app_writes_to(&state, &old_path, "after-failed-copy");
    }

    #[test]
    fn moving_carries_rows_that_are_still_in_the_wal() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let old_path = state.db_path.lock().unwrap().clone();
        let old_wal = dir.path().join("app").join("saldo.db-wal");
        let target = dir.path().join("custom");
        std::fs::create_dir_all(&target).unwrap();
        {
            let conn = state.conn().unwrap();
            let busy: i64 = conn
                .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))
                .unwrap();
            assert_eq!(busy, 0);
            repository::set_app_setting(&conn, "theme", "written-before-move").unwrap();
        }
        assert!(std::fs::metadata(&old_wal).unwrap().len() > 0);

        relocate_db(&state, &target, "move", ConfigAction::WriteCustom).unwrap();

        let new_path = target.join(config::DB_FILENAME);
        assert_eq!(*state.db_path.lock().unwrap(), new_path);
        assert!(!old_path.exists());
        assert_eq!(file_names(&state.app_data_dir), ["db-location.json"]);
        let direct = rusqlite::Connection::open(&new_path).unwrap();
        assert_eq!(
            repository::get_app_setting(&direct, "theme")
                .unwrap()
                .as_deref(),
            Some("written-before-move")
        );
    }

    #[test]
    fn consolidation_currency_is_not_a_generic_setting() {
        let err = validate_setting_key("consolidation_currency_code").unwrap_err();

        assert_eq!(err.code, "VALIDATION");
    }

    #[test]
    fn entering_and_exiting_demo_mode_bump_the_db_generation() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());

        let before_enter = state.db_generation();
        enter_demo(&state).unwrap();
        let after_enter = state.db_generation();
        assert!(after_enter > before_enter);

        exit_demo(&state).unwrap();
        assert!(state.db_generation() > after_enter);
    }

    #[test]
    fn fresh_location_creates_the_db_file_and_bumps_the_db_generation() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let target = dir.path().join("custom");
        std::fs::create_dir_all(&target).unwrap();

        let before = state.db_generation();
        relocate_db(&state, &target, "fresh", ConfigAction::WriteCustom).unwrap();

        assert!(state.db_generation() > before);
        assert_eq!(
            config::resolve_db_location(&state.app_data_dir),
            config::DbLocation::Custom(target.join(config::DB_FILENAME))
        );
    }

    #[test]
    fn moving_the_location_relocates_the_file_and_bumps_the_db_generation() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let old_path = state.db_path.lock().unwrap().clone();
        let target = dir.path().join("custom");
        std::fs::create_dir_all(&target).unwrap();

        let before = state.db_generation();
        relocate_db(&state, &target, "move", ConfigAction::WriteCustom).unwrap();

        assert!(state.db_generation() > before);
        assert!(!old_path.exists());
        assert_eq!(
            config::resolve_db_location(&state.app_data_dir),
            config::DbLocation::Custom(target.join(config::DB_FILENAME))
        );
        let conn = state.conn().unwrap();
        repository::get_app_setting(&conn, "consolidation_currency_code").unwrap();
    }
}
