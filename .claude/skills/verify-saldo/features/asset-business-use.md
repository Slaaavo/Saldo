# Asset business use and private-use expenses

An asset can take a price with VAT, a reclaimed VAT share and a business-use share. Below 100 % business use with VAT reclaimed, the price without VAT depreciates, lost VAT becomes one non-deductible entry in the purchase month, and the dashboard starts at the full price. The share can change from a month the user picks, and earlier months keep their share. An expense, an eKasa rule and a `saldo mcp` line can say the cost is also used privately, so its lost VAT is not deductible.

## Sub-features

- `abu-create` creates a mixed-use asset in the dialog. The summary shows what depreciates, the VAT reclaimed and the lost VAT, the Purchase price locks, and an empty Initial value takes the start value.
- `abu-entries` writes one lost VAT entry in the purchase month beside that month's depreciation, and each month's deductible share.
- `abu-forecast` puts the deductible part on Depreciation and the rest plus lost VAT on Non-deductible costs.
- `abu-restart` keeps every entry after a restart: none doubles and none goes missing.
- `abu-change` adds a share from a month in the edit dialog. Earlier months keep their share, and an edit of the price keeps the change.
- `abu-expense` shows the Also used privately switch only when part of an expense's VAT is reclaimed, and lowers its deductible part.
- `abu-rule` carries an eKasa rule's switch into a line.
- `abu-mcp` records an asset with `businessUse` and a line with `privateUse`, shows the split in the review, and undoes all of it.
- `abu-undo-edited` refuses an undo after a share was edited in Saldo.

## How to get to it (user POV)

- Dashboard → Assets → `Add Asset`, for a legal person in the consolidation currency. The switch sits under the Depreciation section. VAT fields appear for a VAT payer.
- The asset card's menu → `Edit`, for the same fields and `Change from a month`.
- Ledger → `Add Events` → Expense or Prepaid Expense, and the pencil on an entry.
- Ledger → `Import` → eKasa, on the rules step's `Also private` column.
- `scripts/mcp.sh` with `fixtures/mcp-asset-use.json`.

## Driving it with computer use

Preconditions:

- Baseline from the [README](./README.md), on the dev DB.
- `Verify Co` is a VAT payer, and tax model 2 has VAT `all_year` and actual figures through December. Otherwise the non-deductible part of depreciation is averaged, and the sheet shows a partial figure.
- For `abu-mcp` and `abu-rule`, `Verify Co` needs an eKasa rule `kava` at 80/50 with the switch on. Seed it with the SQL in [Recording documents](./mcp-recording.md), using person 4 and adding `private_use` 1, or tick `Also private` in the wizard.

- **Create (`abu-create`).** Open `Add Asset`. Type Asset name `Verify Kona`, choose `Verify Co`, click `2 yr (24)`, set Purchase date to 12 July 2026. Click `Also used privately`: Business use reads 80 and VAT reclaimed 50. Type Price with VAT `15600` and VAT rate `23`. The summary reads Depreciates `12 682.92`, a month `528.46`, deductible `422.76`, VAT reclaimed `1 458.54`, Lost VAT `1 458.54`. Purchase price is disabled and reads `12682.92`. Click `Create`. The dashboard card reads `15 600.00`.
- **Entries (`abu-entries`).** Run `evidence.sh db` on `event` joined to `event_data` where `linked_asset_id` is the asset: 24 rows with `asset_entry` `depreciation` at 8000, and one `lost_vat` row of 145854 at 0 dated 2026-07-12.
- **Forecast (`abu-forecast`).** Open tax model 2. Depreciation gains 2 536.54. Non-deductible costs gains 634.19 and 1 458.54. Event Breakdown has a `Lost VAT on an asset` row.
- **Restart (`abu-restart`).** Quit and relaunch. Counts by `asset_entry` stay the same.
- **Change (`abu-change`).** Create a 48-month asset bought before 2026. Edit it, click `Change from a month`, type `2026-07` and `80`, click `Add`, save. Rows before July stay at 10000 and rows from July read 8000. Edit the price and save: the change is still listed and the rows keep their shares.
- **Expense (`abu-expense`).** Add a 58.30 expense at 23 %, VAT reclaimed 50, deductible 80. The switch shows once the share is between 0 and 100. Save with it off: the tax sheet row reads 42.28 deductible. Turn it on: 37.91 deductible and 14.94 not. The ledger row shows `Also used privately`.
- **Batch (`abu-mcp`, `abu-rule`).** Run `scripts/mcp.sh 40-asset-use .claude/skills/verify-saldo/fixtures/mcp-asset-use.json` with the sandbox disabled. The dry run shows `priceBasis` `net_lost_vat_apart` and `startValueMinor` 1560000 and 830000. The coffee line reports `privateUse` true from the rule, with 8000/5000. Undo each batch: no account, system event or change row is left.
- **Refused undo (`abu-undo-edited`).** Add a share change to a recorded vehicle in the edit dialog. `undo_batch` answers `BATCH_CHANGED` with `asset_edited_in_saldo`. With `includeChanged` it removes everything.

## Gotchas

- Last driven on 2026-11-07 in runs `20261107-093754` and `20261107-094457`, through `saldo mcp` and the dev DB only: `abu-entries`, `abu-forecast` (from `get_forecast`), `abu-restart`, `abu-rule`, `abu-mcp` and `abu-undo-edited`, with the share edit stood in by a SQL insert. `abu-create`, `abu-change`, `abu-expense` and every window step are not verified yet, because computer-use access was declined.
- Model 2 has typed figures, so Non-deductible costs shows the typed value. Read `untypedMinor` or clear the typed cell to see the entries' sum.
- A person who is not a VAT payer sees only Business use, not the VAT fields.
- Change months are typed as `YYYY-MM`. The dialog takes no month picker, because `type="month"` is not available in the macOS web view.
