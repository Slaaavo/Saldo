mod derive;
pub mod models;
mod repository;
#[cfg(test)]
mod tests;

use std::collections::{BTreeSet, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};

use chrono::{Duration, NaiveDate};
use rusqlite::Connection;

use crate::error::AppError;
use crate::features::accounts::commands::{create_account_with_conn, CreateAccountInput};
use crate::features::currency::repository::get_consolidation_currency;
use crate::features::persons::repository::get_person_vat_payer;
use crate::features::tax_models::calculation::calculate_net_amount;
use crate::features::tax_models::repository::list_tax_models;
use crate::features::transactions::commands::{
    create_taxable_event_inner, create_taxable_split_group_inner, delete_event_inner,
    delete_split_group_inner, CreateTaxableEventInput, CreateTaxableSplitGroupInput,
    SplitTaxableLegInput,
};
use crate::features::transactions::models::{Payment, RecordedSource, TaxableKind};
use crate::features::transactions::repository::{
    find_document_entries, find_payments, find_recorded_source, link_cashflows_to_split_group,
    link_cashflows_to_taxable, FindDocumentEntriesParams, FindPaymentsParams, LinkCashflowsParams,
    LinkCashflowsToSplitGroupParams,
};
use crate::features::transactions::SourceRef;
use crate::shared::{with_savepoint_app, Finish, WriteTxn};
use derive::{Derived, EntryShape};
use models::*;
use repository::{Root, StampParams};

const SIMILAR_DOCUMENT_WINDOW_DAYS: i64 = 14;

pub fn record_batch(
    txn: WriteTxn<'_>,
    input: RecordBatchInput,
) -> Result<BatchReceipt, RecordBatchError> {
    let conn = txn.conn();
    let label = input.label.trim().to_owned();
    if label.is_empty() || input.items.is_empty() {
        return Err(invalid("A batch needs a label and at least one document").into());
    }
    let review_token = review_token(&label, &input.items);
    if !input.dry_run && input.review_token.as_deref() != Some(review_token.as_str()) {
        return Err(RecordBatchError::ReviewTokenMismatch);
    }
    let batch_id = repository::create_batch(
        conn,
        repository::CreateBatchParams {
            label,
            document_count: input.items.len() as i64,
        },
    )?;
    let ctx = ItemContext {
        batch_id,
        asset_currency_id: get_consolidation_currency(conn)?.id,
    };
    let mut documents = Vec::new();
    let mut errors = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    for (index, item) in input.items.into_iter().enumerate() {
        let recorded = record_item(conn, &ctx, &mut seen, index, item);
        txn.ensure_open()?;
        match recorded {
            Ok(document) => documents.push(document),
            Err(error) => errors.push(*error),
        }
    }
    if !errors.is_empty() {
        documents.iter_mut().for_each(RecordedDocument::forget_ids);
        return Err(RecordBatchError::Items {
            dry_run: input.dry_run,
            errors,
            passed: documents,
        });
    }
    repository::record_what_the_batch_made(conn, batch_id)?;
    let affected_tax_model_ids =
        affected_tax_models(conn, &repository::list_person_years(conn, batch_id)?)?;
    if input.dry_run {
        txn.finish(Finish::Rollback)?;
        documents.iter_mut().for_each(RecordedDocument::forget_ids);
    } else {
        txn.finish(Finish::Commit)?;
    }
    Ok(BatchReceipt {
        dry_run: input.dry_run,
        review_token: input.dry_run.then_some(review_token),
        batch_id: (!input.dry_run).then_some(batch_id),
        documents,
        affected_tax_model_ids,
    })
}

