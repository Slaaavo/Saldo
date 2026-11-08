use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use tauri::State;

use crate::error::AppError;
use crate::shared::with_savepoint_app;
use crate::AppState;

use super::models::{BucketLink, LinkConflict};
use super::repository;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBucketBalanceUpdateInput {
    pub account_id: i64,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub linked_account_ids: Vec<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBucketBalanceUpdateInput {
    pub event_id: i64,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub linked_account_ids: Vec<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetBucketEnabledInput {
    pub bucket_id: i64,
    pub enabled: bool,
}

#[tauri::command]
pub fn set_bucket_enabled(
    state: State<'_, AppState>,
    input: SetBucketEnabledInput,
) -> Result<(), AppError> {
    let conn = state.conn()?;
    set_bucket_enabled_inner(&conn, input)
}

pub(crate) fn set_bucket_enabled_inner(
    conn: &Connection,
    input: SetBucketEnabledInput,
) -> Result<(), AppError> {
    let switched = repository::set_bucket_enabled(
        conn,
        repository::SetBucketEnabledParams {
            bucket_id: input.bucket_id,
            enabled: input.enabled,
        },
    )?;
    if !switched {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Only a bucket can be switched on or off.".into(),
        });
    }
    Ok(())
}

#[tauri::command]
pub fn create_bucket_balance_update(
    state: State<'_, AppState>,
    input: CreateBucketBalanceUpdateInput,
) -> Result<i64, AppError> {
    crate::shared::validate_event_date(&input.event_date)?;
    if input.amount_minor < 0 {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "amount_minor must be >= 0".into(),
        });
    }
    let conn = state.conn()?;

    // Validate that account_id refers to a bucket account.
    match crate::features::accounts::repository::get_account_type(&conn, input.account_id)? {
        Some(ref t) if t == "bucket" => {}
        Some(_) => {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "account_id must refer to a bucket account".into(),
            });
        }
        None => {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "account_id not found".into(),
            });
        }
    }

    validate_source_accounts(&conn, &input.linked_account_ids)?;

    with_savepoint_app(&conn, || {
        let event_id = crate::features::transactions::repository::create_balance_update_inner(
            &conn,
            crate::features::transactions::repository::CreateBalanceUpdateParams {
                account_id: input.account_id,
                amount_minor: input.amount_minor,
                event_date: input.event_date.clone(),
                note: input.note.clone(),
            },
        )?;

        if let Some(conflict) = repository::check_link_conflicts(
            &conn,
            repository::CheckLinkConflictsParams {
                target_bucket_id: input.account_id,
                new_event_id: event_id,
                new_event_date: input.event_date.clone(),
                proposed_account_ids: input.linked_account_ids.clone(),
            },
        )? {
            return Err(AppError {
                code: "LINK_CONFLICT".into(),
                message: format_conflict_message(&conflict),
            });
        }

        repository::set_bucket_event_links(
            &conn,
            repository::SetBucketEventLinksParams {
                event_id,
                account_ids: input.linked_account_ids,
            },
        )?;

        Ok(event_id)
    })
}

#[tauri::command]
pub fn update_bucket_balance_update(
    state: State<'_, AppState>,
    input: UpdateBucketBalanceUpdateInput,
) -> Result<(), AppError> {
    crate::shared::validate_event_date(&input.event_date)?;
    if input.amount_minor < 0 {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "amount_minor must be >= 0".into(),
        });
    }
    let conn = state.conn()?;

    // Look up the bucket_id from the event.
    let bucket_id: Option<i64> = conn
        .query_row(
            "SELECT e.account_id FROM event e WHERE e.id = ?1 AND e.deleted_at IS NULL",
            params![input.event_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(AppError::from)?;
    let bucket_id = bucket_id.ok_or_else(|| AppError {
        code: "VALIDATION".into(),
        message: "Event not found or deleted".into(),
    })?;

    // Verify it is a bucket account.
    match crate::features::accounts::repository::get_account_type(&conn, bucket_id)? {
        Some(ref t) if t == "bucket" => {}
        _ => {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Event does not belong to a bucket account".into(),
            });
        }
    }

    validate_source_accounts(&conn, &input.linked_account_ids)?;

    with_savepoint_app(&conn, || {
        crate::features::transactions::repository::check_event_split_group_date_conflict(
            &conn,
            crate::features::transactions::repository::CheckEventSplitGroupDateConflictParams {
                event_id: input.event_id,
                new_date: input.event_date.clone(),
            },
        )?;

        if let Some(conflict) = repository::check_link_conflicts(
            &conn,
            repository::CheckLinkConflictsParams {
                target_bucket_id: bucket_id,
                new_event_id: input.event_id,
                new_event_date: input.event_date.clone(),
                proposed_account_ids: input.linked_account_ids.clone(),
            },
        )? {
            return Err(AppError {
                code: "LINK_CONFLICT".into(),
                message: format_conflict_message(&conflict),
            });
        }

        crate::features::transactions::repository::update_event(
            &conn,
            crate::features::transactions::repository::UpdateEventParams {
                event_id: input.event_id,
                amount_minor: input.amount_minor,
                event_date: input.event_date.clone(),
                note: input.note.clone(),
            },
        )?;

        repository::set_bucket_event_links(
            &conn,
            repository::SetBucketEventLinksParams {
                event_id: input.event_id,
                account_ids: input.linked_account_ids,
            },
        )?;

        Ok(())
    })
}

