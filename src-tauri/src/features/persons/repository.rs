use crate::error::AppError;
use crate::features::currency::repository::get_consolidation_currency;
use crate::features::persons::models::PersonRow;
use crate::shared::{local_now, with_savepoint_app};
use rusqlite::{params, Connection, OptionalExtension};

pub struct CreatePersonParams {
    pub name: String,
    pub person_type: String,
    pub vat_payer: bool,
}

pub struct UpdatePersonParams {
    pub person_id: i64,
    pub name: String,
    pub person_type: String,
    pub vat_payer: bool,
}

/// Creates the two hidden default taxable accounts for a person and returns (revenue_id, expense_id).
pub fn create_default_taxable_accounts(
    conn: &Connection,
    person_id: i64,
) -> Result<(i64, i64), AppError> {
    let currency_id = get_consolidation_currency(conn)?.id;

    conn.execute(
        "INSERT INTO account (name, currency_id, account_type, sort_order, person_id)
         VALUES ('__default_revenue', ?1, 'default_revenue', 0, ?2)",
        params![currency_id, person_id],
    )
    .map_err(AppError::from)?;
    let revenue_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO account (name, currency_id, account_type, sort_order, person_id)
         VALUES ('__default_expense', ?1, 'default_expense', 0, ?2)",
        params![currency_id, person_id],
    )
    .map_err(AppError::from)?;
    let expense_id = conn.last_insert_rowid();

    Ok((revenue_id, expense_id))
}

pub fn create_person(conn: &Connection, params: CreatePersonParams) -> Result<i64, AppError> {
    let now = local_now();
    with_savepoint_app(conn, || {
        conn.execute(
            "INSERT INTO person (name, person_type, is_default, created_at, vat_payer) VALUES (?1, ?2, 0, ?3, ?4)",
            params![params.name.as_str(), params.person_type.as_str(), now, params.vat_payer as i64],
        )
        .map_err(AppError::from)?;
        let person_id = conn.last_insert_rowid();

        let (revenue_id, expense_id) = create_default_taxable_accounts(conn, person_id)?;

        conn.execute(
            "UPDATE person SET default_revenue_account_id = ?1, default_expense_account_id = ?2 WHERE id = ?3",
            params![revenue_id, expense_id, person_id],
        )
        .map_err(AppError::from)?;

        Ok(person_id)
    })
}

pub fn list_persons(conn: &Connection) -> rusqlite::Result<Vec<PersonRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, person_type, is_default, created_at,
                default_revenue_account_id, default_expense_account_id, vat_payer
         FROM person
         ORDER BY is_default DESC, name ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let is_default_int: i64 = row.get(3)?;
        let vat_payer_int: i64 = row.get(7)?;
        Ok(PersonRow {
            id: row.get(0)?,
            name: row.get(1)?,
            person_type: row.get(2)?,
            is_default: is_default_int != 0,
            created_at: row.get(4)?,
            default_revenue_account_id: row.get::<_, i64>(5)?,
            default_expense_account_id: row.get::<_, i64>(6)?,
            vat_payer: vat_payer_int != 0,
        })
    })?;
    rows.collect()
}