pub fn undo_batch(txn: WriteTxn<'_>, input: UndoBatchInput) -> Result<UndoReceipt, UndoBatchError> {
    let conn = txn.conn();
    let batch_id = input.batch_id;
    let already_undone = repository::find_batch_undone(conn, batch_id)?
        .ok_or(UndoBatchError::NotFound { batch_id })?;
    if already_undone {
        return Ok(UndoReceipt {
            batch_id,
            already_undone: true,
            removed_event_ids: Vec::new(),
            removed_asset_account_ids: Vec::new(),
            skipped_already_deleted: Vec::new(),
            affected_tax_model_ids: Vec::new(),
        });
    }
    let changed = repository::list_changed_rows(conn, batch_id)?;
    if !changed.is_empty() && !input.include_changed {
        return Err(UndoBatchError::Changed {
            batch_id,
            rows: changed,
        });
    }
    let entries = repository::list_root_entries(conn, batch_id)?;
    let asset_ids = repository::list_batch_asset_ids(conn, batch_id)?;
    let person_years = repository::list_person_years(conn, batch_id)?;

    let mut removed_event_ids = Vec::new();
    let mut skipped_already_deleted = Vec::new();
    let mut removed_groups = BTreeSet::new();
    for entry in entries {
        if entry.deleted {
            skipped_already_deleted.push(entry.event_id);
            continue;
        }
        match entry.split_group_id {
            Some(group) => {
                if removed_groups.insert(group) {
                    delete_split_group_inner(conn, group)?;
                }
            }
            None => delete_event_inner(conn, entry.event_id)?,
        }
        txn.ensure_open()?;
        removed_event_ids.push(entry.event_id);
    }
    for &account_id in &asset_ids {
        crate::features::accounts::repository::delete_account(conn, account_id)?;
        txn.ensure_open()?;
    }
    repository::mark_undone(conn, batch_id)?;
    let affected_tax_model_ids = affected_tax_models(conn, &person_years)?;
    txn.finish(Finish::Commit)?;
    Ok(UndoReceipt {
        batch_id,
        already_undone: false,
        removed_event_ids,
        removed_asset_account_ids: asset_ids,
        skipped_already_deleted,
        affected_tax_model_ids,
    })
}

pub fn list_recent_batches(conn: &Connection, limit: u32) -> Result<Vec<BatchSummary>, AppError> {
    repository::list_recent_batches(conn, limit)
}

struct ItemContext {
    batch_id: i64,
    asset_currency_id: i64,
}

fn record_item(
    conn: &Connection,
    ctx: &ItemContext,
    seen: &mut HashMap<String, usize>,
    index: usize,
    item: DocumentItem,
) -> Result<RecordedDocument, Box<ItemError>> {
    let fail = |source_ref: Option<&SourceRef>, e: AppError| {
        Box::new(ItemError {
            index,
            file: item.document.file.clone(),
            source_ref: source_ref.cloned(),
            code: e.code,
            message: e.message,
            recorded: None,
        })
    };
    let vat_payer = get_person_vat_payer(conn, item.person_id).map_err(|e| fail(None, e))?;
    let derived = derive::document(&item, vat_payer).map_err(|e| fail(None, e))?;
    let source_ref = &derived.source_ref;
    if let Some(&first) = seen.get(source_ref.as_str()) {
        return Err(fail(
            Some(source_ref),
            AppError {
                code: "DUPLICATE_IN_BATCH".into(),
                message: format!("This is the same document as item {first} of this batch"),
            },
        ));
    }
    seen.insert(source_ref.as_str().to_owned(), index);
    if let Some(recorded) =
        find_recorded_source(conn, source_ref).map_err(|e| fail(Some(source_ref), e))?
    {
        let mut error = fail(
            Some(source_ref),
            AppError {
                code: "ALREADY_RECORDED".into(),
                message: already_recorded_message(&recorded),
            },
        );
        error.recorded = Some(recorded);
        return Err(error);
    }
    let payments = load_payments(conn, &item.payment_ids).map_err(|e| fail(Some(source_ref), e))?;
    let similar =
        similar_documents(conn, &item, &derived).map_err(|e| fail(Some(source_ref), e))?;
    let written = with_savepoint_app(conn, || write_item(conn, ctx, &item, &derived))
        .map_err(|e| fail(Some(source_ref), e))?;

    let total_gross_minor = written.lines.iter().map(|l| l.gross_minor).sum::<i64>()
        + asset_parts(&item.content)
            .iter()
            .map(|a| a.gross_minor)
            .sum::<i64>();
    let mut warnings = Vec::new();
    if !payments.is_empty() {
        let payments_total_minor: i64 = payments.iter().map(|p| p.amount_minor).sum();
        if payments_total_minor.abs() != total_gross_minor.abs() {
            warnings.push(Warning::PaymentAmountDiffers {
                payments_total_minor,
                document_total_minor: total_gross_minor,
            });
        }
    }
    let payment_sign = expected_payment_sign(&item.content, total_gross_minor);
    for payment in payments
        .iter()
        .filter(|p| payment_sign != 0 && p.amount_minor.signum() == -payment_sign)
    {
        warnings.push(Warning::PaymentDirectionDiffers {
            payment_id: payment.event_id,
            amount_minor: payment.amount_minor,
        });
    }
    for payment in payments.iter().filter(|p| !p.linked_entry_ids.is_empty()) {
        warnings.push(Warning::PaymentAlreadyLinked {
            payment_id: payment.event_id,
            linked_entry_ids: payment.linked_entry_ids.clone(),
        });
    }
    warnings.extend(similar);

    Ok(RecordedDocument {
        index,
        file: item.document.file,
        source_ref: derived.source_ref,
        person_id: item.person_id,
        entry_date: derived.entry_date,
        date_source: derived.date_source,
        shape: written.shape,
        total_gross_minor,
        lines: written.lines,
        assets: written.assets,
        linked_payment_ids: item.payment_ids,
        warnings,
    })
}

