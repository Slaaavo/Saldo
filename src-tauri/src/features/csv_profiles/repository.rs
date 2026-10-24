use crate::error::AppError;
use crate::features::csv_profiles::models::{ImportProfileRow, ImportProfileRuleRow, RuleInput};
use crate::shared::{local_now, with_savepoint_app};
use rusqlite::{params, Connection, OptionalExtension};

pub struct CreateImportProfileParams {
    pub name: String,
    pub column_mapping_json: String,
    pub rules: Vec<RuleInput>,
}

pub struct UpdateImportProfileParams {
    pub profile_id: i64,
    pub name: String,
    pub column_mapping_json: String,
    pub rules: Vec<RuleInput>,
}

#[derive(Default)]
pub struct SetPreferredProfileParams {
    pub account_id: i64,
    pub profile_id: Option<i64>,
}

fn load_rules_for_profile(
    conn: &Connection,
    profile_id: i64,
) -> Result<Vec<ImportProfileRuleRow>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, profile_id, rule_type, sort_order, params_json
         FROM import_profile_rule
         WHERE profile_id = ?1
         ORDER BY sort_order ASC",
    )?;
    let rows = stmt.query_map(params![profile_id], |row| {
        Ok(ImportProfileRuleRow {
            id: row.get(0)?,
            profile_id: row.get(1)?,
            rule_type: row.get(2)?,
            sort_order: row.get(3)?,
            params_json: row.get(4)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)
}

pub fn list_import_profiles(conn: &Connection) -> Result<Vec<ImportProfileRow>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, column_mapping_json, created_at, updated_at
         FROM import_profile
         ORDER BY name ASC",
    )?;
    let profile_list: Vec<ImportProfileRow> = stmt
        .query_map([], |row| {
            Ok(ImportProfileRow {
                id: row.get(0)?,
                name: row.get(1)?,
                column_mapping_json: row.get(2)?,
                rules: Vec::new(),
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)?;

    let mut result = Vec::with_capacity(profile_list.len());
    for mut profile in profile_list {
        profile.rules = load_rules_for_profile(conn, profile.id)?;
        result.push(profile);
    }
    Ok(result)
}

pub fn create_import_profile(
    conn: &Connection,
    params: CreateImportProfileParams,
) -> Result<i64, AppError> {
    with_savepoint_app(conn, || {
        let now = local_now();
        conn.execute(
            "INSERT INTO import_profile (name, column_mapping_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![params.name.as_str(), params.column_mapping_json.as_str(), now, now],
        )
        .map_err(AppError::from)?;
        let profile_id = conn.last_insert_rowid();
        for rule in &params.rules {
            conn.execute(
                "INSERT INTO import_profile_rule (profile_id, rule_type, sort_order, params_json) VALUES (?1, ?2, ?3, ?4)",
                params![profile_id, rule.rule_type, rule.sort_order, rule.params_json],
            )
            .map_err(AppError::from)?;
        }
        Ok(profile_id)
    })
}

pub fn update_import_profile(
    conn: &Connection,
    params: UpdateImportProfileParams,
) -> Result<(), AppError> {
    with_savepoint_app(conn, || {
        let now = local_now();
        let affected = conn
            .execute(
                "UPDATE import_profile SET name = ?1, column_mapping_json = ?2, updated_at = ?3 WHERE id = ?4",
                params![params.name.as_str(), params.column_mapping_json.as_str(), now, params.profile_id],
            )
            .map_err(AppError::from)?;
        if affected == 0 {
            return Err(AppError {
                code: "NOT_FOUND".into(),
                message: "Import profile not found.".into(),
            });
        }
        conn.execute(
            "DELETE FROM import_profile_rule WHERE profile_id = ?1",
            params![params.profile_id],
        )
        .map_err(AppError::from)?;
        for rule in &params.rules {
            conn.execute(
                "INSERT INTO import_profile_rule (profile_id, rule_type, sort_order, params_json) VALUES (?1, ?2, ?3, ?4)",
                params![params.profile_id, rule.rule_type, rule.sort_order, rule.params_json],
            )
            .map_err(AppError::from)?;
        }
        Ok(())
    })
}

pub fn delete_import_profile(conn: &Connection, profile_id: i64) -> Result<(), AppError> {
    with_savepoint_app(conn, || {
        let affected = conn
            .execute(
                "DELETE FROM import_profile WHERE id = ?1",
                params![profile_id],
            )
            .map_err(AppError::from)?;
        if affected == 0 {
            return Err(AppError {
                code: "NOT_FOUND".into(),
                message: "Import profile not found.".into(),
            });
        }
        Ok(())
    })
}

pub fn get_preferred_profile(
    conn: &Connection,
    account_id: i64,
) -> Result<Option<ImportProfileRow>, AppError> {
    let preferred_profile_id: Option<i64> = conn
        .query_row(
            "SELECT preferred_profile_id FROM account WHERE id = ?1",
            params![account_id],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()
        .map_err(AppError::from)?
        .flatten();

    let profile_id = match preferred_profile_id {
        Some(id) => id,
        None => return Ok(None),
    };

    let profile = conn
        .query_row(
            "SELECT id, name, column_mapping_json, created_at, updated_at
             FROM import_profile
             WHERE id = ?1",
            params![profile_id],
            |row| {
                Ok(ImportProfileRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    column_mapping_json: row.get(2)?,
                    rules: Vec::new(),
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(AppError::from)?;

    match profile {
        Some(mut p) => {
            p.rules = load_rules_for_profile(conn, p.id)?;
            Ok(Some(p))
        }
        None => Ok(None),
    }
}

pub fn set_preferred_profile(
    conn: &Connection,
    params: SetPreferredProfileParams,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE account SET preferred_profile_id = ?1 WHERE id = ?2",
        params![params.profile_id, params.account_id],
    )
    .map_err(AppError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::{create_account, CreateAccountParams};

    fn mk_account(conn: &Connection) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: "Test Account".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .expect("create account failed")
    }

    fn mk_profile(conn: &Connection, name: &str) -> i64 {
        create_import_profile(
            conn,
            CreateImportProfileParams {
                name: name.to_owned(),
                column_mapping_json: "{}".to_owned(),
                rules: Vec::new(),
            },
        )
        .expect("create profile failed")
    }

    #[test]
    fn preferred_profile_is_none_by_default() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        assert!(get_preferred_profile(&conn, account_id).unwrap().is_none());
    }

    #[test]
    fn set_then_get_preferred_profile() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let profile_id = mk_profile(&conn, "Bank CSV");

        set_preferred_profile(
            &conn,
            SetPreferredProfileParams {
                account_id,
                profile_id: Some(profile_id),
            },
        )
        .unwrap();

        let profile = get_preferred_profile(&conn, account_id)
            .unwrap()
            .expect("preferred profile should be set");
        assert_eq!(profile.id, profile_id);
        assert_eq!(profile.name, "Bank CSV");
    }

    #[test]
    fn clearing_preferred_profile_returns_none() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let profile_id = mk_profile(&conn, "Bank CSV");
        set_preferred_profile(
            &conn,
            SetPreferredProfileParams {
                account_id,
                profile_id: Some(profile_id),
            },
        )
        .unwrap();

        set_preferred_profile(
            &conn,
            SetPreferredProfileParams {
                account_id,
                profile_id: None,
            },
        )
        .unwrap();

        assert!(get_preferred_profile(&conn, account_id).unwrap().is_none());
    }

    #[test]
    fn deleting_profile_clears_preferred_profile() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let profile_id = mk_profile(&conn, "Bank CSV");
        set_preferred_profile(
            &conn,
            SetPreferredProfileParams {
                account_id,
                profile_id: Some(profile_id),
            },
        )
        .unwrap();

        delete_import_profile(&conn, profile_id).unwrap();

        let stored: Option<i64> = conn
            .query_row(
                "SELECT preferred_profile_id FROM account WHERE id = ?1",
                params![account_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored, None);
        assert!(get_preferred_profile(&conn, account_id).unwrap().is_none());
    }
}
