use std::collections::BTreeMap;

use crate::error::AppError;
use crate::features::tax_models::calculation::calculate_net_amount;
use crate::features::transactions::models::{DocumentEntry, EntryLine, Payment, TaxableKind};
use rusqlite::types::ToSql;
use rusqlite::{params_from_iter, Connection};

use super::NO_LINK_TO_A_LIVE_ENTRY;

#[derive(Default)]
pub struct FindDocumentEntriesParams {
    pub person_id: Option<i64>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub total_gross_minor: Option<i64>,
    pub text: Option<String>,
    pub source_ref_prefix: Option<String>,
    pub source_ref: Option<String>,
    pub batch_id: Option<i64>,
    pub kinds: Vec<TaxableKind>,
    pub limit: u32,
}

#[derive(Default)]
pub struct FindPaymentsParams {
    pub person_id: Option<i64>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub amount_either_sign_minor: Option<i64>,
    pub original_currency: Option<String>,
    pub note_contains: Option<String>,
    pub unlinked_only: bool,
    pub event_ids: Option<Vec<i64>>,
    pub limit: u32,
}

/// Groups a split group's legs into one document; any other entry is its own document.
const DOC_KEY: &str = "COALESCE(e.split_group_id, -e.id)";

const TAXABLE_ROWS: &str = "
    FROM event e
    JOIN account a ON a.id = e.account_id
    JOIN event_data ed ON ed.id = e.latest_data_id
    LEFT JOIN split_group sg ON sg.id = e.split_group_id
    WHERE e.deleted_at IS NULL
      AND e.is_system_generated = 0
      AND e.event_type IN ('revenue', 'expense', 'prepaid_expense')";

/// Documents the user recorded, newest first; generated rows (prepaid shares, depreciation)
/// never appear. `limit` and every filter apply to whole documents.
pub fn find_document_entries(
    conn: &Connection,
    params: FindDocumentEntriesParams,
) -> Result<Vec<DocumentEntry>, AppError> {
    let doc_keys = find_document_keys(conn, &params)?;
    if doc_keys.is_empty() {
        return Ok(Vec::new());
    }
    let mut documents = load_documents(conn, &doc_keys)?;
    Ok(doc_keys
        .iter()
        .filter_map(|key| documents.remove(key))
        .collect())
}

fn find_document_keys(
    conn: &Connection,
    params: &FindDocumentEntriesParams,
) -> Result<Vec<i64>, AppError> {
    let mut row_filters = String::new();
    let mut doc_filters = String::new();
    let mut values: Vec<Box<dyn ToSql>> = Vec::new();

    if let Some(person_id) = params.person_id {
        row_filters.push_str(" AND a.person_id = ?");
        values.push(Box::new(person_id));
    }
    if !params.kinds.is_empty() {
        row_filters.push_str(&format!(
            " AND e.event_type IN ({})",
            placeholders(params.kinds.len())
        ));
        for kind in &params.kinds {
            values.push(Box::new(kind.event_type()));
        }
    }
    if let Some(from) = &params.from_date {
        doc_filters.push_str(" AND MIN(substr(ed.event_date, 1, 10)) >= ?");
        values.push(Box::new(from.clone()));
    }
    if let Some(to) = &params.to_date {
        doc_filters.push_str(" AND MAX(substr(ed.event_date, 1, 10)) <= ?");
        values.push(Box::new(to.clone()));
    }
    if let Some(total) = params.total_gross_minor {
        doc_filters.push_str(" AND SUM(ed.amount_minor) = ?");
        values.push(Box::new(total));
    }
    if let Some(text) = &params.text {
        doc_filters.push_str(
            " AND MAX(instr(fold(COALESCE(ed.note, '') || ' ' || COALESCE(sg.note, '') || ' '
                                || COALESCE(e.source_ref, '')), fold(?)) > 0)",
        );
        values.push(Box::new(text.clone()));
    }
    if let Some(prefix) = &params.source_ref_prefix {
        doc_filters.push_str(" AND MAX(substr(COALESCE(e.source_ref, ''), 1, length(?)) = ?)");
        values.push(Box::new(prefix.clone()));
        values.push(Box::new(prefix.clone()));
    }
    if let Some(source_ref) = &params.source_ref {
        doc_filters.push_str(" AND MAX(COALESCE(e.source_ref = ?, 0))");
        values.push(Box::new(source_ref.clone()));
    }
    if let Some(batch_id) = params.batch_id {
        doc_filters.push_str(" AND MAX(COALESCE(e.batch_id = ?, 0))");
        values.push(Box::new(batch_id));
    }
    values.push(Box::new(params.limit));

    let sql = format!(
        "SELECT {DOC_KEY} AS doc_key, MAX(substr(ed.event_date, 1, 10)) AS doc_date
         {TAXABLE_ROWS} {row_filters}
         GROUP BY doc_key
         HAVING 1 = 1 {doc_filters}
         ORDER BY doc_date DESC, doc_key DESC
         LIMIT ?"
    );
    let keys = conn
        .prepare(&sql)?
        .query_map(params_from_iter(values.iter()), |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<i64>>>()?;
    Ok(keys)
}

