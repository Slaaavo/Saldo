use std::sync::atomic::Ordering;

use crate::{db, demo, error::AppError, models::*, repository, AppState};
use chrono::NaiveDate;
use serde::Deserialize;
use tauri::State;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBalanceUpdateInput {
    pub account_id: i64,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAccountInput {
    pub name: String,
    pub currency_id: i64,
    pub account_type: Option<String>,
    pub initial_balance_minor: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAccountInput {
    pub account_id: i64,
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEventInput {
    pub event_id: i64,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListEventsFilter {
    pub account_id: Option<i64>,
    pub before_date: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkBalanceEntry {
    pub account_id: i64,
    pub amount_minor: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkCreateBalanceUpdatesInput {
    pub entries: Vec<BulkBalanceEntry>,
    pub event_date: String,
    pub note: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetConsolidationCurrencyInput {
    pub currency_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBucketAllocationInput {
    bucket_id: i64,
    source_account_id: i64,
    amount_minor: i64,
    effective_date: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetFxRateManualInput {
    pub from_currency_id: i64,
    pub to_currency_id: i64,
    pub date: String,
    pub rate_mantissa: i64,
    pub rate_exponent: i64,
}

fn validate_event_date(date_str: &str) -> Result<(), AppError> {
    if date_str.is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "event_date is required".into(),
        });
    }
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d").map_err(|_| AppError {
        code: "VALIDATION".into(),
        message: "event_date must be a valid date in YYYY-MM-DD format".into(),
    })?;
    Ok(())
}

#[tauri::command]
pub fn create_balance_update(
    state: State<'_, AppState>,
    input: CreateBalanceUpdateInput,
) -> Result<i64, AppError> {
    validate_event_date(&input.event_date)?;
    let conn = state.conn()?;
    let event_id = repository::create_balance_update(
        &conn,
        input.account_id,
        input.amount_minor,
        &input.event_date,
        input.note.as_deref(),
    )?;
    Ok(event_id)
}

#[tauri::command]
pub fn get_accounts_snapshot(
    state: State<'_, AppState>,
    date_iso: String,
) -> Result<Vec<SnapshotRow>, AppError> {
    let selected_datetime = if date_iso.len() == 10 {
        format!("{}T23:59:59", date_iso)
    } else {
        date_iso
    };
    let conn = state.conn()?;
    let snapshot = repository::get_accounts_snapshot(&conn, &selected_datetime)?;
    Ok(snapshot)
}

#[tauri::command]
pub fn list_events(
    state: State<'_, AppState>,
    filter: ListEventsFilter,
) -> Result<Vec<EventWithData>, AppError> {
    let conn = state.conn()?;
    let events = repository::list_events(&conn, filter.account_id, filter.before_date.as_deref())?;
    Ok(events)
}

#[tauri::command]
pub fn create_account(
    state: State<'_, AppState>,
    input: CreateAccountInput,
) -> Result<i64, AppError> {
    if input.name.trim().is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Account name is required".into(),
        });
    }
    let account_type = input.account_type.as_deref().unwrap_or("account");
    if account_type != "account" && account_type != "bucket" {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "account_type must be 'account' or 'bucket'".into(),
        });
    }
    let conn = state.conn()?;
    let id = repository::create_account(
        &conn,
        input.name.trim(),
        input.currency_id,
        account_type,
        input.initial_balance_minor,
    )?;
    Ok(id)
}

#[tauri::command]
pub fn update_account(
    state: State<'_, AppState>,
    input: UpdateAccountInput,
) -> Result<(), AppError> {
    if input.name.trim().is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Account name is required".into(),
        });
    }
    let conn = state.conn()?;
    repository::update_account(&conn, input.account_id, input.name.trim())?;
    Ok(())
}

#[tauri::command]
pub fn delete_account(state: State<'_, AppState>, account_id: i64) -> Result<(), AppError> {
    let conn = state.conn()?;
    repository::delete_account(&conn, account_id)?;
    Ok(())
}

#[tauri::command]
pub fn update_event(state: State<'_, AppState>, input: UpdateEventInput) -> Result<(), AppError> {
    validate_event_date(&input.event_date)?;
    let conn = state.conn()?;
    repository::update_event(
        &conn,
        input.event_id,
        input.amount_minor,
        &input.event_date,
        input.note.as_deref(),
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_event(state: State<'_, AppState>, event_id: i64) -> Result<(), AppError> {
    let conn = state.conn()?;
    repository::delete_event(&conn, event_id)?;
    Ok(())
}

#[tauri::command]
pub fn bulk_create_balance_updates(
    state: State<'_, AppState>,
    input: BulkCreateBalanceUpdatesInput,
) -> Result<Vec<i64>, AppError> {
    validate_event_date(&input.event_date)?;
    if input.entries.is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "At least one balance entry is required".into(),
        });
    }
    let conn = state.conn()?;
    let entries: Vec<(i64, i64)> = input
        .entries
        .iter()
        .map(|e| (e.account_id, e.amount_minor))
        .collect();
    let ids = repository::bulk_create_balance_updates(
        &conn,
        &entries,
        &input.event_date,
        input.note.as_deref(),
    )?;
    Ok(ids)
}