#[tauri::command]
pub fn list_links_for_event(
    state: State<'_, AppState>,
    event_id: i64,
) -> Result<Vec<BucketLink>, AppError> {
    let conn = state.conn()?;
    repository::list_links_for_event(&conn, event_id).map_err(AppError::from)
}

#[tauri::command]
pub fn get_latest_bucket_links(
    state: State<'_, AppState>,
    bucket_account_id: i64,
    as_of_date: String,
) -> Result<Vec<BucketLink>, AppError> {
    let conn = state.conn()?;
    repository::list_latest_links_for_bucket(
        &conn,
        repository::ListLatestLinksParams {
            bucket_account_id,
            as_of_date,
        },
    )
    .map_err(AppError::from)
}

/// Every source account a bucket balance is allocated from must be a regular account.
fn validate_source_accounts(conn: &Connection, linked_account_ids: &[i64]) -> Result<(), AppError> {
    for &source_id in linked_account_ids {
        match crate::features::accounts::repository::get_account_type(conn, source_id)? {
            Some(ref t) if t == "account" => {}
            Some(_) => {
                return Err(AppError {
                    code: "VALIDATION".into(),
                    message: "linked_account_ids must all refer to regular account entries".into(),
                });
            }
            None => {
                return Err(AppError {
                    code: "VALIDATION".into(),
                    message: "a linked account_id was not found".into(),
                });
            }
        }
    }
    Ok(())
}

fn format_conflict_message(conflict: &LinkConflict) -> String {
    serde_json::to_string(conflict).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::{create_account, CreateAccountParams};

    fn mk_account(conn: &Connection, account_type: &str) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: format!("{account_type} test"),
                account_type: account_type.to_owned(),
                currency_id: 1,
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn switch_of(conn: &Connection, bucket_id: i64) -> Option<bool> {
        crate::features::transactions::repository::rows_at(conn, "2026-11-08")
            .into_iter()
            .find(|row| row.account_id == bucket_id)
            .and_then(|row| row.bucket)
            .map(|bucket| bucket.enabled)
    }

    fn set(conn: &Connection, bucket_id: i64, enabled: bool) -> Result<(), AppError> {
        set_bucket_enabled_inner(conn, SetBucketEnabledInput { bucket_id, enabled })
    }

    #[test]
    fn set_bucket_enabled_is_absolute_and_idempotent() {
        let conn = initialize_in_memory().unwrap();
        let bucket = mk_account(&conn, "bucket");
        assert_eq!(switch_of(&conn, bucket), Some(true));

        set(&conn, bucket, false).unwrap();
        set(&conn, bucket, false).unwrap();
        assert_eq!(switch_of(&conn, bucket), Some(false));

        set(&conn, bucket, true).unwrap();
        set(&conn, bucket, true).unwrap();
        assert_eq!(switch_of(&conn, bucket), Some(true));
    }

    #[test]
    fn set_bucket_enabled_refuses_a_non_bucket() {
        let conn = initialize_in_memory().unwrap();
        let account = mk_account(&conn, "account");
        let asset = mk_account(&conn, "asset");

        for id in [account, asset, 99_999] {
            let err = set(&conn, id, false).unwrap_err();
            assert_eq!(err.code, "VALIDATION", "{id}");
        }
        let stored: Vec<i64> = conn
            .prepare("SELECT bucket_enabled FROM account WHERE id IN (?1, ?2)")
            .unwrap()
            .query_map(params![account, asset], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(stored, [1, 1]);
    }

    #[test]
    fn update_account_keeps_the_switch_and_the_kind() {
        let conn = initialize_in_memory().unwrap();
        let bucket = mk_account(&conn, "bucket");
        set(&conn, bucket, false).unwrap();

        crate::features::accounts::commands::update_account_with_conn(
            &conn,
            crate::features::accounts::commands::UpdateAccountInput {
                account_id: bucket,
                name: "Renamed".to_owned(),
                iban: None,
                person_id: Some(1),
                purchase_price_minor: None,
                purchase_date: None,
                depreciation_period_months: None,
                asset_use: None,
            },
        )
        .unwrap();

        assert_eq!(switch_of(&conn, bucket), Some(false));
    }

    #[test]
    fn source_accounts_accept_regular_accounts_and_an_empty_list() {
        let conn = initialize_in_memory().unwrap();
        let account = mk_account(&conn, "account");

        validate_source_accounts(&conn, &[]).unwrap();
        validate_source_accounts(&conn, &[account]).unwrap();
    }

    #[test]
    fn source_accounts_reject_a_non_regular_account() {
        let conn = initialize_in_memory().unwrap();
        let account = mk_account(&conn, "account");
        let bucket = mk_account(&conn, "bucket");

        let err = validate_source_accounts(&conn, &[account, bucket]).unwrap_err();

        assert_eq!(err.code, "VALIDATION");
        assert_eq!(
            err.message,
            "linked_account_ids must all refer to regular account entries"
        );
    }

    #[test]
    fn source_accounts_reject_an_unknown_id() {
        let conn = initialize_in_memory().unwrap();
        let account = mk_account(&conn, "account");

        let err = validate_source_accounts(&conn, &[account, 99_999]).unwrap_err();

        assert_eq!(err.code, "VALIDATION");
        assert_eq!(err.message, "a linked account_id was not found");
    }
}
