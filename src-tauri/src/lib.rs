use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use tauri::Manager;

mod commands;
mod db;
mod demo;
mod error;
mod migrations;
mod models;
mod oxr;
mod repository;

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    /// Stash for the real persistent connection while demo mode is active.
    pub persistent_db: Mutex<Option<rusqlite::Connection>>,
    /// True when the app is running with the ephemeral in-memory demo database.
    pub demo_mode: AtomicBool,
}

impl AppState {
    pub fn conn(&self) -> Result<std::sync::MutexGuard<'_, rusqlite::Connection>, error::AppError> {
        self.db
            .lock()
            .map_err(|e| error::AppError::from(e.to_string()))
    }
}

/// Fetch FX rates for `date`, skipping the OXR API call when all rates already exist
/// (unless `force` is `true`). Follows the lock/drop/await/lock pattern: the DB mutex
/// is never held across the async HTTP call.
pub async fn smart_fetch_fx_rates(
    state: &AppState,
    date: &str,
    force: bool,
) -> Result<Vec<models::FxRateRow>, error::AppError> {
    // 1. Read API key (lock → read → drop).
    let api_key = {
        let conn = state.conn()?;
        repository::get_app_setting(&conn, "oxr_app_id")?
            .filter(|k| !k.is_empty())
            .ok_or_else(|| error::AppError {
                code: "CONFIG_ERROR".into(),
                message: "API key not configured. Set oxr_app_id in settings.".into(),
            })?
    };

    // 2. Read consolidation currency and active non-consolidation currencies.
    let (consolidation, active_currencies) = {
        let conn = state.conn()?;
        let consolidation = repository::get_consolidation_currency(&conn)?;
        let active = repository::get_active_foreign_currencies(&conn, consolidation.id)?;
        (consolidation, active)
    };

    // 3. If no active currencies, return stored rates immediately.
    if active_currencies.is_empty() {
        let conn = state.conn()?;
        return Ok(repository::list_fx_rates(&conn, Some(date))?);
    }

    // 4. If not forced, check whether all active currencies already have a rate for this date.
    if !force {
        let conn = state.conn()?;
        let has_all = repository::has_all_fx_rates_for_date(
            &conn,
            consolidation.id,
            &active_currencies,
            date,
        )?;
        if has_all {
            return Ok(repository::list_fx_rates(&conn, Some(date))?);
        }
    }

    // 5. HTTP call — no DB lock held across the await.
    let oxr_response = oxr::fetch_rates(&api_key, Some(date)).await?;

    // 6. Compute cross rates; skip currencies that fail.
    let mut rates: Vec<(String, i64, i64, i64, i64)> = Vec::new();
    for (code, id) in &active_currencies {
        match oxr::compute_cross_rate(&oxr_response.rates, &consolidation.code, code) {
            Ok((m, e)) => rates.push((date.to_string(), consolidation.id, *id, m, e)),
            Err(err) => eprintln!(
                "[smart_fetch_fx_rates] cross rate error for {}: {}",
                code, err.message
            ),
        }
    }

    // 7. Upsert rates.
    if !rates.is_empty() {
        let conn = state.conn()?;
        repository::upsert_fx_rates(&conn, &rates)?;
    }

    // 8. Return the stored rates for this date.
    let conn = state.conn()?;
    Ok(repository::list_fx_rates(&conn, Some(date))?)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");
            std::fs::create_dir_all(&app_dir).expect("failed to create app data dir");
            let db_path = app_dir.join("our-finances.db");

            let conn = db::initialize_db(&db_path).expect("failed to initialize database");

            app.manage(AppState {
                db: Mutex::new(conn),
                persistent_db: Mutex::new(None),
                demo_mode: AtomicBool::new(false),
            });

            // Spawn background FX rate fetch — non-blocking, errors are logged.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                startup_auto_fetch(handle).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_balance_update,
            commands::get_accounts_snapshot,
            commands::list_events,
            commands::create_account,
            commands::update_account,
            commands::delete_account,
            commands::update_event,
            commands::delete_event,
            commands::bulk_create_balance_updates,
            commands::list_currencies,
            commands::get_consolidation_currency,
            commands::set_consolidation_currency,
            commands::set_fx_rate_manual,
            commands::list_fx_rates,
            commands::get_app_setting,
            commands::set_app_setting,
            commands::get_missing_rate_dates,
            commands::fetch_fx_rates,
            commands::create_bucket_allocation,
            commands::list_bucket_allocations,
            commands::get_account_allocated_total,
            commands::check_over_allocation,
            commands::update_sort_order,
            commands::enter_demo_mode,
            commands::exit_demo_mode,
            commands::is_demo_mode,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Background task: fetch missing FX rates on startup without blocking the UI.
///
/// Pass 1 (gap-fill, ≤ 30 calendar dates): fills the gap between the most-recently stored
/// rate date and today for each active non-consolidation currency.
///
/// Pass 2 (ledger-driven backfill, uncapped): fetches rates for any date that has a
/// balance-update event in a foreign-currency account but no stored rate.
async fn startup_auto_fetch(handle: tauri::AppHandle) {
    let state = handle.state::<AppState>();

    // --- Skip silently if no API key configured. ---
    {
        let conn = match state.conn() {
            Ok(g) => g,
            Err(_) => return,
        };
        let has_key = repository::get_app_setting(&conn, "oxr_app_id")
            .ok()
            .flatten()
            .map(|k: String| !k.is_empty())
            .unwrap_or(false);
        if !has_key {
            return;
        }
    }

    // --- Read consolidation currency and active non-consolidation currencies. ---
    let (consolidation_id, active_currencies): (i64, Vec<(String, i64)>) = {
        let conn = match state.conn() {
            Ok(g) => g,
            Err(_) => return,
        };
        let consol = match repository::get_consolidation_currency(&conn) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[fx-startup] consolidation lookup failed: {}", e);
                return;
            }
        };
        let active = match repository::get_active_foreign_currencies(&conn, consol.id) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[fx-startup] active currency lookup failed: {}", e);
                return;
            }
        };
        (consol.id, active)
    };

    if active_currencies.is_empty() {
        return;
    }

    let today = chrono::Local::now().date_naive();
    let today_str = today.format("%Y-%m-%d").to_string();

    // --- Pass 1: gap-fill (capped at 30 calendar dates). ---
    let gap_dates: Vec<String> = {
        let conn = match state.conn() {
            Ok(g) => g,
            Err(_) => return,
        };

        let mut needed: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for (_, id) in &active_currencies {
            let latest = match repository::get_latest_fx_rate_date(&conn, consolidation_id, *id) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("[fx-startup] get_latest_fx_rate_date error: {}", e);
                    continue;
                }
            };

            let start = if let Some(latest_str) = latest {
                match chrono::NaiveDate::parse_from_str(&latest_str, "%Y-%m-%d") {
                    Ok(d) => match d.succ_opt() {
                        Some(next) => next,
                        None => continue,
                    },
                    Err(_) => continue,
                }
            } else {
                // No rates stored yet; fill just today.
                today
            };

            let mut cur = start;
            while cur <= today {
                needed.insert(cur.format("%Y-%m-%d").to_string());
                cur = match cur.succ_opt() {
                    Some(n) => n,
                    None => break,
                };
            }
        }

        // Keep the 30 most-recent dates to cap API usage.
        needed.into_iter().rev().take(30).rev().collect()
    };

    for date in &gap_dates {
        if let Err(e) = smart_fetch_fx_rates(&state, date, false).await {
            eprintln!("[fx-startup/gap-fill/{}] error: {}", date, e.message);
        }
    }

    // --- Pass 2: ledger-driven backfill (uncapped). ---
    let ledger_dates: Vec<String> = {
        let conn = match state.conn() {
            Ok(g) => g,
            Err(_) => return,
        };
        match repository::get_dates_needing_fx_rates(&conn, consolidation_id) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[fx-startup] get_dates_needing_fx_rates error: {}", e);
                Vec::new()
            }
        }
    };

    // Exclude dates already fetched in Pass 1.
    let gap_set: std::collections::HashSet<&str> = gap_dates.iter().map(|s| s.as_str()).collect();
    let remaining: Vec<String> = ledger_dates
        .into_iter()
        .filter(|d| !gap_set.contains(d.as_str()) && d.as_str() <= today_str.as_str())
        .collect();

    for date in &remaining {
        if let Err(e) = smart_fetch_fx_rates(&state, date, false).await {
            eprintln!("[fx-startup/ledger-backfill/{}] error: {}", date, e.message);
        }
    }
}
