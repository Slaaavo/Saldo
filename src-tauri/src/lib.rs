use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::Manager;

mod config;
mod db;
mod demo;
mod error;
mod features;
mod migrations;
mod oxr;
mod shared;

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    /// Stash for the real persistent connection while demo mode is active.
    pub persistent_db: Mutex<Option<rusqlite::Connection>>,
    /// True when the app is running with the ephemeral in-memory demo database.
    pub demo_mode: AtomicBool,
    /// Absolute path to the app data directory (always the OS default).
    pub app_data_dir: PathBuf,
    /// Path to the currently active database file (may differ from app_data_dir).
    pub db_path: Mutex<PathBuf>,
    /// Set to true at startup when a configured custom folder is missing; cleared after any
    /// successful location change.
    pub fallback_warning: AtomicBool,
    /// Bumped, with the `db` lock held, on every swap of `db` (demo mode enter/exit, DB location
    /// change). An async task that read from one database and wants to write after an await
    /// compares it first, so its result never lands in a different database.
    db_generation: AtomicU64,
}

impl AppState {
    pub fn new(
        conn: rusqlite::Connection,
        app_data_dir: PathBuf,
        db_path: PathBuf,
        fallback_warning: bool,
    ) -> Self {
        Self {
            db: Mutex::new(conn),
            persistent_db: Mutex::new(None),
            demo_mode: AtomicBool::new(false),
            app_data_dir,
            db_path: Mutex::new(db_path),
            fallback_warning: AtomicBool::new(fallback_warning),
            db_generation: AtomicU64::new(0),
        }
    }

    pub fn conn(&self) -> Result<std::sync::MutexGuard<'_, rusqlite::Connection>, error::AppError> {
        self.db
            .lock()
            .map_err(|e| error::AppError::from(e.to_string()))
    }

    /// The current DB generation. Read it while holding the `db` lock to pair it with the
    /// connection being read.
    pub fn db_generation(&self) -> u64 {
        self.db_generation.load(Ordering::SeqCst)
    }

    /// Replace the connection behind an already-held `db` guard and return the old one. Every
    /// swap of `db` goes through here (or `replace_conn`) so the generation is bumped under the lock.
    pub fn swap_conn(
        &self,
        db: &mut rusqlite::Connection,
        new_conn: rusqlite::Connection,
    ) -> rusqlite::Connection {
        self.db_generation.fetch_add(1, Ordering::SeqCst);
        std::mem::replace(db, new_conn)
    }

    /// Lock `db`, swap in `new_conn` and return the old connection.
    pub fn replace_conn(
        &self,
        new_conn: rusqlite::Connection,
    ) -> Result<rusqlite::Connection, error::AppError> {
        let mut db = self.conn()?;
        Ok(self.swap_conn(&mut db, new_conn))
    }
}

/// Upsert fetched `rates` unless the active database was swapped since `generation` was read
/// (demo mode entered or exited, or the DB location changed, during the HTTP call). Returns
/// whether the rates were written. The generation is compared under the `db` lock, which every
/// swap also holds while bumping it, so a write can never land in a different database.
fn store_fetched_rates(
    state: &AppState,
    generation: u64,
    rates: &[(String, i64, i64, i64, i64)],
) -> Result<bool, error::AppError> {
    let conn = state.conn()?;
    if state.db_generation() != generation {
        return Ok(false);
    }
    features::currency::repository::upsert_fx_rates(&conn, rates)?;
    Ok(true)
}

