# Recording documents through `saldo mcp`

`saldo mcp` is a second mode of the Saldo binary that serves seven MCP tools on stdin and stdout, so Claude can record invoices and eKasa receipts without driving the window. `read_receipt` decodes a receipt's QR code and, for an online receipt, fetches its lines from Finančná správa. `record_batch` writes revenue, expense and prepaid entries, split groups for several VAT rates or receipt items, credit notes and assets, links their payments, and refuses a document or receipt that is already recorded. `undo_batch` removes a batch. The open app shows MCP writes the next time its window gets focus, with an "Updated with changes made outside Saldo" toast.

## Sub-features

- `mcp-context` returns the database path, persons, tax models and recent batches, and refuses with a sentence when the database is behind, ahead, missing or in a missing custom folder.
- `mcp-dry-run` previews a batch through the real write path and writes nothing. A refused batch lists every failing document.
- `mcp-record` writes a batch: the entry date from the delivery date or the issue date, one entry or a split group, prepaid shares, an asset at its net or gross price with depreciation and a starting value dated at purchase, and payment links.
- `mcp-duplicate` refuses a document whose issuer and number are already recorded, in any letter case.
- `mcp-review` refuses a write that does not carry the `reviewToken` of a dry run of the identical batch.
- `mcp-undo` removes a batch's entries and assets. A second undo changes nothing.
- `mcp-refresh` refreshes the open app after an MCP write when its window regains focus, and shows the toast.
- `mcp-receipt-read` reads an offline receipt from a PNG and from a PDF (PDFium from the bundle's `Contents/Resources`, working directory `/`), refuses a non-eKasa QR code and a relative path, and looks up a typed online id at Finančná správa when the file's QR code does not decode. A typed id that names another receipt than a decoded QR code is refused.
- `mcp-receipt-record` records a receipt as one leg per item with the item names as notes and the shop as the group note. Each line's shares come from the user, else the person's eKasa rule (accents ignored), else Claude, and the dry run names the source. It refuses missing shares, an offline receipt whose lines or date do not match its QR code, a changed reading, and a receipt already in Saldo.

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
- **Receipts (`mcp-receipt-read`, `mcp-receipt-record`).** Person 1 needs an eKasa profile with rules `káva|čaj` 100/100, `pivo` 0/0 and the uncompilable `ka(va`. The wizard saves profiles, but its `Import` menu and file panel need display-scope control; without it, seed the dev DB (the app may run):

  ```sql
  INSERT INTO ekasa_import_profile (person_id, default_deductible_pct_bps, default_vat_reclaimable_pct_bps, created_at, updated_at) VALUES (1, 10000, 10000, '2026-10-30T17:30:00', '2026-10-30T17:30:00');
  INSERT INTO ekasa_import_profile_rule (profile_id, sort_order, name_pattern, deductible_pct_bps, vat_reclaimable_pct_bps) SELECT id, 0, 'káva|čaj', 10000, 10000 FROM ekasa_import_profile WHERE person_id = 1;
  INSERT INTO ekasa_import_profile_rule (profile_id, sort_order, name_pattern, deductible_pct_bps, vat_reclaimable_pct_bps) SELECT id, 1, 'pivo', 0, 0 FROM ekasa_import_profile WHERE person_id = 1;
  INSERT INTO ekasa_import_profile_rule (profile_id, sort_order, name_pattern, deductible_pct_bps, vat_reclaimable_pct_bps) SELECT id, 2, 'ka(va', 10000, 0 FROM ekasa_import_profile WHERE person_id = 1;
  ```

  Run `scripts/mcp.sh 30-mcp-receipts .claude/skills/verify-saldo/fixtures/mcp-receipts.json` with the sandbox disabled. It needs the network for one lookup. In order:
  - `not-ekasa.png` gives `QR_NOT_EKASA`, and a relative path gives `INVALID_ARGUMENTS`.
  - The PNG and the PDF read as `kind: offline`, date `2026-10-15`, `totalMinor` 2340.
  - The typed id `O-000…0` with `not-ekasa.png` gives `RECEIPT_NOT_FOUND` from Finančná správa. The same id with the offline PDF gives `VALIDATION`, because the PDF's QR code names another receipt.
  - A dry run without shares gives `SHARES_NEEDED` naming the milk and paper lines, which no rule matches.
  - Beer misread as 2.05 gives `RECEIPT_TOTAL_DIFFERS` (22.95 against 23.40).
  - Date 2026-10-16 gives `RECEIPT_DATE_DIFFERS`.
  - A zeroed `readToken` gives `RECEIPT_CHANGED`.
  - The `record` step writes a split group of four legs with these sources: coffee `rule` 1 (`KAVA` against `káva|čaj`, with Claude's 0/0 in `shares.claude`), milk `claude` 0/0, paper `user` 100/100, beer `rule` 2 0/0. It also carries the warnings `payment_amount_differs` (payment 8 linked) and `invalid_ekasa_rule` (position 3, "unclosed group").
  - The later read reports `alreadyRecorded`, and a second batch gets `ALREADY_RECORDED`.

  `evidence.sh db` on `event` with `batch_id = <batch>` shows the four legs with their notes, shares and the `offline:` reference, and `split_group` holds the shop. In the window, the Ledger shows `Personal`, `Split ×4`, `VERIFY POTRAVINY s.r.o.`, 23.40 EUR on 15 October 2026 after the `visibilitychange` refresh, and `Unmatched cashflows` drops by one. Undo it as in `mcp-undo`; `read_receipt` then reports `alreadyRecorded: null`.

## Gotchas

- Receipts were last driven on 2026-10-30 in run `20261030-171812-feat-mcp-ekasa-receipts`: `mcp-receipt-read` and `mcp-receipt-record` as above, plus `mcp-undo` and a rerun of `mcp-batch.json` (invoices unchanged, same forecast). An online receipt with real items was not driven, because no real receipt id was at hand; the typed-id lookup proved only the network path. The wizard's rules step was not driven either (no display-scope control). Since then the typed id is used only when the QR code does not decode: the fixture's two typed-id steps for that rule have not been driven yet, and cargo tests cover them.
- `read_receipt` takes absolute paths only, so a steps file names its receipts as `${FIXTURES}/receipts/...`. A `read_receipt` step with `"save": "pdf"` lets a later step pass `"$pdf.receipt"` and `"$pdf.date"` (see `mcp-drive.py`).
- Last driven end to end on 2026-10-30, on the dev DB. Run `20261029-212440-saldo-mcp` drove `mcp-context`, `mcp-dry-run`, `mcp-record`, `mcp-duplicate` and `mcp-undo` through the stdio client. Run `20261030-071022-saldo-mcp` drove `mcp-review`, `mcp-record`, `mcp-duplicate`, `mcp-refresh` (through the `visibilitychange` path; the focus path needs full-screen control) and `mcp-undo` stopping at an edit made in the window. A real Claude Code client was not tried, because the bundled `claude` CLI was not signed in.
- The duplicate check counts live rows only. An undone batch's documents can be recorded again, which is how the fixture reruns.
- Entries store a bare date (`2026-10-01`), the same as entries created in the app. Generated rows (prepaid shares, depreciation) carry a time.
- A document with only assets refuses `paymentIds`, because Saldo links payments to entries, not to assets.
- A forecast line counts entries only for months the model marks as actual and only while the line's year-so-far is not typed. A line that did not move usually has one of those two causes.