struct Written {
    shape: RecordedShape,
    lines: Vec<RecordedLine>,
    assets: Vec<RecordedAsset>,
}

struct Line {
    gross_minor: i64,
    vat_rate_bps: i64,
    deductible_pct_bps: Option<i64>,
    vat_reclaimable_pct_bps: Option<i64>,
}

fn write_item(
    conn: &Connection,
    ctx: &ItemContext,
    item: &DocumentItem,
    derived: &Derived,
) -> Result<Written, AppError> {
    let (event_type, lines, prepaid_until) = match &item.content {
        DocumentContent::Revenue(revenue) => (
            "revenue",
            revenue
                .lines
                .iter()
                .map(|l| Line {
                    gross_minor: l.gross_minor,
                    vat_rate_bps: l.vat_rate_bps.get(),
                    deductible_pct_bps: None,
                    vat_reclaimable_pct_bps: None,
                })
                .collect(),
            None,
        ),
        DocumentContent::Expense(expense) => (
            "expense",
            expense.lines.iter().map(expense_line).collect(),
            None,
        ),
        DocumentContent::PrepaidExpense(prepaid) => (
            "prepaid_expense",
            vec![expense_line(&prepaid.line)],
            Some(prepaid.prepaid_until.clone()),
        ),
    };

    let shape = match derived.entries {
        EntryShape::None => RecordedShape::AssetsOnly,
        EntryShape::Single => {
            let line = &lines[0];
            let event_id = create_taxable_event_inner(
                conn,
                CreateTaxableEventInput {
                    person_id: item.person_id,
                    event_type: event_type.to_owned(),
                    amount_minor: line.gross_minor,
                    event_date: derived.entry_date.clone(),
                    note: item.note.clone(),
                    vat_rate_bps: Some(line.vat_rate_bps),
                    vat_reclaimable_pct_bps: line.vat_reclaimable_pct_bps,
                    expense_deductible_pct_bps: line.deductible_pct_bps,
                    prepaid_until: prepaid_until.clone(),
                    reclaimed_vat: derived.reclaimed_vat,
                    source_ref: Some(derived.source_ref.clone()),
                },
            )?;
            stamp(conn, ctx, Root::Event(event_id))?;
            if prepaid_until.is_some() {
                RecordedShape::Prepaid {
                    event_id: Some(event_id),
                    generated_shares: repository::count_prepaid_shares(conn, event_id)?,
                }
            } else {
                RecordedShape::Single {
                    event_id: Some(event_id),
                }
            }
        }
        EntryShape::Split => {
            let split_group_id = create_taxable_split_group_inner(
                conn,
                CreateTaxableSplitGroupInput {
                    person_id: item.person_id,
                    event_type: event_type.to_owned(),
                    group_note: item.note.clone(),
                    legs: lines
                        .iter()
                        .map(|line| SplitTaxableLegInput {
                            amount_minor: line.gross_minor,
                            event_date: derived.entry_date.clone(),
                            note: item.note.clone(),
                            vat_rate_bps: Some(line.vat_rate_bps),
                            vat_reclaimable_pct_bps: line.vat_reclaimable_pct_bps,
                            expense_deductible_pct_bps: line.deductible_pct_bps,
                            prepaid_until: None,
                            reclaimed_vat: derived.reclaimed_vat,
                        })
                        .collect(),
                    source_ref: Some(derived.source_ref.clone()),
                },
            )?;
            stamp(conn, ctx, Root::SplitGroup(split_group_id))?;
            RecordedShape::SplitGroup {
                split_group_id: Some(split_group_id),
            }
        }
    };

    let mut assets = Vec::new();
    for (part, price) in asset_parts(&item.content).iter().zip(&derived.assets) {
        let account_id = create_account_with_conn(
            conn,
            CreateAccountInput {
                name: part.name.trim().to_owned(),
                currency_id: ctx.asset_currency_id,
                account_type: Some("asset".to_owned()),
                initial_balance_minor: Some(price.purchase_price_minor),
                price_per_unit: None,
                linked_asset_ids: None,
                iban: None,
                person_id: Some(item.person_id),
                purchase_price_minor: Some(price.purchase_price_minor),
                purchase_date: Some(derived.entry_date.clone()),
                depreciation_period_months: Some(part.depreciation_months),
                initial_balance_date: Some(derived.entry_date.clone()),
            },
        )?;
        stamp(
            conn,
            ctx,
            Root::AssetAccount {
                account_id,
                source_ref: derived.source_ref.clone(),
            },
        )?;
        assets.push(RecordedAsset {
            account_id: Some(account_id),
            name: part.name.trim().to_owned(),
            purchase_price_minor: price.purchase_price_minor,
            price_basis: price.price_basis,
            depreciation_months: part.depreciation_months,
        });
    }

    if !item.payment_ids.is_empty() {
        let cashflow_event_ids = item.payment_ids.clone();
        match shape {
            RecordedShape::Single {
                event_id: Some(taxable_event_id),
            }
            | RecordedShape::Prepaid {
                event_id: Some(taxable_event_id),
                ..
            } => link_cashflows_to_taxable(
                conn,
                LinkCashflowsParams {
                    taxable_event_id,
                    cashflow_event_ids,
                },
            )?,
            RecordedShape::SplitGroup {
                split_group_id: Some(split_group_id),
            } => link_cashflows_to_split_group(
                conn,
                LinkCashflowsToSplitGroupParams {
                    split_group_id,
                    cashflow_event_ids,
                },
            )?,
            _ => unreachable!("derive refuses payments on a document with only assets"),
        }
    }

    Ok(Written {
        shape,
        lines: lines
            .iter()
            .map(|line| RecordedLine {
                gross_minor: line.gross_minor,
                net_minor: calculate_net_amount(line.gross_minor, line.vat_rate_bps),
                vat_rate_bps: line.vat_rate_bps,
            })
            .collect(),
        assets,
    })
}

