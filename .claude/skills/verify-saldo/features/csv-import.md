# CSV import

The CSV import wizard turns a bank export into cashflow events on one account. The user picks a file and the target account, maps columns (date, amount, partner IBAN, currency, FX rate, note), optionally adds transformation rules, reviews the rows (duplicates, new partners, transfers to own accounts, splits, bucket tags), and imports them. At the end the wizard offers to save the setup as an import profile.

## Sub-features

- `csv-upload` accepts a `.csv` file and a target account, with an optional saved profile.
- `csv-map` auto-detects `Date`, `Amount`, `IBAN`, and `Note` or `Description` headers, and lets the user remap.
- `csv-rules` adds optional transformation rules before review.
- `csv-review` lists parsed rows with duplicate, new-partner, transfer and split handling, and imports the selected ones.
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
- **Review and import (`csv-review`).** The dialog reads `2 rows parsed, 0 possible duplicates`, with `Select All (2/2)` and a table of `Date`, `Amount`, `Partner / IBAN`, `Note`, `Bucket` (`Select bucket`) and `Actions` (`Split`). The rows are `2026-10-01 -12.50 EUR` and `2026-10-02 250.00 EUR`. Click `Import 2 rows`. The dialog shows `Import Successful`, "Imported 2 transactions", and "Save this configuration as a profile?" with a `Profile name` field, `Save Profile` and `Close`.
- **Save a profile (`csv-profile`).** Click `Profile name`, type `Verify CSV`, and click `Save Profile`. A `Profile saved` toast appears, and the dialog closes. You can use the background tools again from here.
- **Second view.** On the Ledger, the `Cashflows` chip narrows the list to the two rows, each with a `Cashflow` badge and the CSV note. Sidebar → `Import Profiles` lists `Verify CSV` with `3` columns, `0` rules, and `Delete`.
- **Stored rows.** Run `evidence.sh db <nn>-cashflows "select d.event_date, d.amount_minor, d.note from event e join event_data d on d.id = e.latest_data_id join account a on a.id = e.account_id where a.name = 'Verify Checking' and e.event_type = 'cashflow' and e.deleted_at is null order by d.event_date"`. It shows `-1250` and `25000`, dated `2026-10-01T12:00:00` and `2026-10-02T12:00:00`. Then run `evidence.sh db <nn>-profile "select name, column_mapping_json from import_profile"`.

## Gotchas

- Last driven end to end on 2026-10-18, on the dev DB, with display-scope tools that the user approved for the run. Run `20261018-213407-claude-keen-boyd-d370a7` verified `csv-upload`, `csv-map`, `csv-rules` (skipped, no rules), `csv-review`, and `csv-profile`. Duplicates, partners, transfers, splits and bucket tags on the review step weren't exercised.
- Display-scope tools take over the user's screen, so ask before using them. If the user declines, report `csv-*` as not verified. A display screenshot includes the desktop and the Dock, so crop it to the window before `evidence.sh keep`, with `sips --cropToHeightWidth <h> <w> --cropOffset <y> <x> <in> --out <out>`.
- In the background, a click on `Import` does nothing. The focus-before-trigger keyboard trick from `SKILL.md` might open it: a click on the blank title row followed by three `tab`s reached `Portfolio Item`, so a fourth `tab` plus `down` should land on `Import`. That's untested. The native open panel still needs display scope.
- The wizard's step titles (`Upload File`, `Map Columns`, `Review & Import`) exist in `en.yml`, but the dialog never shows them. Its title stays `Import CSV`. Recognize each step by its content.
- Dropping a file onto the dialog works in the webview, because Tauri's own drag-drop handling is off for the window. Computer use has no file-drag source, though.
- The partner column only auto-detects a header named `IBAN`. Rows whose IBAN matches another own account become transfers. The account's own IBAN is ignored.
- Rows after an account's latest balance update change its dashboard balance. Rows before it don't. The fixture's 1–2 October rows left `Verify Checking` at its 18 October balance.
- Re-importing the same file flags every row as a duplicate on the review step. Use a fresh account or new dates for a clean rerun. The dev DB already holds these two rows on `Verify Checking`, and `Verify CSV` is saved as a profile.
