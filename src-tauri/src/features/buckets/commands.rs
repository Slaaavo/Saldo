use crate::error::AppError;
use crate::AppState;
use serde::Deserialize;
use tauri::State;

use super::models::{BucketAllocation, OverAllocationWarning};
use super::repository;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBucketAllocationInput {
    bucket_id: i64,
    source_account_id: i64,
    amount_minor: i64,
    effective_date: String,
}

#[tauri::command]
pub fn create_bucket_allocation(
    state: State<'_, AppState>,
    input: CreateBucketAllocationInput,
) -> Result<i64, AppError> {
    crate::shared::validate_event_date(&input.effective_date)?;
    if input.amount_minor < 0 {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "amount_minor must be >= 0".into(),
        });
    }
    let conn = state.conn()?;

    // Validate that bucket_id refers to a bucket account.
    let bucket_type: Option<String> =
        crate::features::accounts::repository::get_account_type(&conn, input.bucket_id)
            .map_err(AppError::from)?;
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

    // Validate that source_account_id refers to a regular account or asset.
    let source_type: Option<String> =
        crate::features::accounts::repository::get_account_type(&conn, input.source_account_id)
            .map_err(AppError::from)?;
    match source_type.as_deref() {
        Some("account") | Some("asset") => {}
        Some(_) => {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "source_account_id must refer to a regular account or asset".into(),
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
    let balance = crate::features::accounts::repository::get_account_balance_at_date(
        &conn,
        input.source_account_id,
        &selected_datetime,
    )
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
