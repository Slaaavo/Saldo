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
- The user has approved display-scope control for this run. The `Import` menu, the account select, and the native file panel can't be driven from the background.

- **Open the wizard.** Open `AXLink "Ledger"`. The `AXPopUpButton "Import"` is a dropdown menu. With display-scope tools, click it and then choose `CSV`. The `Import CSV` dialog opens on `Upload File`.
- **Pick the file.** Click `Browse`. A native open panel appears. Press `cmd+shift+g`, type the fixture's absolute path, press `return` twice. The file name replaces `Drag and drop a CSV file here`.
- **Pick the account.** Open the `Select account to import into` select and choose `Verify Checking`. Click `Next`.
- **Map columns.** `Map Columns` shows `Auto-detected` for Date, Amount and Note. Click `Next`, and `Next` again on `Transformation Rules`.
- **Review and import.** `Review & Import` shows `2 rows parsed, 0 possible duplicates`. Click `Import 2 rows`. `Import Successful` reads `Imported 2 transactions`. Click `Close`, or save a profile.
- **Second view.** The Ledger lists the two cashflows with their notes. The `Cashflows` filter chip narrows the list to them.
- **Stored rows.** Run `evidence.sh db <nn>-cashflows "select d.event_date, d.amount_minor, d.note from event e join event_data d on d.id = e.latest_data_id join account a on a.id = e.account_id where a.name = 'Verify Checking' and e.event_type = 'cashflow' and e.deleted_at is null order by d.event_date"`. It shows `-1250` and `25000`.

## Gotchas

- Last driven: not yet driven with this harness. Every step above is from the code and the Playwright specs in `tests/`, not from a verified run. The first agent to drive it updates this file.
- The `Import` trigger, both selects, and the open panel all need display-scope tools. Those take over the user's screen, so ask before using them. If the user declines, report `csv-*` as not verified.
- Dropping a file onto the dialog works in the webview, because Tauri's own drag-drop handling is off for the window. Computer use has no file-drag source, though.
- The partner column only auto-detects a header named `IBAN`. Rows whose IBAN matches another own account become transfers. The account's own IBAN is ignored.
- Rows after an account's latest balance update change its dashboard balance. Rows before it don't.
- Re-importing the same file flags every row as a duplicate on the review step. Use a fresh account or new dates for a clean rerun.
