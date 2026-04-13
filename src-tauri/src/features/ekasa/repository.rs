use crate::error::AppError;
use crate::features::ekasa::models::{EkasaImportProfileRow, EkasaRuleInput, EkasaRuleRow};
use crate::shared::{local_now, with_savepoint_app};
use rusqlite::{params, Connection, OptionalExtension};

pub struct UpsertEkasaProfileParams {
    pub person_id: i64,
    pub default_deductible_pct_bps: i64,
    pub default_vat_reclaimable_pct_bps: i64,
    pub rules: Vec<EkasaRuleInput>,
}

fn load_rules_for_profile(
    conn: &Connection,
    profile_id: i64,
) -> Result<Vec<EkasaRuleRow>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, profile_id, sort_order, name_pattern, deductible_pct_bps, vat_reclaimable_pct_bps
         FROM ekasa_import_profile_rule
         WHERE profile_id = ?1
         ORDER BY sort_order ASC",
    )?;
    let rows = stmt.query_map(params![profile_id], |row| {
        Ok(EkasaRuleRow {
            id: row.get(0)?,
            profile_id: row.get(1)?,
            sort_order: row.get(2)?,
            name_pattern: row.get(3)?,
            deductible_pct_bps: row.get(4)?,
            vat_reclaimable_pct_bps: row.get(5)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)
}

pub fn get_ekasa_profile_for_person(
    conn: &Connection,
    person_id: i64,
) -> Result<Option<EkasaImportProfileRow>, AppError> {
    let profile = conn
        .query_row(
            "SELECT id, person_id, default_deductible_pct_bps, default_vat_reclaimable_pct_bps, created_at, updated_at
             FROM ekasa_import_profile
             WHERE person_id = ?1",
            params![person_id],
            |row| {
                Ok(EkasaImportProfileRow {
                    id: row.get(0)?,
                    person_id: row.get(1)?,
                    default_deductible_pct_bps: row.get(2)?,
                    default_vat_reclaimable_pct_bps: row.get(3)?,
                    rules: Vec::new(),
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
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

pub fn upsert_ekasa_profile(
    conn: &Connection,
    params: UpsertEkasaProfileParams,
) -> Result<EkasaImportProfileRow, AppError> {
    with_savepoint_app(conn, || {
        let now = local_now();
        conn.execute(
            "INSERT INTO ekasa_import_profile
                 (person_id, default_deductible_pct_bps, default_vat_reclaimable_pct_bps, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(person_id) DO UPDATE SET
                 default_deductible_pct_bps     = excluded.default_deductible_pct_bps,
                 default_vat_reclaimable_pct_bps = excluded.default_vat_reclaimable_pct_bps,
                 updated_at                      = excluded.updated_at",
            params![
                params.person_id,
                params.default_deductible_pct_bps,
                params.default_vat_reclaimable_pct_bps,
                now
            ],
        )
        .map_err(AppError::from)?;

        let profile_id: i64 = conn
            .query_row(
                "SELECT id FROM ekasa_import_profile WHERE person_id = ?1",
                params![params.person_id],
                |row| row.get(0),
            )
            .map_err(AppError::from)?;

        conn.execute(
            "DELETE FROM ekasa_import_profile_rule WHERE profile_id = ?1",
            params![profile_id],
        )
        .map_err(AppError::from)?;

        for rule in &params.rules {
            conn.execute(
                "INSERT INTO ekasa_import_profile_rule
                     (profile_id, sort_order, name_pattern, deductible_pct_bps, vat_reclaimable_pct_bps)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    profile_id,
                    rule.sort_order,
                    rule.name_pattern.as_str(),
                    rule.deductible_pct_bps,
                    rule.vat_reclaimable_pct_bps
                ],
            )
            .map_err(AppError::from)?;
        }

        let profile = conn
            .query_row(
                "SELECT id, person_id, default_deductible_pct_bps, default_vat_reclaimable_pct_bps, created_at, updated_at
                 FROM ekasa_import_profile
                 WHERE id = ?1",
                params![profile_id],
                |row| {
                    Ok(EkasaImportProfileRow {
                        id: row.get(0)?,
                        person_id: row.get(1)?,
                        default_deductible_pct_bps: row.get(2)?,
                        default_vat_reclaimable_pct_bps: row.get(3)?,
                        rules: Vec::new(),
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                    })
                },
            )
            .map_err(AppError::from)?;

        let rules = load_rules_for_profile(conn, profile.id)?;
        Ok(EkasaImportProfileRow { rules, ..profile })
    })
}

pub fn delete_ekasa_profile(conn: &Connection, profile_id: i64) -> Result<(), AppError> {
    let affected = conn
        .execute(
            "DELETE FROM ekasa_import_profile WHERE id = ?1",
            params![profile_id],
        )
        .map_err(AppError::from)?;
    if affected == 0 {
        return Err(AppError {
            code: "NOT_FOUND".into(),
            message: "eKasa import profile not found.".into(),
        });
    }
    Ok(())
}

pub fn list_ekasa_profiles(conn: &Connection) -> Result<Vec<EkasaImportProfileRow>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, person_id, default_deductible_pct_bps, default_vat_reclaimable_pct_bps, created_at, updated_at
         FROM ekasa_import_profile
         ORDER BY id ASC",
    )?;
    let profile_list: Vec<EkasaImportProfileRow> = stmt
        .query_map([], |row| {
            Ok(EkasaImportProfileRow {
                id: row.get(0)?,
                person_id: row.get(1)?,
                default_deductible_pct_bps: row.get(2)?,
                default_vat_reclaimable_pct_bps: row.get(3)?,
                rules: Vec::new(),
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::persons::repository::{create_person, CreatePersonParams};

    fn get_default_person_id(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM person WHERE is_default = 1", [], |row| {
            row.get(0)
        })
        .expect("default person not found")
    }

    fn make_rule(sort_order: i64, name_pattern: &str) -> EkasaRuleInput {
        EkasaRuleInput {
            sort_order,
            name_pattern: name_pattern.to_owned(),
            deductible_pct_bps: 5000,
            vat_reclaimable_pct_bps: 5000,
        }
    }

    #[test]
    fn create_and_read_profile() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = get_default_person_id(&conn);

        let row = upsert_ekasa_profile(
            &conn,
            UpsertEkasaProfileParams {
                person_id,
                default_deductible_pct_bps: 7500,
                default_vat_reclaimable_pct_bps: 10000,
                rules: vec![make_rule(1, "Cafe")],
            },
        )
        .expect("upsert failed");

        assert_eq!(row.person_id, person_id);
        assert_eq!(row.default_deductible_pct_bps, 7500);
        assert_eq!(row.default_vat_reclaimable_pct_bps, 10000);
        assert_eq!(row.rules.len(), 1);
        assert_eq!(row.rules[0].name_pattern, "Cafe");

        let fetched = get_ekasa_profile_for_person(&conn, person_id)
            .expect("get failed")
            .expect("profile should exist");
        assert_eq!(fetched.id, row.id);
        assert_eq!(fetched.rules.len(), 1);
    }

    #[test]
    fn upsert_updates_existing_profile() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = get_default_person_id(&conn);

        let first = upsert_ekasa_profile(
            &conn,
            UpsertEkasaProfileParams {
                person_id,
                default_deductible_pct_bps: 5000,
                default_vat_reclaimable_pct_bps: 5000,
                rules: vec![],
            },
        )
        .expect("first upsert failed");

        let second = upsert_ekasa_profile(
            &conn,
            UpsertEkasaProfileParams {
                person_id,
                default_deductible_pct_bps: 10000,
                default_vat_reclaimable_pct_bps: 10000,
                rules: vec![],
            },
        )
        .expect("second upsert failed");

        assert_eq!(first.id, second.id, "upsert should return the same row ID");
        assert_eq!(second.default_deductible_pct_bps, 10000);
        assert_eq!(second.default_vat_reclaimable_pct_bps, 10000);

        let all = list_ekasa_profiles(&conn).expect("list failed");
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn upsert_replaces_rules_atomically() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = get_default_person_id(&conn);

        upsert_ekasa_profile(
            &conn,
            UpsertEkasaProfileParams {
                person_id,
                default_deductible_pct_bps: 10000,
                default_vat_reclaimable_pct_bps: 10000,
                rules: vec![make_rule(1, "OldRule1"), make_rule(2, "OldRule2")],
            },
        )
        .expect("first upsert failed");

        let updated = upsert_ekasa_profile(
            &conn,
            UpsertEkasaProfileParams {
                person_id,
                default_deductible_pct_bps: 10000,
                default_vat_reclaimable_pct_bps: 10000,
                rules: vec![make_rule(1, "NewRule")],
            },
        )
        .expect("second upsert failed");

        assert_eq!(updated.rules.len(), 1);
        assert_eq!(updated.rules[0].name_pattern, "NewRule");
    }

    #[test]
    fn one_profile_per_person_constraint() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = get_default_person_id(&conn);

        upsert_ekasa_profile(
            &conn,
            UpsertEkasaProfileParams {
                person_id,
                default_deductible_pct_bps: 5000,
                default_vat_reclaimable_pct_bps: 5000,
                rules: vec![],
            },
        )
        .expect("first upsert failed");

        upsert_ekasa_profile(
            &conn,
            UpsertEkasaProfileParams {
                person_id,
                default_deductible_pct_bps: 8000,
                default_vat_reclaimable_pct_bps: 8000,
                rules: vec![],
            },
        )
        .expect("second upsert (update) must succeed");

        let all = list_ekasa_profiles(&conn).expect("list failed");
        assert_eq!(
            all.len(),
            1,
            "UNIQUE(person_id) — only one profile per person"
        );
        assert_eq!(all[0].default_deductible_pct_bps, 8000);
    }

    #[test]
    fn delete_removes_profile_and_cascades_rules() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = get_default_person_id(&conn);

        let row = upsert_ekasa_profile(
            &conn,
            UpsertEkasaProfileParams {
                person_id,
                default_deductible_pct_bps: 10000,
                default_vat_reclaimable_pct_bps: 10000,
                rules: vec![make_rule(1, "Rule1"), make_rule(2, "Rule2")],
            },
        )
        .expect("upsert failed");

        delete_ekasa_profile(&conn, row.id).expect("delete failed");

        let fetched = get_ekasa_profile_for_person(&conn, person_id).expect("get failed");
        assert!(fetched.is_none(), "profile should be gone after delete");

        let rule_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ekasa_import_profile_rule WHERE profile_id = ?1",
                params![row.id],
                |r| r.get(0),
            )
            .expect("rule count query failed");
        assert_eq!(rule_count, 0, "cascade delete should remove all rules");
    }

    #[test]
    fn list_returns_all_profiles_with_rules() {
        let conn = initialize_in_memory().expect("DB init failed");
        let default_person_id = get_default_person_id(&conn);

        let second_person_id = create_person(
            &conn,
            CreatePersonParams {
                name: "Other Person".to_owned(),
                person_type: "physical".to_owned(),
                vat_payer: false,
            },
        )
        .expect("create second person failed");

        upsert_ekasa_profile(
            &conn,
            UpsertEkasaProfileParams {
                person_id: default_person_id,
                default_deductible_pct_bps: 10000,
                default_vat_reclaimable_pct_bps: 10000,
                rules: vec![make_rule(1, "Food")],
            },
        )
        .expect("upsert 1 failed");

        upsert_ekasa_profile(
            &conn,
            UpsertEkasaProfileParams {
                person_id: second_person_id,
                default_deductible_pct_bps: 5000,
                default_vat_reclaimable_pct_bps: 5000,
                rules: vec![make_rule(1, "Office"), make_rule(2, "Travel")],
            },
        )
        .expect("upsert 2 failed");

        let all = list_ekasa_profiles(&conn).expect("list failed");
        assert_eq!(all.len(), 2);

        let first = all
            .iter()
            .find(|p| p.person_id == default_person_id)
            .expect("first profile not found");
        assert_eq!(first.rules.len(), 1);
        assert_eq!(first.rules[0].name_pattern, "Food");

        let second = all
            .iter()
            .find(|p| p.person_id == second_person_id)
            .expect("second profile not found");
        assert_eq!(second.rules.len(), 2);
    }

    #[test]
    fn get_returns_none_for_person_with_no_profile() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = get_default_person_id(&conn);
        let result = get_ekasa_profile_for_person(&conn, person_id).expect("get failed");
        assert!(result.is_none());
    }

    #[test]
    fn delete_returns_not_found_for_missing_id() {
        let conn = initialize_in_memory().expect("DB init failed");
        let result = delete_ekasa_profile(&conn, 99999);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "NOT_FOUND");
    }

    #[test]
    fn delete_person_cascades_to_profile() {
        let conn = initialize_in_memory().expect("DB init failed");

        // Insert a bare person directly (no default accounts) so it can be
        // deleted without hitting ON DELETE RESTRICT from the account table.
        conn.execute(
            "INSERT INTO person (name, person_type, is_default, created_at, vat_payer) VALUES ('Cascade Test', 'physical', 0, '2024-01-01T00:00:00', 0)",
            [],
        )
        .expect("insert person failed");
        let person_id = conn.last_insert_rowid();

        upsert_ekasa_profile(
            &conn,
            UpsertEkasaProfileParams {
                person_id,
                default_deductible_pct_bps: 5000,
                default_vat_reclaimable_pct_bps: 5000,
                rules: vec![make_rule(1, "AnyRule")],
            },
        )
        .expect("upsert failed");

        conn.execute("DELETE FROM person WHERE id = ?1", params![person_id])
            .expect("person delete failed");

        let fetched = get_ekasa_profile_for_person(&conn, person_id).expect("get failed");
        assert!(
            fetched.is_none(),
            "profile should be cascaded away when person is deleted"
        );
    }

    #[test]
    fn rules_returned_in_sort_order() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = get_default_person_id(&conn);

        upsert_ekasa_profile(
            &conn,
            UpsertEkasaProfileParams {
                person_id,
                default_deductible_pct_bps: 10000,
                default_vat_reclaimable_pct_bps: 10000,
                rules: vec![
                    make_rule(3, "Third"),
                    make_rule(1, "First"),
                    make_rule(2, "Second"),
                ],
            },
        )
        .expect("upsert failed");

        let profile = get_ekasa_profile_for_person(&conn, person_id)
            .expect("get failed")
            .expect("profile should exist");

        assert_eq!(profile.rules.len(), 3);
        assert_eq!(profile.rules[0].name_pattern, "First");
        assert_eq!(profile.rules[1].name_pattern, "Second");
        assert_eq!(profile.rules[2].name_pattern, "Third");
    }
}