struct Leg {
    doc_key: i64,
    kind: TaxableKind,
    person_id: i64,
    date: String,
    note: Option<String>,
    group_note: Option<String>,
    source_ref: Option<String>,
    batch_id: Option<i64>,
    split_group_id: Option<i64>,
    prepaid_until: Option<String>,
    reclaimed_vat: Option<bool>,
    line: EntryLine,
}

fn load_documents(
    conn: &Connection,
    doc_keys: &[i64],
) -> Result<BTreeMap<i64, DocumentEntry>, AppError> {
    let sql = format!(
        "SELECT {DOC_KEY}, e.id, e.event_type, a.person_id, substr(ed.event_date, 1, 10),
                ed.amount_minor, ed.note, sg.note, e.source_ref, e.batch_id, e.split_group_id,
                ed.prepaid_until, ed.reclaimed_vat, ed.vat_rate_bps,
                ed.expense_deductible_pct_bps, ed.vat_reclaimable_pct_bps
         {TAXABLE_ROWS} AND {DOC_KEY} IN ({})
         ORDER BY e.id",
        placeholders(doc_keys.len())
    );
    let legs = conn
        .prepare(&sql)?
        .query_map(params_from_iter(doc_keys.iter()), |row| {
            let gross_minor: i64 = row.get(5)?;
            let vat_rate_bps: Option<i64> = row.get(13)?;
            Ok(Leg {
                doc_key: row.get(0)?,
                kind: row.get(2)?,
                person_id: row.get(3)?,
                date: row.get(4)?,
                note: row.get(6)?,
                group_note: row.get(7)?,
                source_ref: row.get(8)?,
                batch_id: row.get(9)?,
                split_group_id: row.get(10)?,
                prepaid_until: row.get(11)?,
                reclaimed_vat: row.get(12)?,
                line: EntryLine {
                    event_id: row.get(1)?,
                    gross_minor,
                    net_minor: calculate_net_amount(gross_minor, vat_rate_bps.unwrap_or(0)),
                    vat_rate_bps,
                    deductible_pct_bps: row.get(14)?,
                    vat_reclaimable_pct_bps: row.get(15)?,
                },
            })
        })?
        .collect::<rusqlite::Result<Vec<Leg>>>()?;

    let event_ids: Vec<i64> = legs.iter().map(|leg| leg.line.event_id).collect();
    let links = linked_pairs(conn, LinkLookup::PaymentsOfEntries, &event_ids)?;

    let mut documents: BTreeMap<i64, DocumentEntry> = BTreeMap::new();
    for leg in legs {
        let payments = links.get(&leg.line.event_id).cloned().unwrap_or_default();
        let document = documents
            .entry(leg.doc_key)
            .or_insert_with(|| DocumentEntry {
                kind: leg.kind,
                person_id: leg.person_id,
                date: leg.date.clone(),
                total_gross_minor: 0,
                note: leg.group_note.clone().or(leg.note.clone()),
                source_ref: leg.source_ref.clone(),
                batch_id: leg.batch_id,
                split_group_id: leg.split_group_id,
                prepaid_until: leg.prepaid_until.clone(),
                reclaimed_vat: leg.reclaimed_vat,
                lines: Vec::new(),
                linked_payment_ids: Vec::new(),
            });
        document.total_gross_minor += leg.line.gross_minor;
        document.lines.push(leg.line);
        for payment in payments {
            if !document.linked_payment_ids.contains(&payment) {
                document.linked_payment_ids.push(payment);
            }
        }
    }
    for document in documents.values_mut() {
        document.linked_payment_ids.sort_unstable();
    }
    Ok(documents)
}

