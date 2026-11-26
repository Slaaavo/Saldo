# CSV import

The CSV import wizard turns a bank export into cashflow events on one account. The user picks a file and the target account, maps columns (date, amount, partner IBAN, currency, FX rate, note), optionally adds transformation rules, reviews the rows (new partners, transfers to own accounts, splits, bucket tags), and imports them in one write. Rust decides which rows Saldo already has (`preview_csv_import`), and the review folds those under "N already in Saldo". At the end the wizard offers to save the setup as an import profile.

## Sub-features

- `csv-upload` accepts a `.csv` file and a target account, with an optional saved profile.
- `csv-map` auto-detects `Date`, `Amount`, `IBAN`, and `Note` or `Description` headers, and lets the user remap.
- `csv-rules` adds optional transformation rules before review.
- `csv-review` lists the new rows with new-partner, transfer and split handling, folds the rows already in Saldo under a count, and imports the selected ones.
- `csv-match` is the matching: a re-imported export adds nothing, a transfer imported from both accounts ends as one pair with each leg on its own bank's day, and a row whose other side is a plain cash flow on the counterpart account completes that transfer.
- `csv-holds` is card holds: a profile's `Card hold from column` rule marks pending rows (badge `Card hold`), its `Declined payment from column` rule skips declined ones (`N declined payment(s) skipped`), and the `Card number` mapping reads the card. A later export posts each hold in place: at the same amount it folds under "already in Saldo" ("Posts the card hold dated <day>"), at another amount it is a new row, ticked ("Card hold posted: A → B"), and a hold the newer export no longer lists shows under `Cancelled card holds`, ticked for removal.
- `csv-profile` saves or updates an import profile after a successful import (listed under Import Profiles).

## How to get to it (user POV)

- Ledger page → `Import` menu → `CSV`.
- Dashboard → Ledger section → `Import` menu → `CSV`.
- Sidebar → `Import Profiles` lists saved profiles. Creating and editing profiles happens in the wizard.

## Driving it with computer use

Preconditions:

- Baseline from the [README](./README.md), on the dev DB, with an account such as `Verify Checking`.
- The fixture is `.claude/skills/verify-saldo/fixtures/cashflows.csv`: two rows, with `Date,Amount,Note` headers.
- The user has approved display-scope control for this run (`request_full_control`). The `Import` menu and the native file panel can't be driven from the background. Call `app_release` before the first display-scope action, and call `release_full_control` as soon as the display-scope part is done. Display-scope screenshots use screen coordinates, not window coordinates.

- **Open the wizard.** Open `AXLink "Ledger"`. With display-scope tools, click the `Import` button. A menu with `CSV` and `eKasa Receipt` drops down. Click `CSV`. The `Import CSV` dialog opens. It shows no step title, only the drop zone (`Drag and drop a CSV file here`, `Browse`), `Select account to import into`, and `Profile` set to `— none —`.
- **Pick the file.** Click `Browse`. A native open sheet appears. Press `cmd+shift+g`, type the fixture's absolute path, and press `return`. The panel opens the `fixtures` folder with `cashflows.csv` selected. Click `Open`. The drop zone now reads `cashflows.csv`.
- **Pick the account.** Click the `Select account to import into` select. It lists accounts only, not buckets. Click `Verify Checking`, then click `Next`.
- **Map columns (`csv-map`).** The dialog reads "Match your CSV columns to the transaction fields". `Date` and `Amount` (both `(required)`) and `Note` show `Auto-detected`. `Partner / IBAN`, `Currency` and `FX Rate` show `— skip —`. Click `Next`.
- **Rules (`csv-rules`).** `Transformation Rules` says "No rules configured. Click Next to skip." and offers `Select rule type` and `Add`. Click `Next`.
- **Review and import (`csv-review`).** The dialog reads `New rows: 2. Already in Saldo: 0.`, with `Select All (2/2)` and a table of `Date`, `Amount`, `Partner / IBAN`, `Note`, `Bucket` (`Select bucket`) and `Actions` (`Split`). The rows are `2026-10-01 -12.50 EUR` and `2026-10-02 250.00 EUR`. Click `Import 2 rows`. The dialog shows `Import Successful`, "Imported 2 transactions", and "Save this configuration as a profile?" with a `Profile name` field, `Save Profile` and `Close`.
- **Save a profile (`csv-profile`).** Click `Profile name`, type `Verify CSV`, and click `Save Profile`. A `Profile saved` toast appears, and the dialog closes. You can use the background tools again from here.
- **Second view.** On the Ledger, the `Cashflows` chip narrows the list to the two rows, each with a `Cashflow` badge and the CSV note. Sidebar → `Import Profiles` lists `Verify CSV` with `3` columns, `0` rules, and `Delete`.
- **Stored rows.** Run `evidence.sh db <nn>-cashflows "select d.event_date, d.amount_minor, d.note from event e join event_data d on d.id = e.latest_data_id join account a on a.id = e.account_id where a.name = 'Verify Checking' and e.event_type = 'cashflow' and e.deleted_at is null order by d.event_date"`. It shows `-1250` and `25000`, dated `2026-10-01T12:00:00` and `2026-10-02T12:00:00`. Then run `evidence.sh db <nn>-profile "select name, column_mapping_json from import_profile"`.