/// Resolves the correct hidden default account ID for a person and event type.
/// Returns NOT_FOUND if the person does not exist, DOMAIN_ERROR if the account FK is NULL.
pub fn resolve_default_taxable_account(
    conn: &Connection,
    person_id: i64,
    event_type: &str,
) -> Result<i64, AppError> {
    let row: Option<(Option<i64>, Option<i64>)> = conn
        .query_row(
            "SELECT default_revenue_account_id, default_expense_account_id FROM person WHERE id = ?1",
            params![person_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(AppError::from)?;

    let (rev_id, exp_id) = row.ok_or_else(|| AppError {
        code: "NOT_FOUND".into(),
        message: "Person not found".into(),
    })?;

    match event_type {
        "revenue" => rev_id.ok_or_else(|| AppError {
            code: "DOMAIN_ERROR".into(),
            message: "Person has no default revenue account".into(),
        }),
        "expense" | "prepaid_expense" => exp_id.ok_or_else(|| AppError {
            code: "DOMAIN_ERROR".into(),
            message: "Person has no default expense account".into(),
        }),
        _ => Err(AppError {
            code: "VALIDATION".into(),
            message: "event_type must be 'revenue' or 'expense'".into(),
        }),
    }
}

pub fn update_person(conn: &Connection, params: UpdatePersonParams) -> Result<(), AppError> {
    let affected = conn
        .execute(
            "UPDATE person SET name = ?1, person_type = ?2, vat_payer = ?3 WHERE id = ?4",
            params![
                params.name.as_str(),
                params.person_type.as_str(),
                params.vat_payer as i64,
                params.person_id
            ],
        )
        .map_err(AppError::from)?;
    if affected == 0 {
        return Err(AppError {
            code: "NOT_FOUND".into(),
            message: "Person not found.".into(),
        });
    }
    Ok(())
}

/// Returns the default expense account ID for the given person.
/// Returns NOT_FOUND if the person does not exist, DOMAIN_ERROR if the FK is NULL.
pub fn get_default_expense_account_id(conn: &Connection, person_id: i64) -> Result<i64, AppError> {
    resolve_default_taxable_account(conn, person_id, "expense")
}

/// Returns the vat_payer flag for the given person.
/// Returns NOT_FOUND if the person does not exist.
pub fn get_person_vat_payer(conn: &Connection, person_id: i64) -> Result<bool, AppError> {
    let row: Option<i64> = conn
        .query_row(
            "SELECT vat_payer FROM person WHERE id = ?1",
            params![person_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(AppError::from)?;

    row.map(|v| v != 0).ok_or_else(|| AppError {
        code: "NOT_FOUND".into(),
        message: "Person not found".into(),
    })
}

pub fn get_person_accounts(conn: &Connection, person_id: i64) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT id FROM account WHERE person_id = ?1")?;
    let rows = stmt.query_map(params![person_id], |row| row.get(0))?;
    rows.collect()
}

/// Delete a non-default person together with their tax models, accounts and events.
pub fn delete_person(conn: &Connection, person_id: i64) -> Result<(), AppError> {
    with_savepoint_app(conn, || {
        ensure_person_deletable(conn, person_id)?;

        // The hidden default accounts are deleted below with the rest, so the person
        // must stop pointing at them first.
        conn.execute(
            "UPDATE person SET default_revenue_account_id = NULL, default_expense_account_id = NULL
             WHERE id = ?1",
            params![person_id],
        )
        .map_err(AppError::from)?;

        crate::features::tax_models::repository::delete_tax_models_for_person(conn, person_id)?;

        let account_ids = get_person_accounts(conn, person_id).map_err(AppError::from)?;
        for account_id in account_ids {
            crate::features::accounts::repository::delete_account(conn, account_id)?;
        }

        conn.execute("DELETE FROM person WHERE id = ?1", params![person_id])
            .map_err(AppError::from)?;
        Ok(())
    })
}

fn ensure_person_deletable(conn: &Connection, person_id: i64) -> Result<(), AppError> {
    let is_default: i64 = conn
        .query_row(
            "SELECT is_default FROM person WHERE id = ?1",
            params![person_id],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError {
                code: "NOT_FOUND".into(),
                message: "Person not found.".into(),
            },
            other => AppError::from(other),
        })?;

    if is_default != 0 {
        return Err(AppError {
            code: "DOMAIN_ERROR".into(),
            message: "Cannot delete the default person.".into(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::{create_account, CreateAccountParams};
    use crate::features::tax_models::repository::{
        create_tax_model, BracketInput, CreateTaxModelParams,
    };
    use crate::features::transactions::repository::depreciation::generate_depreciation_events;
    use crate::features::transactions::repository::taxable_links::{
        link_cashflows_to_taxable, LinkCashflowsParams,
    };
    use crate::features::transactions::repository::{
        create_cashflow, create_taxable_event, create_transfer, CashflowEntry,
        CreateTaxableEventParams,
    };

    fn get_default_person_id(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM person WHERE is_default = 1", [], |row| {
            row.get(0)
        })
        .expect("default person not found")
    }

    #[test]
    fn create_and_list_returns_new_person() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id = create_person(
            &conn,
            CreatePersonParams {
                name: "Alice Corp".to_owned(),
                person_type: "legal".to_owned(),
                vat_payer: false,
            },
        )
        .expect("create_person failed");

        let persons = list_persons(&conn).expect("list_persons failed");
        let created = persons
            .iter()
            .find(|p| p.id == id)
            .expect("new person not in list");
        assert_eq!(created.name, "Alice Corp");
        assert_eq!(created.person_type, "legal");
        assert!(!created.is_default);
    }

    #[test]
    fn create_person_auto_creates_default_taxable_accounts() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id = create_person(
            &conn,
            CreatePersonParams {
                name: "Tax Tester Ltd".to_owned(),
                person_type: "legal".to_owned(),
                vat_payer: false,
            },
        )
        .expect("create_person failed");

        let persons = list_persons(&conn).expect("list_persons failed");
        let person = persons
            .iter()
            .find(|p| p.id == id)
            .expect("person not found");

        assert!(
            person.default_revenue_account_id > 0,
            "revenue account ID should be set"
        );
        assert!(
            person.default_expense_account_id > 0,
            "expense account ID should be set"
        );
        assert_ne!(
            person.default_revenue_account_id, person.default_expense_account_id,
            "revenue and expense accounts should be distinct"
        );

        let revenue_type: String = conn
            .query_row(
                "SELECT account_type FROM account WHERE id = ?1",
                [person.default_revenue_account_id],
                |row| row.get(0),
            )
            .expect("revenue account not found");
        assert_eq!(revenue_type, "default_revenue");

        let expense_type: String = conn
            .query_row(
                "SELECT account_type FROM account WHERE id = ?1",
                [person.default_expense_account_id],
                |row| row.get(0),
            )
            .expect("expense account not found");
        assert_eq!(expense_type, "default_expense");
    }

    #[test]
    fn create_person_default_accounts_use_the_consolidation_currency() {
        let conn = initialize_in_memory().expect("DB init failed");
        let usd: i64 = conn
            .query_row("SELECT id FROM currency WHERE code = 'USD'", [], |r| {
                r.get(0)
            })
            .unwrap();
        crate::features::currency::repository::set_consolidation_currency(&conn, usd).unwrap();

        let id = create_person(
            &conn,
            CreatePersonParams {
                name: "Dollar Ltd".to_owned(),
                person_type: "legal".to_owned(),
                vat_payer: false,
            },
        )
        .expect("create_person failed");

        let currencies: Vec<i64> = conn
            .prepare(
                "SELECT currency_id FROM account
                 WHERE person_id = ?1 AND account_type IN ('default_revenue', 'default_expense')",
            )
            .unwrap()
            .query_map([id], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(currencies, vec![usd, usd]);
    }

    #[test]
    fn default_person_has_default_accounts_after_migration() {
        let conn = initialize_in_memory().expect("DB init failed");

        let persons = list_persons(&conn).expect("list_persons failed");
        let default_person = persons
            .iter()
            .find(|p| p.is_default)
            .expect("default person not found");

        assert!(
            default_person.default_revenue_account_id > 0,
            "default person should have revenue account after migration 024 backfill"
        );
        assert!(
            default_person.default_expense_account_id > 0,
            "default person should have expense account after migration 024 backfill"
        );
    }

    fn mk_account(conn: &Connection, params: CreateAccountParams) -> i64 {
        create_account(conn, params).expect("create account failed")
    }

    fn count(conn: &Connection, sql: &str, id: i64) -> i64 {
        conn.query_row(sql, params![id], |row| row.get(0))
            .expect("count query failed")
    }

    #[test]
    fn delete_person_removes_all_their_data() {
        let conn = initialize_in_memory().expect("DB init failed");
        let default_id = get_default_person_id(&conn);
        let person_id = create_person(
            &conn,
            CreatePersonParams {
                name: "Leaving Ltd".to_owned(),
                person_type: "legal".to_owned(),
                vat_payer: false,
            },
        )
        .expect("create_person failed");

        let other_account_id = mk_account(
            &conn,
            CreateAccountParams {
                name: "Other Checking".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                person_id: Some(default_id),
                ..Default::default()
            },
        );
        let checking_id = mk_account(
            &conn,
            CreateAccountParams {
                name: "Leaving Checking".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                person_id: Some(person_id),
                ..Default::default()
            },
        );
        let asset_id = mk_account(
            &conn,
            CreateAccountParams {
                name: "Leaving Laptop".to_owned(),
                currency_id: 1,
                account_type: "asset".to_owned(),
                person_id: Some(person_id),
                purchase_price_minor: Some(120_000),
                purchase_date: Some("2024-01-01".to_owned()),
                depreciation_period_months: Some(12),
                ..Default::default()
            },
        );
        generate_depreciation_events(&conn).expect("depreciation failed");

        let (_, other_leg_id) = create_transfer(
            &conn,
            &CashflowEntry {
                account_id: checking_id,
                amount_minor: -2_000,
                event_date: "2026-02-01T12:00:00".to_owned(),
                note: None,
                counterpart_account_id: Some(other_account_id),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .expect("create_transfer failed");

        let cashflow_id = create_cashflow(
            &conn,
            &CashflowEntry {
                account_id: checking_id,
                amount_minor: 5_000,
                event_date: "2026-03-01T12:00:00".to_owned(),
                note: None,
                counterpart_account_id: None,
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .expect("create_cashflow failed");
        let revenue_account_id =
            resolve_default_taxable_account(&conn, person_id, "revenue").unwrap();
        let revenue_id = create_taxable_event(
            &conn,
            CreateTaxableEventParams {
                account_id: revenue_account_id,
                event_type: "revenue".to_owned(),
                amount_minor: 5_000,
                event_date: "2026-03-01T00:00:00".to_owned(),
                ..Default::default()
            },
        )
        .expect("create_taxable_event failed");
        link_cashflows_to_taxable(
            &conn,
            LinkCashflowsParams {
                taxable_event_id: revenue_id,
                cashflow_event_ids: vec![cashflow_id],
            },
        )
        .expect("link failed");

        let tax_model_id = create_tax_model(
            &conn,
            CreateTaxModelParams {
                name: "Leaving 2026".to_owned(),
                calendar_year: 2026,
                person_id,
                vat_status: "none".to_owned(),
                vat_from_date: None,
                reserve_fund_current_minor: None,
                reserve_fund_pct_bps: None,
                reserve_fund_max_minor: None,
                dividend_tax_rate_bps: None,
                brackets: vec![BracketInput {
                    sort_order: 0,
                    lower_bound_minor: 0,
                    rate_type: "flat".to_owned(),
                    flat_rate_bps: Some(1500),
                    tiers_json: None,
                }],
            },
        )
        .expect("create_tax_model failed");

        let account_ids = get_person_accounts(&conn, person_id).unwrap();
        assert_eq!(account_ids.len(), 4);

        delete_person(&conn, person_id).expect("delete_person failed");

        assert_eq!(
            count(
                &conn,
                "SELECT COUNT(*) FROM person WHERE id = ?1",
                person_id
            ),
            0
        );
        assert_eq!(
            count(
                &conn,
                "SELECT COUNT(*) FROM account WHERE person_id = ?1",
                person_id
            ),
            0
        );
        for account_id in account_ids {
            assert_eq!(
                count(
                    &conn,
                    "SELECT COUNT(*) FROM event WHERE account_id = ?1",
                    account_id
                ),
                0
            );
        }
        assert_eq!(
            count(
                &conn,
                "SELECT COUNT(*) FROM event WHERE linked_asset_id = ?1",
                asset_id
            ),
            0
        );
        assert_eq!(
            count(
                &conn,
                "SELECT COUNT(*) FROM tax_model WHERE person_id = ?1",
                person_id
            ),
            0
        );
        assert_eq!(
            count(
                &conn,
                "SELECT COUNT(*) FROM tax_model_bracket WHERE tax_model_id = ?1",
                tax_model_id
            ),
            0
        );
        let (event_type, linked_event_id): (String, Option<i64>) = conn
            .query_row(
                "SELECT event_type, linked_event_id FROM event WHERE id = ?1",
                params![other_leg_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(event_type, "cashflow");
        assert_eq!(linked_event_id, None);
    }

    #[test]
    fn default_person_cannot_be_deleted() {
        let conn = initialize_in_memory().expect("DB init failed");
        let default_id = get_default_person_id(&conn);
        let result = delete_person(&conn, default_id);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "DOMAIN_ERROR");
    }

    #[test]
    fn update_person_changes_name() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id = create_person(
            &conn,
            CreatePersonParams {
                name: "Old Name".to_owned(),
                person_type: "physical".to_owned(),
                vat_payer: false,
            },
        )
        .expect("create_person failed");

        update_person(
            &conn,
            UpdatePersonParams {
                person_id: id,
                name: "New Name".to_owned(),
                person_type: "physical".to_owned(),
                vat_payer: false,
            },
        )
        .expect("update_person failed");

        let persons = list_persons(&conn).expect("list_persons failed");
        let updated = persons
            .iter()
            .find(|p| p.id == id)
            .expect("person not found");
        assert_eq!(updated.name, "New Name");
    }

    #[test]
    fn resolve_default_taxable_account_returns_not_found_for_missing_person() {
        let conn = initialize_in_memory().expect("DB init failed");
        let result = resolve_default_taxable_account(&conn, 999_999, "revenue");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "NOT_FOUND");
    }

    #[test]
    fn resolve_default_taxable_account_returns_correct_ids() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id = create_person(
            &conn,
            CreatePersonParams {
                name: "Resolver Test".to_owned(),
                person_type: "physical".to_owned(),
                vat_payer: false,
            },
        )
        .expect("create_person failed");

        let persons = list_persons(&conn).expect("list_persons failed");
        let person = persons
            .iter()
            .find(|p| p.id == id)
            .expect("person not found");

        let rev =
            resolve_default_taxable_account(&conn, id, "revenue").expect("revenue resolve failed");
        assert_eq!(rev, person.default_revenue_account_id);

        let exp =
            resolve_default_taxable_account(&conn, id, "expense").expect("expense resolve failed");
        assert_eq!(exp, person.default_expense_account_id);
    }
}