fn expense_line(line: &ExpenseLine) -> Line {
    Line {
        gross_minor: line.gross_minor,
        vat_rate_bps: line.vat_rate_bps.get(),
        deductible_pct_bps: Some(line.deductible_pct_bps.get()),
        vat_reclaimable_pct_bps: Some(line.vat_reclaimable_pct_bps.get()),
    }
}

fn asset_parts(content: &DocumentContent) -> &[AssetPart] {
    match content {
        DocumentContent::Expense(expense) => &expense.assets,
        _ => &[],
    }
}

fn stamp(conn: &Connection, ctx: &ItemContext, root: Root) -> Result<(), AppError> {
    repository::stamp(
        conn,
        StampParams {
            batch_id: ctx.batch_id,
            root,
        },
    )
}

fn load_payments(conn: &Connection, payment_ids: &[i64]) -> Result<Vec<Payment>, AppError> {
    if payment_ids.is_empty() {
        return Ok(Vec::new());
    }
    find_payments(
        conn,
        FindPaymentsParams {
            event_ids: Some(payment_ids.to_vec()),
            limit: payment_ids.len() as u32,
            ..Default::default()
        },
    )
}

/// An expense is paid out and revenue comes in; a credit note (a negative total) turns both
/// around.
fn expected_payment_sign(content: &DocumentContent, total_gross_minor: i64) -> i64 {
    let incoming = matches!(content, DocumentContent::Revenue(_));
    total_gross_minor.signum() * if incoming { 1 } else { -1 }
}

