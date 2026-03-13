use std::sync::atomic::Ordering;

use crate::error::AppError;
use crate::AppState;
use tauri::State;

use super::models::{DbLocationInfo, PickDbFolderResult};
use super::repository;

const SETTING_ALLOWLIST: &[&str] = &["consolidation_currency_code", "oxr_app_id", "theme"];

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
    let demo_conn = crate::db::initialize_in_memory().map_err(AppError::from)?;
    crate::demo::seed_demo_data(&demo_conn, oxr_app_id, theme)?;

    // Swap: stash persistent connection, promote demo connection.
    // Lock ordering: db first, then persistent_db.
    {
        let mut db = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
        let mut pdb = state
            .persistent_db
            .lock()
            .map_err(|e| AppError::from(e.to_string()))?;
        let old_conn = std::mem::replace(&mut *db, demo_conn);
        *pdb = Some(old_conn);
    }

    state.demo_mode.store(true, Ordering::SeqCst);
    Ok(())
}

/// Restore the persistent database and drop the in-memory demo database.
/// All demo data is discarded. No-op if demo mode is not active.
#[tauri::command]
pub fn exit_demo_mode(state: State<'_, AppState>) -> Result<(), AppError> {
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
        let _demo = std::mem::replace(&mut *db, persistent);
    }

    state.demo_mode.store(false, Ordering::SeqCst);
    Ok(())
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
#[tauri::command]
pub fn pick_db_folder(
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

    let new_db_path = target_folder.join(crate::config::DB_FILENAME);

    // Same-folder check: compare canonicalized folder paths.
    {
        let db_path_guard = state
            .db_path
            .lock()
            .map_err(|e| AppError::from(e.to_string()))?;
        let current_folder = db_path_guard
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| db_path_guard.clone());
        let target_canonical = target_folder.canonicalize().ok();
        let current_canonical = current_folder.canonicalize().ok();
        if let (Some(t), Some(c)) = (target_canonical, current_canonical) {
            if t == c {
                return Ok(());
            }
        }
    }

    match action {
        "switch" => {
            let new_conn = crate::db::initialize_db(&new_db_path).map_err(AppError::from)?;
            {
                let mut db_guard = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
                let _ = std::mem::replace(&mut *db_guard, new_conn);
            }
            match config_action {
                ConfigAction::WriteCustom => {
                    crate::config::write_db_location(&state.app_data_dir, target_folder)?
                }
                ConfigAction::ClearToDefault => {
                    crate::config::clear_db_location(&state.app_data_dir)?
                }
            }
            {
                let mut db_path_guard = state
                    .db_path
                    .lock()
                    .map_err(|e| AppError::from(e.to_string()))?;
                *db_path_guard = new_db_path;
            }
        }
        "move" => {
            let old_path: std::path::PathBuf = {
                let db_path_guard = state
                    .db_path
                    .lock()
                    .map_err(|e| AppError::from(e.to_string()))?;
                db_path_guard.clone()
            };

            // 1. WAL checkpoint — flush WAL to main DB file.
            {
                let conn = state.conn()?;
                crate::db::wal_checkpoint(&conn).map_err(AppError::from)?;
            }

            // 2. Swap to in-memory to release the file lock on Windows.
            {
                let in_memory = crate::db::initialize_in_memory().map_err(AppError::from)?;
                let mut db_guard = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
                // _old_conn is dropped here, releasing the file lock.
                let _old_conn = std::mem::replace(&mut *db_guard, in_memory);
            }

            // 3. Copy file.
            if let Err(e) = std::fs::copy(&old_path, &new_db_path) {
                if let Ok(restored) = crate::db::initialize_db(&old_path) {
                    if let Ok(mut db_guard) = state.db.lock() {
                        let _ = std::mem::replace(&mut *db_guard, restored);
                    }
                }
                return Err(AppError::from(e));
            }

            // 4. Open new DB (runs pending migrations in case schema is ahead).
            let new_conn = match crate::db::initialize_db(&new_db_path) {
                Ok(c) => c,
                Err(e) => {
                    let _ = std::fs::remove_file(&new_db_path);
                    if let Ok(restored) = crate::db::initialize_db(&old_path) {
                        if let Ok(mut db_guard) = state.db.lock() {
                            let _ = std::mem::replace(&mut *db_guard, restored);
                        }
                    }
                    return Err(AppError::from(e));
                }
            };

            // 5. Integrity check.
            if let Err(e) = crate::db::integrity_check(&new_conn) {
                drop(new_conn);
                let _ = std::fs::remove_file(&new_db_path);
                if let Ok(restored) = crate::db::initialize_db(&old_path) {
                    if let Ok(mut db_guard) = state.db.lock() {
                        let _ = std::mem::replace(&mut *db_guard, restored);
                    }
                }
                return Err(e);
            }

            // 6. Swap to new connection.
            {
                let mut db_guard = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
                let _ = std::mem::replace(&mut *db_guard, new_conn);
            }

            // 7. Persist config and update tracked path.
            match config_action {
                ConfigAction::WriteCustom => {
                    crate::config::write_db_location(&state.app_data_dir, target_folder)?
                }
                ConfigAction::ClearToDefault => {
                    crate::config::clear_db_location(&state.app_data_dir)?
                }
            }
            {
                let mut db_path_guard = state
                    .db_path
                    .lock()
                    .map_err(|e| AppError::from(e.to_string()))?;
                *db_path_guard = new_db_path;
            }

            // 8. Delete original file and any WAL / SHM side-files.
            let _ = std::fs::remove_file(&old_path);
            let old_wal = std::path::PathBuf::from(format!("{}-wal", old_path.display()));
            let old_shm = std::path::PathBuf::from(format!("{}-shm", old_path.display()));
            let _ = std::fs::remove_file(&old_wal);
            let _ = std::fs::remove_file(&old_shm);
        }
        "fresh" => {
            let new_conn = crate::db::initialize_db(&new_db_path).map_err(AppError::from)?;
            {
                let mut db_guard = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
                let _ = std::mem::replace(&mut *db_guard, new_conn);
            }
            match config_action {
                ConfigAction::WriteCustom => {
                    crate::config::write_db_location(&state.app_data_dir, target_folder)?
                }
                ConfigAction::ClearToDefault => {
                    crate::config::clear_db_location(&state.app_data_dir)?
                }
            }
            {
                let mut db_path_guard = state
                    .db_path
                    .lock()
                    .map_err(|e| AppError::from(e.to_string()))?;
                *db_path_guard = new_db_path;
            }
        }
        _ => {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: format!(
                    "Invalid action '{}'. Expected 'switch', 'move', or 'fresh'.",
                    action
                ),
            });
        }
    }

    state.fallback_warning.store(false, Ordering::SeqCst);
    Ok(())
}

/// Changes the active database to the given folder using the specified action:
/// - `"switch"` — open an existing DB at the target location
/// - `"move"`   — WAL-checkpoint → swap to in-memory → copy file → open new → integrity check → swap → delete old
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