/// Fetch FX rates for `date`, skipping the OXR API call when all rates already exist
/// (unless `force` is `true`). Follows the lock/drop/await/lock pattern: the DB mutex
/// is never held across the async HTTP call. Rates fetched for a database that is no longer
/// active when the call returns are discarded.
pub async fn smart_fetch_fx_rates(
    state: &AppState,
    date: &str,
    force: bool,
) -> Result<Vec<features::currency::FxRateRow>, error::AppError> {
    // 1. Read API key (lock → read → drop), noting which database generation it came from.
    let (api_key, generation) = {
        let conn = state.conn()?;
        let generation = state.db_generation();
        let api_key = features::settings::repository::get_app_setting(&conn, "oxr_app_id")?
            .filter(|k| !k.is_empty())
            .ok_or_else(|| error::AppError {
                code: "CONFIG_ERROR".into(),
                message: "API key not configured. Set oxr_app_id in settings.".into(),
            })?;
        (api_key, generation)
    };

    // 2. Read consolidation currency and active non-consolidation currencies.
    let (consolidation, active_currencies) = {
        let conn = state.conn()?;
        let consolidation = features::currency::repository::get_consolidation_currency(&conn)?;
        let active =
            features::currency::repository::get_active_foreign_currencies(&conn, consolidation.id)?;
        (consolidation, active)
    };

    // 3. If no active currencies, return stored rates immediately.
    if active_currencies.is_empty() {
        let conn = state.conn()?;
        return Ok(features::currency::repository::list_fx_rates(
            &conn,
            Some(date),
        )?);
    }

    // 4. If not forced, check whether all active currencies already have a rate for this date.
    if !force {
        let conn = state.conn()?;
        let has_all = features::currency::repository::has_all_fx_rates_for_date(
            &conn,
            features::currency::repository::HasAllFxRatesForDateParams {
                consolidation_id: consolidation.id,
                active_currencies: active_currencies.clone(),
                date: date.to_owned(),
            },
        )?;
        if has_all {
            return Ok(features::currency::repository::list_fx_rates(
                &conn,
                Some(date),
            )?);
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

    // 7. Upsert rates, unless the active database changed during the HTTP call.
    if !rates.is_empty() && !store_fetched_rates(state, generation, &rates)? {
        eprintln!(
            "[smart_fetch_fx_rates] database changed while fetching {}; rates discarded",
            date
        );
    }

    // 8. Return the stored rates for this date.
    let conn = state.conn()?;
    Ok(features::currency::repository::list_fx_rates(
        &conn,
        Some(date),
    )?)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");
            std::fs::create_dir_all(&app_dir).expect("failed to create app data dir");

            // Use the custom location when its saldo.db is present; otherwise fall back to
            // app_data_dir with a warning flag.
            let (db_path, fallback_warning) = config::resolve_db_path(&app_dir);

            let conn = db::initialize_db(&db_path).expect("failed to initialize database");

            if let Err(e) = features::transactions::repository::generate_depreciation_events(&conn)
            {
                eprintln!("Depreciation check failed: {}", e.message);
            }

            app.manage(AppState::new(conn, app_dir, db_path, fallback_warning));

            // Spawn background FX rate fetch — non-blocking, errors are logged.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                startup_auto_fetch(handle).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            features::create_balance_update,
            features::get_accounts_snapshot,
            features::list_events,
            features::create_account,
            features::update_account,
            features::delete_account,
            features::update_event,
            features::delete_event,
            features::delete_split_group,
            features::bulk_create_balance_updates,
            features::create_cashflow,
            features::bulk_create_cashflows,
            features::list_currencies,
            features::get_consolidation_currency,
            features::set_consolidation_currency,
            features::set_fx_rate_manual,
            features::list_fx_rates,
            features::get_app_setting,
            features::set_app_setting,
            features::get_missing_rate_dates,
            features::fetch_fx_rates,
            features::create_bucket_balance_update,
            features::update_bucket_balance_update,
            features::list_links_for_event,
            features::get_latest_bucket_links,
            features::update_sort_order,
            features::enter_demo_mode,
            features::exit_demo_mode,
            features::is_demo_mode,
            features::get_db_location,
            features::pick_db_folder,
            features::change_db_location,
            features::reset_db_location,
            features::check_default_db,
            features::create_custom_unit,
            features::list_custom_units,
            features::update_custom_unit,
            features::update_asset_value,
            features::list_account_asset_links,
            features::set_account_asset_links,
            features::get_bulk_update_exclusions,
            features::set_bulk_update_exclusions,
            features::create_partner_account,
            features::list_partner_accounts,
            features::update_partner_account,
            features::delete_partner_account,
            features::create_split_group,
            features::get_event_by_id,
            features::update_transfer,
            features::list_import_profiles,
            features::create_import_profile,
            features::update_import_profile,
            features::delete_import_profile,
            features::get_preferred_profile,
            features::set_preferred_profile,
            features::create_person,
            features::list_persons,
            features::update_person,
            features::delete_person,
            features::create_tax_model,
            features::list_tax_models,
            features::get_tax_model,
            features::update_tax_model,
            features::delete_tax_model,
            features::save_tax_forecast_figures,
            features::create_taxable_event,
            features::create_taxable_split_group,
            features::update_taxable_event,
            features::update_taxable_split_group,
            features::link_cashflows_to_taxable,
            features::link_cashflows_to_split_group,
            features::unlink_cashflow_from_taxable,
            features::list_linked_cashflows,
            features::list_eligible_cashflows,
            features::get_unmatched_cashflow_count,
            features::calculate_tax_model,
            features::delete_ekasa_profile,
            features::get_ekasa_profile,
            features::list_ekasa_profiles,
            features::upsert_ekasa_profile,
            features::process_receipt_file,
        ])
        .run({
            let mut ctx = tauri::generate_context!();
            ctx.set_default_window_icon(Some(
                tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))
                    .expect("failed to load window icon"),
            ));
            ctx
        })
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
    // The date lists below are derived from this database; the generation lets the fetch loops
    // stop once demo mode or the DB location has swapped it for another one.
    let generation = {
        let conn = match state.conn() {
            Ok(g) => g,
            Err(_) => return,
        };
        let has_key = features::settings::repository::get_app_setting(&conn, "oxr_app_id")
            .ok()
            .flatten()
            .map(|k: String| !k.is_empty())
            .unwrap_or(false);
        if !has_key {
            return;
        }
        state.db_generation()
    };

    // --- Read consolidation currency and active non-consolidation currencies. ---
    let (consolidation_id, active_currencies): (i64, Vec<(String, i64)>) = {
        let conn = match state.conn() {
            Ok(g) => g,
            Err(_) => return,
        };
        let consol = match features::currency::repository::get_consolidation_currency(&conn) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[fx-startup] consolidation lookup failed: {}", e);
                return;
            }
        };
        let active =
            match features::currency::repository::get_active_foreign_currencies(&conn, consol.id) {
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
            let latest = match features::currency::repository::get_latest_fx_rate_date(
                &conn,
                features::currency::repository::GetLatestFxRateDateParams {
                    from_currency_id: consolidation_id,
                    to_currency_id: *id,
                },
            ) {
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

    let app_state: &AppState = &state;
    let db_unchanged = || app_state.db_generation() == generation;
    let fetch = |date: String| async move { smart_fetch_fx_rates(app_state, &date, false).await };

    if !fetch_dates("gap-fill", &gap_dates, &db_unchanged, &fetch).await {
        return;
    }

    // --- Pass 2: ledger-driven backfill (uncapped). ---
    let ledger_dates: Vec<String> = {
        let conn = match state.conn() {
            Ok(g) => g,
            Err(_) => return,
        };
        match features::currency::repository::get_dates_needing_fx_rates(&conn, consolidation_id) {
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

    fetch_dates("ledger-backfill", &remaining, &db_unchanged, &fetch).await;
}

/// An error every remaining date would hit too: a missing key (`CONFIG_ERROR`) or a key OXR
/// refused or throttled (`OXR_ACCESS_ERROR`). Other OXR errors can be specific to one date.
fn is_fatal_fx_error(e: &error::AppError) -> bool {
    matches!(e.code.as_str(), "OXR_ACCESS_ERROR" | "CONFIG_ERROR")
}

/// Fetches rates for each of `dates` in turn, logging failures. Returns `false` when the whole
/// startup fetch should stop: the database was swapped, so the date lists are stale, or a fatal
/// error means further requests are pointless.
async fn fetch_dates<F, Fut>(
    pass: &str,
    dates: &[String],
    db_unchanged: &impl Fn() -> bool,
    fetch: &F,
) -> bool
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<features::currency::FxRateRow>, error::AppError>>,
{
    for date in dates {
        if !db_unchanged() {
            return false;
        }
        if let Err(e) = fetch(date.clone()).await {
            eprintln!("[fx-startup/{}/{}] error: {}", pass, date, e.message);
            if is_fatal_fx_error(&e) {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> AppState {
        let conn = db::initialize_in_memory().expect("DB init failed");
        AppState::new(conn, PathBuf::new(), PathBuf::new(), false)
    }

    fn eur_to_usd_rates(state: &AppState) -> Vec<(String, i64, i64, i64, i64)> {
        let conn = state.conn().unwrap();
        let id = |code: &str| -> i64 {
            conn.query_row("SELECT id FROM currency WHERE code = ?1", [code], |r| {
                r.get(0)
            })
            .unwrap()
        };
        vec![("2026-03-01".to_string(), id("EUR"), id("USD"), 10842, -4)]
    }

    fn stored_rate_count(state: &AppState) -> i64 {
        let conn = state.conn().unwrap();
        conn.query_row("SELECT COUNT(*) FROM fx_rate", [], |r| r.get(0))
            .unwrap()
    }

    fn app_error(code: &str) -> error::AppError {
        error::AppError {
            code: code.into(),
            message: "failed".into(),
        }
    }

    /// Runs `fetch_dates` over three dates where the fetch for the second one fails with `code`;
    /// returns the dates that were requested and the result.
    fn run_fetch_dates_failing_second(code: &str) -> (Vec<String>, bool) {
        let dates: Vec<String> = ["2026-03-01", "2026-03-02", "2026-03-03"]
            .map(String::from)
            .to_vec();
        let requested = Mutex::new(Vec::new());
        let completed = tauri::async_runtime::block_on(fetch_dates(
            "test",
            &dates,
            &|| true,
            &|date: String| {
                requested.lock().unwrap().push(date.clone());
                let result = if date == "2026-03-02" {
                    Err(app_error(code))
                } else {
                    Ok(Vec::new())
                };
                async move { result }
            },
        ));
        (requested.into_inner().unwrap(), completed)
    }

    #[test]
    fn fetch_dates_stops_on_an_oxr_access_error() {
        let (requested, completed) = run_fetch_dates_failing_second("OXR_ACCESS_ERROR");
        assert_eq!(requested, ["2026-03-01", "2026-03-02"]);
        assert!(!completed);
    }

    #[test]
    fn fetch_dates_continues_past_an_oxr_error_for_one_date() {
        let (requested, completed) = run_fetch_dates_failing_second("OXR_ERROR");
        assert_eq!(requested, ["2026-03-01", "2026-03-02", "2026-03-03"]);
        assert!(completed);
    }

    #[test]
    fn fetch_dates_stops_on_a_config_error() {
        let (requested, completed) = run_fetch_dates_failing_second("CONFIG_ERROR");
        assert_eq!(requested, ["2026-03-01", "2026-03-02"]);
        assert!(!completed);
    }

    #[test]
    fn fetch_dates_continues_past_other_errors() {
        let (requested, completed) = run_fetch_dates_failing_second("NETWORK_ERROR");
        assert_eq!(requested, ["2026-03-01", "2026-03-02", "2026-03-03"]);
        assert!(completed);
    }

    #[test]
    fn fetch_dates_stops_when_the_database_changes() {
        let dates: Vec<String> = ["2026-03-01", "2026-03-02"].map(String::from).to_vec();
        let unchanged = AtomicBool::new(true);
        let requested = Mutex::new(Vec::new());
        let completed = tauri::async_runtime::block_on(fetch_dates(
            "test",
            &dates,
            &|| unchanged.load(Ordering::SeqCst),
            &|date: String| {
                requested.lock().unwrap().push(date);
                unchanged.store(false, Ordering::SeqCst);
                async { Ok(Vec::new()) }
            },
        ));
        assert_eq!(requested.into_inner().unwrap(), ["2026-03-01"]);
        assert!(!completed);
    }

    #[test]
    fn store_fetched_rates_writes_when_the_database_is_unchanged() {
        let state = test_state();
        let rates = eur_to_usd_rates(&state);
        let generation = state.db_generation();

        assert!(store_fetched_rates(&state, generation, &rates).unwrap());
        assert_eq!(stored_rate_count(&state), 1);
    }

    #[test]
    fn store_fetched_rates_discards_rates_after_the_database_was_swapped() {
        let state = test_state();
        let rates = eur_to_usd_rates(&state);
        let generation = state.db_generation();

        let other_db = db::initialize_in_memory().expect("DB init failed");
        state.replace_conn(other_db).unwrap();

        assert!(!store_fetched_rates(&state, generation, &rates).unwrap());
        assert_eq!(stored_rate_count(&state), 0);
    }

    #[test]
    fn store_fetched_rates_discards_rates_after_a_swap_and_swap_back() {
        let state = test_state();
        let rates = eur_to_usd_rates(&state);
        let generation = state.db_generation();

        let first = state
            .replace_conn(db::initialize_in_memory().expect("DB init failed"))
            .unwrap();
        state.replace_conn(first).unwrap();

        assert!(!store_fetched_rates(&state, generation, &rates).unwrap());
        assert_eq!(stored_rate_count(&state), 0);
    }
}