### Already in Saldo and transfers (`csv-match`)

Load `fixtures/csv-transfers.sql` while the app is down (`sqlite3 "$HOME/Library/Application Support/com.teamsumbal.saldo.dev/saldo.db" < .claude/skills/verify-saldo/fixtures/csv-transfers.sql`, sandbox off), then launch. It adds Verify TB, Verify CSOB and Verify Card, own EUR accounts with IBANs and 1000.00 each on 1 Nov. Import, in this order:

1. `fixtures/csv-transfers-card.csv` into Verify Card (`Date,Amount,Note`): `New rows: 1`, a plain +50.00 on 12 Nov. Import it.
2. `fixtures/csv-transfers-tb.csv` into Verify TB (`IBAN` auto-detects): three new rows. -100.00 on 10 Nov shows `Transfer` to Verify CSOB. -50.00 on 11 Nov says "Becomes one transfer with the entry on Verify Card dated 2026-11-12". -4.20 is a plain cash flow. Import all three.
3. `fixtures/csv-transfers-csob.csv` into Verify CSOB: `New rows: 0. Already in Saldo: 1.`, and the button reads `Finish`. Open `1 already in Saldo`: the +100.00 row says "Transfer with Verify TB, moves from 2026-11-10 to 2026-11-11". Click `Finish`. With no saved profile the save-profile step follows and reads "Imported 0 transactions"; click `Close`. An account with an unchanged profile closes instead, with the toast "Nothing new to import".
4. Import `fixtures/csv-transfers-csob.csv` again: `Already in Saldo: 1`, and the row has no "moves" hint any more.

Stored rows: `evidence.sh db <nn>-pairs "select a.name, e.event_type, d.amount_minor, d.event_date, (select a2.name from event o join account a2 on a2.id = o.account_id where o.id = e.linked_event_id) as other, (select be.day from bank_entry be where be.event_id = e.id) as bank_day from event e join event_data d on d.id = e.latest_data_id join account a on a.id = e.account_id where a.name like 'Verify %' and e.event_type in ('cashflow','transfer') and e.deleted_at is null order by a.name, d.event_date"`. Expect exactly five rows. Verify CSOB +10000 on 11 Nov and Verify TB -10000 on 10 Nov are linked to each other. Verify Card +5000 on 12 Nov and Verify TB -5000 on 11 Nov are a `transfer` pair. Verify TB -420 is a `cashflow`. Every row has a bank day.

### Card holds (`csv-holds`)

Load `fixtures/card-holds.sql` while the app is down (as for the transfers fixture), then launch. It adds Verify Holds, an own EUR account at 500.00 on 10 Nov, sorted first, with the preferred profile `Verify TB card` (sign, card hold and declined rules, `Card number` mapped to `Číslo karty`). Import into Verify Holds, with its profile:

1. `fixtures/card-holds-1.csv`: `New rows: 8`, a blue line `1 declined payment skipped`, and a `Card hold` badge on 7 rows (all but the 15 Nov Kaviaren row). Import all 8. The card shows 300.64 EUR. Click "Matches bank": "Verified <today>".
2. `fixtures/card-holds-2.csv`: `New rows: 1. Already in Saldo: 6.`, `1 declined payment skipped`. The new row is GITHUB 18 Nov -32.99, ticked, "Card hold posted: -33.00 EUR → -32.99 EUR". `Cancelled card holds` lists ZSSK.SK 17 Nov -12.00, ticked. Open `6 already in Saldo`: the 2128 KRAJ, Kaufland, Tchibo and both Zeleznice rows say "Posts the card hold dated <day>". Import. The toast reads "Updated 1 card hold to its booked amount. Removed 1 cancelled card hold". The card shows 312.65 EUR and still "Verified <today>".
3. `fixtures/card-holds-2.csv` again: `New rows: 0. Already in Saldo: 7.`, no cancelled holds, `Finish` changes no row.
4. The older `fixtures/card-holds-1.csv` again: `New rows: 0. Already in Saldo: 8.`, `1 declined payment skipped`, no cancelled holds. ZSSK stays removed, and GITHUB, Kaufland and Tchibo are not added a second time. `Finish` changes no row, and the card still shows 312.65 EUR.
5. `fixtures/card-holds-3.csv`: one new row, BENZINA VERIFY 20 Nov -40.00 with a `Card hold` badge. Click `Split`, triple-click part 1's amount, type `-30`, click `+ Add split` (part 2 fills in -10.00). Import: "Imported 2 transactions". The card turns amber, because the hold is dated before the check's day.
6. `fixtures/card-holds-4.csv`: `New rows: 1`, BENZINA VERIFY -38.50, ticked, with an amber warning "The split card hold of -40.00 EUR dated 2026-11-20 was booked at -38.50 EUR. Saldo removes the split hold and imports this row instead. To keep the parts, split this row again here." Untick the row: the line reads "The split card hold of -40.00 EUR dated 2026-11-20 stays and this row is skipped". Tick it again. No `Cancelled card holds` section shows. Import: "Imported 1 transaction. Updated 1 card hold to its booked amount." Both split parts are deleted and the -38.50 row is live, posted from the hold: in the stored rows below its `held_day` is 2026-11-20, `held_amount_minor` -4000 and `posting_change_minor` 150, and neither split part is `cancelled`. The card stays amber by the same difference as after step 5, because the check, made before the replacement, still counts the split hold and not the row that replaced it.