#[tauri::command]
pub fn list_currencies(state: State<'_, AppState>) -> Result<Vec<Currency>, AppError> {
    let conn = state.conn()?;
    let currencies = repository::list_currencies(&conn)?;
    Ok(currencies)
}

#[tauri::command]
pub fn get_consolidation_currency(state: State<'_, AppState>) -> Result<Currency, AppError> {
    let conn = state.conn()?;
    let currency = repository::get_consolidation_currency(&conn)?;
    Ok(currency)
}

#[tauri::command]
pub fn set_consolidation_currency(
    state: State<'_, AppState>,
    input: SetConsolidationCurrencyInput,
) -> Result<(), AppError> {
    let conn = state.conn()?;
    repository::set_consolidation_currency(&conn, input.currency_id)?;
    Ok(())
}

#[tauri::command]
pub fn set_fx_rate_manual(
    state: State<'_, AppState>,
    input: SetFxRateManualInput,
) -> Result<(), AppError> {
    if input.rate_mantissa == 0 {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "rate_mantissa must not be zero".into(),
        });
    }
    validate_event_date(&input.date)?;
    let conn = state.conn()?;
    repository::set_fx_rate_manual(
        &conn,
        input.from_currency_id,
        input.to_currency_id,
        &input.date,
        input.rate_mantissa,
        input.rate_exponent,
    )?;
    Ok(())
}

#[tauri::command]
pub fn list_fx_rates(
    state: State<'_, AppState>,
    date: Option<String>,
) -> Result<Vec<FxRateRow>, AppError> {
    let conn = state.conn()?;
    let rates = repository::list_fx_rates(&conn, date.as_deref())?;
    Ok(rates)
}

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

#[tauri::command]
pub fn get_missing_rate_dates(state: State<'_, AppState>) -> Result<Vec<String>, AppError> {
    let conn = state.conn()?;
    let consolidation = repository::get_consolidation_currency(&conn)?;
    let dates = repository::get_dates_needing_fx_rates(&conn, consolidation.id)?;
    Ok(dates)
}

#[tauri::command]
pub async fn fetch_fx_rates(
    state: tauri::State<'_, crate::AppState>,
    date_iso: Option<String>,
    force: Option<bool>,
) -> Result<Vec<FxRateRow>, AppError> {
    let store_date =
        date_iso.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    crate::smart_fetch_fx_rates(&state, &store_date, force.unwrap_or(false)).await
}

#[tauri::command]
pub fn create_bucket_allocation(
    state: State<'_, AppState>,
    input: CreateBucketAllocationInput,
) -> Result<i64, AppError> {
    validate_event_date(&input.effective_date)?;
    if input.amount_minor < 0 {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "amount_minor must be >= 0".into(),
        });
    }
    let conn = state.conn()?;

    // Validate that bucket_id refers to a bucket account.
    let bucket_type: Option<String> =
        repository::get_account_type(&conn, input.bucket_id).map_err(AppError::from)?;
    match bucket_type.as_deref() {
        Some("bucket") => {}
        Some(_) => {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "bucket_id must refer to a bucket account".into(),
            });
        }
        None => {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "bucket_id not found".into(),
            });
        }
    }

    // Validate that source_account_id refers to a regular account.
    let source_type: Option<String> =
        repository::get_account_type(&conn, input.source_account_id).map_err(AppError::from)?;
    match source_type.as_deref() {
        Some("account") => {}
        Some(_) => {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "source_account_id must refer to a regular account".into(),
            });
        }
        None => {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "source_account_id not found".into(),
            });
        }
    }

    // Over-allocation check.
    // Get the account balance at end-of-day on the effective date.
    let selected_datetime = format!("{}T23:59:59", input.effective_date);
    let balance =
        repository::get_account_balance_at_date(&conn, input.source_account_id, &selected_datetime)
            .map_err(AppError::from)?;

    // Total already allocated from this source across all buckets (excluding this bucket's
    // existing allocation, queried below, so the user can update an existing allocation).
    let total_allocated = repository::get_account_allocated_total(
        &conn,
        input.source_account_id,
        &input.effective_date,
    )
    .map_err(AppError::from)?;

    // Latest existing allocation from this source to THIS specific bucket at or before the date.
    let existing_to_this_bucket: i64 = repository::get_existing_allocation_to_bucket(
        &conn,
        input.source_account_id,
        input.bucket_id,
        &input.effective_date,
    )
    .map_err(AppError::from)?;

    // available = balance - (total allocated across all buckets) + (existing to this bucket)
    // This allows the user to re-allocate up to their full available balance for this bucket.
    let available = balance - total_allocated + existing_to_this_bucket;
    if input.amount_minor > available {
        return Err(AppError {
            code: "OVER_ALLOCATION".into(),
            message: "Exceeds available balance".into(),
        });
    }

    let id = repository::create_bucket_allocation(
        &conn,
        input.bucket_id,
        input.source_account_id,
        input.amount_minor,
        &input.effective_date,
    )
    .map_err(AppError::from)?;

    Ok(id)
}

