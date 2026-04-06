use rusqlite::{params, OptionalExtension};
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

    // Validate each source account is a regular account.
    for &source_id in &input.linked_account_ids {
        match crate::features::accounts::repository::get_account_type(&conn, source_id)? {
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

    with_savepoint_app(&conn, || {
        let event_id = crate::features::transactions::repository::create_balance_update_inner(
            &conn,
            input.account_id,
            input.amount_minor,
            &input.event_date,
            input.note.as_deref(),
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

    // Validate each source account is a regular account.
    for &source_id in &input.linked_account_ids {
        match crate::features::accounts::repository::get_account_type(&conn, source_id)? {
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

    with_savepoint_app(&conn, || {
        crate::features::transactions::repository::check_event_split_group_date_conflict(
            &conn,
            input.event_id,
            &input.event_date,
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
            input.event_id,
            input.amount_minor,
            &input.event_date,
            input.note.as_deref(),
        )
        .map_err(|s| AppError {
            code: "APP_ERROR".into(),
            message: s,
        })?;

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

fn format_conflict_message(conflict: &LinkConflict) -> String {
    serde_json::to_string(conflict).unwrap_or_default()
}