/// Cash flows of the person (or of everyone), newest first.
pub fn find_payments(
    conn: &Connection,
    params: FindPaymentsParams,
) -> Result<Vec<Payment>, AppError> {
    let mut filters = String::new();
    let mut values: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(person_id) = params.person_id {
        filters.push_str(" AND a.person_id = ?");
        values.push(Box::new(person_id));
    }
    if let Some(from) = params.from_date {
        filters.push_str(" AND substr(ed.event_date, 1, 10) >= ?");
        values.push(Box::new(from));
    }
    if let Some(to) = params.to_date {
        filters.push_str(" AND substr(ed.event_date, 1, 10) <= ?");
        values.push(Box::new(to));
    }
    if let Some(amount) = params.amount_either_sign_minor {
        filters.push_str(" AND (abs(ed.amount_minor) = ? OR abs(ed.original_amount_minor) = ?)");
        values.push(Box::new(amount.abs()));
        values.push(Box::new(amount.abs()));
    }
    if let Some(currency) = params.original_currency {
        filters.push_str(" AND oc.code = ?");
        values.push(Box::new(currency.to_uppercase()));
    }
    if let Some(text) = params.note_contains {
        filters.push_str(" AND instr(fold(COALESCE(ed.note, '')), fold(?)) > 0");
        values.push(Box::new(text));
    }
    if params.unlinked_only {
        filters.push_str(&format!(" AND {NO_LINK_TO_A_LIVE_ENTRY}"));
    }
    if let Some(event_ids) = &params.event_ids {
        filters.push_str(&format!(" AND e.id IN ({})", placeholders(event_ids.len())));
        for id in event_ids {
            values.push(Box::new(*id));
        }
    }
    values.push(Box::new(params.limit));

    let sql = format!(
        "SELECT e.id, e.account_id, a.name, a.person_id, substr(ed.event_date, 1, 10),
                ed.amount_minor, c.code, ed.original_amount_minor, oc.code, ed.note
         FROM event e
         JOIN account a ON a.id = e.account_id
         JOIN currency c ON c.id = a.currency_id
         JOIN event_data ed ON ed.id = e.latest_data_id
         LEFT JOIN currency oc ON oc.id = ed.original_currency_id
         WHERE e.deleted_at IS NULL AND e.event_type = 'cashflow' {filters}
         ORDER BY ed.event_date DESC, e.id DESC
         LIMIT ?"
    );
    let mut payments = conn
        .prepare(&sql)?
        .query_map(params_from_iter(values.iter()), |row| {
            Ok(Payment {
                event_id: row.get(0)?,
                account_id: row.get(1)?,
                account_name: row.get(2)?,
                person_id: row.get(3)?,
                date: row.get(4)?,
                amount_minor: row.get(5)?,
                currency: row.get(6)?,
                original_amount_minor: row.get(7)?,
                original_currency: row.get(8)?,
                note: row.get(9)?,
                linked_entry_ids: Vec::new(),
            })
        })?
        .collect::<rusqlite::Result<Vec<Payment>>>()?;

    let ids: Vec<i64> = payments.iter().map(|p| p.event_id).collect();
    let links = linked_pairs(conn, LinkLookup::EntriesOfPayments, &ids)?;
    for payment in &mut payments {
        payment.linked_entry_ids = links.get(&payment.event_id).cloned().unwrap_or_default();
    }
    Ok(payments)
}

#[derive(Clone, Copy)]
enum LinkLookup {
    PaymentsOfEntries,
    EntriesOfPayments,
}

impl LinkLookup {
    fn select_where_in(self) -> &'static str {
        match self {
            LinkLookup::PaymentsOfEntries => {
                "SELECT tcl.taxable_event_id, tcl.cashflow_event_id
                 FROM taxable_cashflow_link tcl
                 JOIN event ce ON ce.id = tcl.cashflow_event_id AND ce.deleted_at IS NULL
                 WHERE tcl.taxable_event_id IN"
            }
            LinkLookup::EntriesOfPayments => {
                "SELECT tcl.cashflow_event_id, tcl.taxable_event_id
                 FROM taxable_cashflow_link tcl
                 JOIN event te ON te.id = tcl.taxable_event_id AND te.deleted_at IS NULL
                 WHERE tcl.cashflow_event_id IN"
            }
        }
    }
}