/// Any live entry of the same person and kind with the same total near the entry date, so the
/// same invoice keyed by IČO once and by name once, or typed in by hand, shows up. An entry
/// with this document's reference never gets here: it was refused as already recorded.
fn similar_documents(
    conn: &Connection,
    item: &DocumentItem,
    derived: &Derived,
) -> Result<Vec<Warning>, AppError> {
    let (kind, entries_total) = match &item.content {
        DocumentContent::Revenue(revenue) => (
            TaxableKind::Revenue,
            revenue.lines.iter().map(|l| l.gross_minor).sum(),
        ),
        DocumentContent::Expense(expense) => (
            TaxableKind::Expense,
            expense.lines.iter().map(|l| l.gross_minor).sum(),
        ),
        DocumentContent::PrepaidExpense(prepaid) => {
            (TaxableKind::PrepaidExpense, prepaid.line.gross_minor)
        }
    };
    let Ok(date) = NaiveDate::parse_from_str(&derived.entry_date, "%Y-%m-%d") else {
        return Ok(Vec::new());
    };
    if derived.entries == EntryShape::None {
        return Ok(Vec::new());
    }
    let window = Duration::days(SIMILAR_DOCUMENT_WINDOW_DAYS);
    let found = find_document_entries(
        conn,
        FindDocumentEntriesParams {
            person_id: Some(item.person_id),
            from_date: Some((date - window).format("%Y-%m-%d").to_string()),
            to_date: Some((date + window).format("%Y-%m-%d").to_string()),
            total_gross_minor: Some(entries_total),
            kinds: vec![kind],
            limit: 5,
            ..Default::default()
        },
    )?;
    Ok(found
        .into_iter()
        .filter_map(|doc| {
            Some(Warning::SimilarDocument {
                event_id: doc.lines.first()?.event_id,
                source_ref: doc.source_ref,
                date: doc.date,
            })
        })
        .collect())
}

fn affected_tax_models(
    conn: &Connection,
    person_years: &BTreeSet<(i64, i64)>,
) -> Result<Vec<i64>, AppError> {
    let mut ids: Vec<i64> = list_tax_models(conn)?
        .into_iter()
        .filter(|model| person_years.contains(&(model.person_id, model.calendar_year)))
        .map(|model| model.id)
        .collect();
    ids.sort_unstable();
    Ok(ids)
}

/// Ties a real write to the dry run the user reviewed. `DefaultHasher::new` is the same
/// SipHash with the same keys in every run of one build, which is all a dry run and its write
/// need.
fn review_token(label: &str, items: &[DocumentItem]) -> String {
    let mut hasher = DefaultHasher::new();
    (label, items).hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn already_recorded_message(recorded: &RecordedSource) -> String {
    let (what, batch_id) = match recorded {
        RecordedSource::Entry {
            event_id, batch_id, ..
        } => (format!("entry {event_id}"), batch_id),
        RecordedSource::Asset {
            asset_account_id,
            batch_id,
            ..
        } => (format!("asset account {asset_account_id}"), batch_id),
    };
    match batch_id {
        Some(batch_id) => format!("This document is already in Saldo ({what}, batch {batch_id})"),
        None => format!("This document is already in Saldo ({what})"),
    }
}

fn invalid(message: &str) -> AppError {
    AppError {
        code: "VALIDATION".into(),
        message: message.into(),
    }
}