#[tauri::command]
pub fn list_bucket_allocations(
    state: State<'_, AppState>,
    bucket_id: i64,
    as_of_date: String,
) -> Result<Vec<BucketAllocation>, AppError> {
    let conn = state.conn()?;
    let allocations = repository::list_bucket_allocations(&conn, bucket_id, &as_of_date)
        .map_err(AppError::from)?;
    Ok(allocations)
}

#[tauri::command]
pub fn get_account_allocated_total(
    state: State<'_, AppState>,
    source_account_id: i64,
    as_of_date: String,
) -> Result<i64, AppError> {
    let conn = state.conn()?;
    let total = repository::get_account_allocated_total(&conn, source_account_id, &as_of_date)
        .map_err(AppError::from)?;
    Ok(total)
}

#[tauri::command]
pub fn check_over_allocation(
    state: State<'_, AppState>,
    source_account_id: i64,
    as_of_date: String,
) -> Result<Option<OverAllocationWarning>, AppError> {
    let conn = state.conn()?;
    let warning = repository::check_over_allocation(&conn, source_account_id, &as_of_date)
        .map_err(AppError::from)?;
    Ok(warning)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortOrderEntry {
    pub account_id: i64,
    pub sort_order: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSortOrderInput {
    pub entries: Vec<SortOrderEntry>,
}

#[tauri::command]
pub fn update_sort_order(
    state: State<'_, AppState>,
    input: UpdateSortOrderInput,
) -> Result<(), AppError> {
    if input.entries.is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "entries must not be empty".into(),
        });
    }
    let conn = state.conn()?;
    let pairs: Vec<(i64, i64)> = input
        .entries
        .iter()
        .map(|e| (e.account_id, e.sort_order))
        .collect();
    repository::update_sort_order(&conn, &pairs)?;
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
    let demo_conn = db::initialize_in_memory().map_err(AppError::from)?;
    demo::seed_demo_data(&demo_conn, oxr_app_id, theme)?;

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
            let new_conn = db::initialize_db(&new_db_path).map_err(AppError::from)?;
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
                db::wal_checkpoint(&conn).map_err(AppError::from)?;
            }

            // 2. Swap to in-memory to release the file lock on Windows.
            {
                let in_memory = db::initialize_in_memory().map_err(AppError::from)?;
                let mut db_guard = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
                // _old_conn is dropped here, releasing the file lock.
                let _old_conn = std::mem::replace(&mut *db_guard, in_memory);
            }

            // 3. Copy file.
            if let Err(e) = std::fs::copy(&old_path, &new_db_path) {
                if let Ok(restored) = db::initialize_db(&old_path) {
                    if let Ok(mut db_guard) = state.db.lock() {
                        let _ = std::mem::replace(&mut *db_guard, restored);
                    }
                }
                return Err(AppError::from(e));
            }

            // 4. Open new DB (runs pending migrations in case schema is ahead).
            let new_conn = match db::initialize_db(&new_db_path) {
                Ok(c) => c,
                Err(e) => {
                    let _ = std::fs::remove_file(&new_db_path);
                    if let Ok(restored) = db::initialize_db(&old_path) {
                        if let Ok(mut db_guard) = state.db.lock() {
                            let _ = std::mem::replace(&mut *db_guard, restored);
                        }
                    }
                    return Err(AppError::from(e));
                }
            };

            // 5. Integrity check.
            if let Err(e) = db::integrity_check(&new_conn) {
                drop(new_conn);
                let _ = std::fs::remove_file(&new_db_path);
                if let Ok(restored) = db::initialize_db(&old_path) {
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
            let new_conn = db::initialize_db(&new_db_path).map_err(AppError::from)?;
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