fn linked_pairs(
    conn: &Connection,
    lookup: LinkLookup,
    ids: &[i64],
) -> Result<BTreeMap<i64, Vec<i64>>, AppError> {
    let mut pairs: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
    if ids.is_empty() {
        return Ok(pairs);
    }
    let sql = format!(
        "{} ({}) ORDER BY 1, 2",
        lookup.select_where_in(),
        placeholders(ids.len())
    );
    let rows = conn
        .prepare(&sql)?
        .query_map(params_from_iter(ids.iter()), |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (key, value) in rows {
        pairs.entry(key).or_default().push(value);
    }
    Ok(pairs)
}

fn placeholders(count: usize) -> String {
    vec!["?"; count].join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::{create_account, CreateAccountParams};
    use crate::features::persons::repository::{
        create_person, get_default_expense_account_id, CreatePersonParams,
    };
    use crate::features::transactions::repository::{
        create_cashflow, create_taxable_event, create_taxable_split_group_with_legs, delete_event,
        link_cashflows_to_taxable, recalculate_depreciation_for_asset, CashflowEntry,
        CreateTaxableEventParams, CreateTaxableSplitGroupWithLegsParams, LinkCashflowsParams,
        TaxableEventLeg,
    };
    use crate::features::transactions::SourceRef;

    struct Books {
        person_id: i64,
        expense_account_id: i64,
        bank_account_id: i64,
    }

    fn books(conn: &Connection) -> Books {
        let person_id = create_person(
            conn,
            CreatePersonParams {
                name: "Firma s.r.o.".to_owned(),
                person_type: "legal".to_owned(),
                vat_payer: true,
            },
        )
        .unwrap();
        let bank_account_id = create_account(
            conn,
            CreateAccountParams {
                name: "Tatra business".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                person_id: Some(person_id),
                ..Default::default()
            },
        )
        .unwrap();
        Books {
            person_id,
            expense_account_id: get_default_expense_account_id(conn, person_id).unwrap(),
            bank_account_id,
        }
    }

    fn invoice(issuer: &str, number: &str) -> Option<SourceRef> {
        Some(SourceRef::invoice(issuer, number).unwrap())
    }

    fn expense(
        conn: &Connection,
        books: &Books,
        date: &str,
        gross: i64,
        source_ref: Option<SourceRef>,
    ) -> i64 {
        create_taxable_event(
            conn,
            CreateTaxableEventParams {
                account_id: books.expense_account_id,
                event_type: "expense".to_owned(),
                amount_minor: gross,
                event_date: date.to_owned(),
                note: Some("Orange paušál".to_owned()),
                vat_rate_bps: Some(2300),
                expense_deductible_pct_bps: Some(8000),
                vat_reclaimable_pct_bps: Some(8000),
                source_ref,
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn leg(gross: i64, vat_rate_bps: i64) -> TaxableEventLeg {
        TaxableEventLeg {
            amount_minor: gross,
            event_date: "2026-10-04".to_owned(),
            note: None,
            vat_rate_bps: Some(vat_rate_bps),
            vat_reclaimable_pct_bps: Some(10000),
            expense_deductible_pct_bps: Some(10000),
            prepaid_until: None,
            reclaimed_vat: Some(true),
        }
    }

    fn split_expense(conn: &Connection, books: &Books) -> i64 {
        create_taxable_split_group_with_legs(
            conn,
            CreateTaxableSplitGroupWithLegsParams {
                account_id: books.expense_account_id,
                event_type: "expense".to_owned(),
                group_note: Some("Metro".to_owned()),
                legs: vec![leg(12_300, 2300), leg(2_140, 700)],
                source_ref: invoice("31347037", "2026/10/551"),
            },
        )
        .unwrap()
    }

    fn payment(conn: &Connection, books: &Books, amount: i64, note: &str) -> i64 {
        create_cashflow(
            conn,
            &CashflowEntry {
                account_id: books.bank_account_id,
                amount_minor: amount,
                event_date: "2026-10-02".to_owned(),
                note: Some(note.to_owned()),
                counterpart_account_id: None,
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap()
    }

    fn find(conn: &Connection, params: FindDocumentEntriesParams) -> Vec<DocumentEntry> {
        find_document_entries(
            conn,
            FindDocumentEntriesParams {
                limit: params.limit.max(1),
                ..params
            },
        )
        .unwrap()
    }

    fn all(person_id: i64) -> FindDocumentEntriesParams {
        FindDocumentEntriesParams {
            person_id: Some(person_id),
            limit: 50,
            ..Default::default()
        }
    }

    #[test]
    fn a_split_group_is_one_document_with_its_lines() {
        let conn = initialize_in_memory().unwrap();
        let books = books(&conn);
        let group_id = split_expense(&conn, &books);

        let found = find(&conn, all(books.person_id));

        assert_eq!(found.len(), 1);
        let doc = &found[0];
        assert_eq!(doc.split_group_id, Some(group_id));
        assert_eq!(doc.kind, TaxableKind::Expense);
        assert_eq!(doc.total_gross_minor, 14_440);
        assert_eq!(doc.note.as_deref(), Some("Metro"));
        assert_eq!(
            doc.source_ref.as_deref(),
            Some("invoice:31347037/2026/10/551")
        );
        let nets: Vec<i64> = doc.lines.iter().map(|line| line.net_minor).collect();
        assert_eq!(nets, [10_000, 2_000]);
    }

    #[test]
    fn the_limit_counts_documents_and_newest_comes_first() {
        let conn = initialize_in_memory().unwrap();
        let books = books(&conn);
        split_expense(&conn, &books);
        let older = expense(&conn, &books, "2026-09-30", 2_399, None);

        let page = find(
            &conn,
            FindDocumentEntriesParams {
                limit: 1,
                ..all(books.person_id)
            },
        );
        let everything = find(&conn, all(books.person_id));

        assert_eq!(page.len(), 1);
        assert_eq!(page[0].lines.len(), 2, "a split group is never cut");
        assert_eq!(everything.len(), 2);
        assert_eq!(everything[1].lines[0].event_id, older);
    }

    #[test]
    fn filters_by_issuer_reference_total_and_text() {
        let conn = initialize_in_memory().unwrap();
        let books = books(&conn);
        split_expense(&conn, &books);
        let orange = expense(
            &conn,
            &books,
            "2026-09-30",
            2_399,
            invoice("35697270", "FV1"),
        );
        expense(
            &conn,
            &books,
            "2026-08-31",
            2_399,
            invoice("35697270", "FV0"),
        );

        let by_issuer = find(
            &conn,
            FindDocumentEntriesParams {
                source_ref_prefix: Some(
                    crate::features::transactions::source_ref::invoice_issuer_prefix("35697270")
                        .unwrap(),
                ),
                ..all(books.person_id)
            },
        );
        let by_reference = find(
            &conn,
            FindDocumentEntriesParams {
                source_ref: Some("invoice:35697270/FV1".to_owned()),
                ..all(books.person_id)
            },
        );
        let by_total = find(
            &conn,
            FindDocumentEntriesParams {
                total_gross_minor: Some(14_440),
                ..all(books.person_id)
            },
        );
        let by_text = find(
            &conn,
            FindDocumentEntriesParams {
                text: Some("metro".to_owned()),
                ..all(books.person_id)
            },
        );

        assert_eq!(by_issuer.len(), 2);
        assert_eq!(by_reference.len(), 1);
        assert_eq!(by_reference[0].lines[0].event_id, orange);
        assert_eq!(by_total.len(), 1);
        assert!(by_total[0].split_group_id.is_some());
        assert_eq!(by_text.len(), 1);
        assert!(by_text[0].split_group_id.is_some());
    }

    #[test]
    fn text_searches_ignore_case_and_accents() {
        let conn = initialize_in_memory().unwrap();
        let books = books(&conn);
        let orange = expense(&conn, &books, "2026-09-30", 2_399, None);
        let skoda = payment(&conn, &books, -30_000, "ŠKODA FINANCIAL SERVICES");
        let cez = payment(&conn, &books, -8_000, "CEZ SLOVENSKO");
        payment(&conn, &books, -500, "ORANGE");

        let entries = find(
            &conn,
            FindDocumentEntriesParams {
                text: Some("PAUSAL".to_owned()),
                ..all(books.person_id)
            },
        );
        let note = |text: &str| {
            payments(
                &conn,
                FindPaymentsParams {
                    note_contains: Some(text.to_owned()),
                    ..Default::default()
                },
            )
        };

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].lines[0].event_id, orange);
        assert_eq!(note("skoda"), [skoda]);
        assert_eq!(note("čez"), [cez]);
    }

    #[test]
    fn deleted_entries_and_generated_rows_never_appear() {
        let conn = initialize_in_memory().unwrap();
        let books = books(&conn);
        let deleted = expense(&conn, &books, "2026-09-30", 2_399, None);
        delete_event(&conn, deleted).unwrap();
        let asset = create_account(
            &conn,
            CreateAccountParams {
                name: "MacBook".to_owned(),
                currency_id: 1,
                account_type: "asset".to_owned(),
                person_id: Some(books.person_id),
                purchase_price_minor: Some(292_601),
                purchase_date: Some("2026-01-12".to_owned()),
                depreciation_period_months: Some(48),
                ..Default::default()
            },
        )
        .unwrap();
        recalculate_depreciation_for_asset(&conn, asset).unwrap();

        assert!(find(&conn, all(books.person_id)).is_empty());
    }

    #[test]
    fn a_document_lists_the_payments_linked_to_it() {
        let conn = initialize_in_memory().unwrap();
        let books = books(&conn);
        let entry = expense(&conn, &books, "2026-09-30", 2_399, None);
        let paid = payment(&conn, &books, -2_399, "ORANGE SLOVENSKO");
        link_cashflows_to_taxable(
            &conn,
            LinkCashflowsParams {
                taxable_event_id: entry,
                cashflow_event_ids: vec![paid],
            },
        )
        .unwrap();

        let found = find(&conn, all(books.person_id));

        assert_eq!(found[0].linked_payment_ids, [paid]);
    }

    fn payments(conn: &Connection, params: FindPaymentsParams) -> Vec<i64> {
        find_payments(
            conn,
            FindPaymentsParams {
                limit: 50,
                ..params
            },
        )
        .unwrap()
        .into_iter()
        .map(|p| p.event_id)
        .collect()
    }

    #[test]
    fn a_payment_matches_its_amount_of_either_sign_or_its_original_amount() {
        let conn = initialize_in_memory().unwrap();
        let books = books(&conn);
        let card = payment(&conn, &books, -2_399, "ORANGE");
        let usd_id: i64 = conn
            .query_row("SELECT id FROM currency WHERE code = 'USD'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let foreign = create_cashflow(
            &conn,
            &CashflowEntry {
                account_id: books.bank_account_id,
                amount_minor: -4_512,
                event_date: "2026-10-02".to_owned(),
                note: Some("CARD OPENAI *CHATGPT".to_owned()),
                counterpart_account_id: None,
                bucket_id: None,
                original_currency_id: Some(usd_id),
                original_amount_minor: Some(-4_900),
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        let by_amount = payments(
            &conn,
            FindPaymentsParams {
                amount_either_sign_minor: Some(2_399),
                ..Default::default()
            },
        );
        let by_original = payments(
            &conn,
            FindPaymentsParams {
                amount_either_sign_minor: Some(4_900),
                original_currency: Some("usd".to_owned()),
                ..Default::default()
            },
        );
        let by_note = payments(
            &conn,
            FindPaymentsParams {
                note_contains: Some("openai".to_owned()),
                ..Default::default()
            },
        );

        assert_eq!(by_amount, [card]);
        assert_eq!(by_original, [foreign]);
        assert_eq!(by_note, [foreign]);
        let found = find_payments(
            &conn,
            FindPaymentsParams {
                amount_either_sign_minor: Some(4_900),
                limit: 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(found[0].original_currency.as_deref(), Some("USD"));
        assert_eq!(found[0].amount_minor, -4_512);
    }

    #[test]
    fn unlinked_only_ignores_links_to_deleted_entries() {
        let conn = initialize_in_memory().unwrap();
        let books = books(&conn);
        let linked = payment(&conn, &books, -2_399, "ORANGE");
        let freed = payment(&conn, &books, -1_000, "OTHER");
        let free = payment(&conn, &books, -500, "FREE");
        for (cashflow, gross) in [(linked, 2_399), (freed, 1_000)] {
            let entry = expense(&conn, &books, "2026-09-30", gross, None);
            link_cashflows_to_taxable(
                &conn,
                LinkCashflowsParams {
                    taxable_event_id: entry,
                    cashflow_event_ids: vec![cashflow],
                },
            )
            .unwrap();
            if cashflow == freed {
                delete_event(&conn, entry).unwrap();
            }
        }

        let mut unlinked = payments(
            &conn,
            FindPaymentsParams {
                person_id: Some(books.person_id),
                unlinked_only: true,
                ..Default::default()
            },
        );
        unlinked.sort_unstable();

        assert_eq!(unlinked, [freed, free]);
    }
}
