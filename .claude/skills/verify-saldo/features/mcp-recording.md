# Recording documents through `saldo mcp`

`saldo mcp` is a second mode of the Saldo binary that serves six MCP tools on stdin and stdout, so Claude can record invoices without driving the window. `record_batch` writes revenue, expense and prepaid entries, split groups for several VAT rates, credit notes and assets, links their payments, and refuses a document that is already recorded. `undo_batch` removes a batch. The open app shows MCP writes the next time its window gets focus, with an "Updated with changes made outside Saldo" toast.

## Sub-features

- `mcp-context` returns the database path, persons, tax models and recent batches, and refuses with a sentence when the database is behind, ahead, missing or in a missing custom folder.
- `mcp-dry-run` previews a batch through the real write path and writes nothing. A refused batch lists every failing document.
- `mcp-record` writes a batch: the entry date from the delivery date or the issue date, one entry or a split group, prepaid shares, an asset at its net or gross price with depreciation and a starting value dated at purchase, and payment links.
- `mcp-duplicate` refuses a document whose issuer and number are already recorded, in any letter case.
- `mcp-review` refuses a write that does not carry the `reviewToken` of a dry run of the identical batch.
- `mcp-undo` removes a batch's entries and assets. A second undo changes nothing.
- `mcp-refresh` refreshes the open app after an MCP write when its window regains focus, and shows the toast.

## How to get to it (user POV)

- Claude Code with the server registered: `claude mcp add --scope user saldo -- /Applications/saldo.app/Contents/MacOS/saldo mcp`. The tools appear as `mcp__saldo__*`, and the `record-invoices` skill drives them.
- In a verification run: `scripts/mcp.sh`, which starts `saldo mcp` from the run's `Saldo Dev.app` bundle. Never start a plain `cargo build` binary with `mcp`: it carries the real identifier and opens the real database.
- The refresh: switch back to the Saldo window after an MCP write.

## Driving it with computer use

Preconditions:

- Baseline from the [README](./README.md), on the dev DB, started with `launch.sh`. `mcp.sh` refuses without the run's bundle, and `mcp-drive.py` stops before the first step unless `saldo_context` reports the dev DB path.
- The dev DB holds the CSV fixture rows on `Verify Checking` (person `Personal`, id 1): `verify-saldo coffee` -12.50 and `verify-saldo refund` 250.00. On the current dev DB their ids are 8 and 9, which `fixtures/mcp-batch.json` links. After `reset-dev-db.sh`, run the [CSV import](./csv-import.md) recipe first and put the new ids into the fixture.
- Tax model 1 belongs to person 1 for 2026.

- **Record (`mcp-record`, `mcp-duplicate`, `mcp-review`).** Run `scripts/mcp.sh 20-mcp-record .claude/skills/verify-saldo/fixtures/mcp-batch.json` with the sandbox disabled. The transcript shows `find_payments` listing ids 8 and 9. A write sent without a dry run comes back `isError: true` with `REVIEW_TOKEN_MISMATCH`. Each `record` step sends a dry run, which answers with a `reviewToken`, and then the write with that token, which answers `isError: false` with a `batchId`. The repeat of `ver-001` stops at its dry run with `BATCH_REFUSED` and `ALREADY_RECORDED`, so no write is sent. The forecast for model 1 shows `revenue` so far 25000, `depreciation` 42344, `deductible_costs` 14850 and `non_deductible_costs` 50 (model 1 is not a VAT payer, so every VAT counts as cost).
- **Stored rows.** Run `evidence.sh db 21-mcp-rows "select e.id, e.event_type, d.event_date, d.amount_minor, e.source_ref, e.batch_id, e.linked_prepaid_event_id from event e join event_data d on d.id = e.latest_data_id where e.deleted_at is null and (e.batch_id = <batch> or e.linked_prepaid_event_id in (select id from event where batch_id = <batch>))"`. It shows the single entries, the two split legs of `VER-002`, the credit note at -500, the revenue, the prepaid parent with shares of 10060 and 1940, the sleeve and the laptop's balance update of 203252 dated 2026-03-04. `select * from account where batch_id = <batch>` shows `Verify MCP laptop` at 203252 over 48 months.
- **Refresh (`mcp-refresh`).** Before the record step, open `AXLink "Ledger"` and click the `Revenue` chip: it shows "No events to display." After the record step the open window still shows the old list, because a background click does not give it focus. Fire the hook's `visibilitychange` path instead: `app_menu` `["Window", "Minimize"]`, then `app_bring_to_current_space` with the window id, then `app_screenshot`. `Verify MCP issued invoice` 250.00 EUR appears with the linked check, `Unmatched cashflows` drops by 2, and the toast `Updated with changes made outside Saldo` shows. The `Prepaid Expenses` chip shows `Verify MCP yearly licence`.
- **App edit stops the undo.** Edit `Verify MCP yearly licence` with its pencil: triple-click `Amount`, type `121.00`, click `Save`. An `undo_batch` without `includeChanged` then answers `BATCH_CHANGED` with that event and `edited_in_saldo`.
- **Undo (`mcp-undo`).** Write `[{"call": "undo_batch", "args": {"batchId": <batch>}}, {"call": "undo_batch", "args": {"batchId": <batch>}}]` to a file in the scratchpad and run it through `mcp.sh 22-mcp-undo <file>`. The first answer has `alreadyUndone: false` and lists the removed event ids and the asset account. The second has `alreadyUndone: true`. The stored-rows query returns nothing, and the Ledger loses the rows after the window regains focus.
- **Dry run (`mcp-dry-run`).** Turn a `record` step into `{"call": "record_batch", "args": {..., "dryRun": true}}`. The answer has `dryRun: true`, a `reviewToken`, null ids and the derived facts (`dateSource`, `shape`, `priceBasis`), and the row count does not change.

## Gotchas

- Last driven end to end on 2026-10-30, on the dev DB. Run `20261029-212440-saldo-mcp` drove `mcp-context`, `mcp-dry-run`, `mcp-record`, `mcp-duplicate` and `mcp-undo` through the stdio client. Run `20261030-071022-saldo-mcp` drove `mcp-review`, `mcp-record`, `mcp-duplicate`, `mcp-refresh` (through the `visibilitychange` path; the focus path needs full-screen control) and `mcp-undo` stopping at an edit made in the window. A real Claude Code client was not tried, because the bundled `claude` CLI was not signed in.
- The duplicate check counts live rows only. An undone batch's documents can be recorded again, which is how the fixture reruns.
- Entries store a bare date (`2026-10-01`), the same as entries created in the app. Generated rows (prepaid shares, depreciation) carry a time.
- A document with only assets refuses `paymentIds`, because Saldo links payments to entries, not to assets.
- A forecast line counts entries only for months the model marks as actual and only while the line's year-so-far is not typed. A line that did not move usually has one of those two causes.
