use std::sync::Mutex;
use tauri::Manager;

mod commands;
mod db;
mod error;
mod migrations;
mod models;
mod oxr;
mod repository;

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
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

    // --- Read API key; skip silently if absent. ---
    let api_key: String = {
        let conn = match state.db.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        match repository::get_app_setting(&conn, "oxr_app_id") {
            Ok(Some(k)) if !k.is_empty() => k,
            _ => return,
        }
    };

    // --- Read consolidation currency and active non-consolidation currencies. ---
    let (consolidation_id, consolidation_code, active_currencies): (
        i64,
        String,
        Vec<(String, i64)>,
    ) = {
        let conn = match state.db.lock() {
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
        (consol.id, consol.code, active)
    };

    if active_currencies.is_empty() {
        return;
    }

    let today = chrono::Local::now().date_naive();
    let today_str = today.format("%Y-%m-%d").to_string();

    // --- Pass 1: gap-fill (capped at 30 calendar dates). ---
    let gap_dates: Vec<String> = {
        let conn = match state.db.lock() {
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

    fetch_and_upsert_dates(
        &state,
        &api_key,
        consolidation_id,
        &consolidation_code,
        &active_currencies,
        &gap_dates,
        "gap-fill",
    )
    .await;

    // --- Pass 2: ledger-driven backfill (uncapped). ---
    let ledger_dates: Vec<String> = {
        let conn = match state.db.lock() {
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

    fetch_and_upsert_dates(
        &state,
        &api_key,
        consolidation_id,
        &consolidation_code,
        &active_currencies,
        &remaining,
        "ledger-backfill",
    )
    .await;
}

/// Fetch OXR rates for each date in `dates` and upsert them.  Errors are logged per-date;
/// the loop always continues with remaining dates.
async fn fetch_and_upsert_dates(
    state: &tauri::State<'_, AppState>,
    api_key: &str,
    consolidation_id: i64,
    consolidation_code: &str,
    active_currencies: &[(String, i64)],
    dates: &[String],
    pass_name: &str,
) {
    for date in dates {
        let resp = match oxr::fetch_rates(api_key, Some(date)).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "[fx-startup/{}/{}] fetch error: {}",
                    pass_name, date, e.message
                );
                continue;
            }
        };

        let mut rates: Vec<(String, i64, i64, i64, i64)> = Vec::new();
        for (code, id) in active_currencies {
            match oxr::compute_cross_rate(&resp.rates, consolidation_code, code) {
                Ok((m, e)) => rates.push((date.clone(), consolidation_id, *id, m, e)),
                Err(err) => eprintln!(
                    "[fx-startup/{}/{}] cross rate error for {}: {}",
                    pass_name, date, code, err.message
                ),
            }
        }

        if rates.is_empty() {
            continue;
        }

        match state.db.lock() {
            Ok(conn) => {
                if let Err(e) = repository::upsert_fx_rates(&conn, &rates) {
                    eprintln!("[fx-startup/{}/{}] upsert error: {}", pass_name, date, e);
                }
            }
            Err(_) => eprintln!("[fx-startup/{}/{}] DB lock poisoned", pass_name, date),
        }
    }
}