Stored rows: `evidence.sh db <nn>-holds "select d.event_date, d.amount_minor, d.note, e.deleted_at is not null as deleted, be.card, be.hold_seen_to, be.posted_at is not null as posted, be.posting_change_minor, be.held_day, be.held_amount_minor, be.cancelled_at is not null as cancelled, be.cancelled_by_day from event e join event_data d on d.id = e.latest_data_id join account a on a.id = e.account_id left join bank_entry be on be.event_id = e.id where a.name = 'Verify Holds' and e.event_type = 'cashflow' order by d.event_date, d.amount_minor"`. After step 2: every posted hold keeps its own day (Kaufland stays on 18 Nov, Tchibo on 17 Nov) and its `held_day` is that day, every posted note is the booked text (`2128 KRAJ`, `WWW.TCHIBO.SK`), GITHUB is -3299 with `posting_change_minor` 1 and `held_amount_minor` -3300, ZSSK is deleted with `cancelled` and `cancelled_by_day` 2026-11-19, and no row is pending. After step 4 the rows are the same.

### A re-import of a real export

To prove re-imports add nothing on real data, copy a read-only copy of the real `saldo.db` into the dev folder while the app is down, after `reset-dev-db.sh`. Checkpoint the copy first (`PRAGMA wal_checkpoint(TRUNCATE)`), and delete its `oxr_app_id` row from `app_setting`, so the dev app makes no OXR calls. Import an export the real DB already holds, with the account's own profile. The review shows `New rows: 0` and every row under `already in Saldo`. After `Finish`, `select count(*) from bank_entry` equals the file's row count, and a second import of the same file changes no row of `event`, `event_data` or `bank_entry`. Run `reset-dev-db.sh` afterwards, so later runs don't start on real data.

## Gotchas

- Run `20261122-140055-feat-csv-import-writer-in-rust` (22 Nov 2026) drove `csv-match` end to end with full-screen control: a re-import of a real card export (`download-20.11.2026 (2).csv` into Kreditka with its profile, on a copy of the real DB) showed `New rows: 0. Already in Saldo: 88.` twice, and the second import changed no row; then the transfer fixtures gave exactly the five expected rows. With full-screen control the dialog steps sit at fixed screen points once the window is zoomed (double-click its title bar): `Browse`, the account select and `Next` keep their places from step to step.

- Last driven end to end on 2026-10-18, on the dev DB, with display-scope tools that the user approved for the run. Run `20261018-213407-claude-keen-boyd-d370a7` verified `csv-upload`, `csv-map`, `csv-rules` (skipped, no rules), `csv-review`, and `csv-profile`. Duplicates, partners, transfers, splits and bucket tags on the review step weren't exercised.
- Run `20261025-081534-worktree-open-review-fixes` also drove the review step's new-partner flow (type a name in the `New` box, then `Create partner`), a bucket tag, and `Split` into three parts (set part 1, `+ Add part`, which pre-fills the remainder), with a semicolon file whose note header contains commas (`Note, memo,, extra, more`): the mapping step shows that header whole and auto-detects `Date`, `Amount` and `IBAN`. In display scope, wheel scrolling does not move the review table or the page; drag the table's scrollbar, and use `End`/`Home` after clicking blank page space.
- Display-scope tools take over the user's screen, so ask before using them. If the user declines, report `csv-*` as not verified. A display screenshot includes the desktop and the Dock, so crop it to the window before `evidence.sh keep`, with `sips --cropToHeightWidth <h> <w> --cropOffset <y> <x> <in> --out <out>`.
- In the background, a click on `Import` does nothing. The focus-before-trigger keyboard trick from `SKILL.md` might open it: a click on the blank title row followed by three `tab`s reached `Portfolio Item`, so a fourth `tab` plus `down` should land on `Import`. That's untested. The native open panel still needs display scope.
- The dialog's title stays `Import CSV` on every step. Recognize each step by its content.
- Dropping a file onto the dialog works in the webview, because Tauri's own drag-drop handling is off for the window. Computer use has no file-drag source, though.
- The partner column only auto-detects a header named `IBAN`. Rows whose IBAN matches another own account become transfers. The account's own IBAN is ignored.
- Rows after an account's latest balance update change its dashboard balance. Rows before it don't. The fixture's 1–2 October rows left `Verify Checking` at its 18 October balance.
- Re-importing the same file puts every row under `already in Saldo` and leaves nothing ticked. Use a fresh account or new dates for a clean rerun. The dev DB already holds these two rows on `Verify Checking`, and `Verify CSV` is saved as a profile.
