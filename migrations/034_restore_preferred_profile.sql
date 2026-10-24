-- Migration 034: Restore account.preferred_profile_id.
-- Migration 020 added the column, but migration 024 rebuilt the account table with an
-- explicit column list that omitted it, so no database has the column any more.

ALTER TABLE account ADD COLUMN preferred_profile_id INTEGER REFERENCES import_profile(id) ON DELETE SET NULL;
