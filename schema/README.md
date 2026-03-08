Schema description — saldo (MVP)

This folder contains canonical SQL schema and example seeds for the MVP.

Files
- ddl.sql — canonical schema DDL (create tables, indexes, trigger). Not an applied migration.
- seeds.sql — example seed data and a short usage snippet (development/testing only).

Recommended PRAGMAs (set these on DB connection before use)
- PRAGMA foreign_keys = ON;   -- enforce foreign key constraints
- PRAGMA journal_mode = WAL; -- enable write-ahead logging for better concurrency
- PRAGMA synchronous = NORMAL; -- balance durability and throughput for desktop app
- PRAGMA page_size = 4096;   -- optional tuning

Notes & usage
- `latest_data_id` is nullable to support simple insert flow: insert `event`, insert `event_data`, trigger updates `latest_data_id`.
- All datetime columns store ISO local datetimes (YYYY-MM-DDTHH:MM:SS[.fff]); the UI may pass dates as end-of-day (`YYYY-MM-DDT23:59:59`) for snapshots.
- Monetary values are stored as integer minor units (`amount_minor`), e.g., EUR cents. Use 64-bit integers in Rust (`i64`).

Snapshot query
- Use the correlated-subquery pattern documented in `ddl.sql` or implement equivalent snapshot logic in Rust using the same SQL to ensure correctness.

Applying schema
- Do not run `ddl.sql` as a migration in production without review. Prefer an ordered migrations folder (e.g., `/migrations/001_initial.sql`) applied by your migration runner.

Artefact structure (recommended)
- /migrations/           -- ordered, idempotent SQL migration files used at startup
- /schema/ddl.sql        -- canonical, human-readable schema reference (this file)
- /schema/seeds.sql      -- development/example seeds (this file)
- /schema/README.md      -- this document
- /schema/er-diagram.svg -- optional visual ER diagram (tool-generated)

