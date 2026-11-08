# Buckets

Buckets earmark money that sits in real accounts, such as an emergency fund or holiday savings. A bucket's balance is its own additional balance plus the balances of the accounts linked to it. Left to Spend is Liquid (Total Balance until an asset exists) minus the balance of every bucket that is switched on, so allocating to a bucket lowers Left to Spend without moving money. An income budget is a bucket too, with its own map in [Income budget](./income-budget.md).

## Sub-features

- `bucket-create` adds a bucket from the Buckets section.
- `bucket-extra` sets a bucket's additional balance with `UPDATE BALANCE`.
- `bucket-link` links an account to a bucket so the bucket mirrors that account's balance.
- `bucket-left-to-spend` shows the `LEFT TO SPEND` metric once a bucket exists, equal to Liquid (or Total Balance) minus buckets.
- `bucket-breakdown` shows the balance breakdown tooltip (additional balance, linked accounts, tagged cashflows) on a bucket card.
- `bucket-switch` turns a bucket off and on with the switch in its card's top row. Off, Left to Spend ignores the bucket, and the card greys its amount with an `Off` pill.
- `bucket-switch-breakdown` lists a switched-off bucket under `NOT COUNTED` in the Left to Spend breakdown, as "Switched off on its card."
- `bucket-switch-past` applies the switch at a time-machine date too, because it is a current setting.

## How to get to it (user POV)

- Dashboard → Buckets → `Add Bucket`.
- Dashboard → a bucket card → `UPDATE BALANCE`, with its `LINKED ACCOUNTS` section and `Link Account`.
- Dashboard → hero metrics → `LEFT TO SPEND`.
- Dashboard → hover a linked bucket's amount → `Balance breakdown`.
- Dashboard → a bucket card → the switch next to `⋮`.

## Driving it with computer use

Preconditions:

- Baseline from the [README](./README.md). On the dev DB, accounts such as `Verify Checking` and `Verify Savings` already have balances (see [Balance update](./balance-update.md)). In demo mode, the seeded buckets `Emergency Fund`, `Retirement Fund` and `Holiday Savings` exist.

- **Create a bucket.** On the dashboard, click `AXButton "Add Bucket"`. A `Create Bucket` dialog opens with `Bucket Name`, `Currency` (EUR), and `Initial Balance (optional)`. Click `AXTextField "Bucket Name"`, type `Verify Holiday`, and click `AXButton "Create"`. A `Verify Holiday 0.00 EUR` card appears under Buckets, and `LEFT TO SPEND` appears next to `TOTAL BALANCE` with the same value.
- **Allocate an additional balance (`bucket-extra`).** Click the bucket card's `AXButton "UPDATE BALANCE"`. `Update Balance` opens with Account preset to the bucket, `AXTextField "Additional Balance"` holding a real `0`, `Date`, `Note`, and a `LINKED ACCOUNTS` section with `AXButton "Link Account"`. Triple-click `Additional Balance`, type `200`, add a note, and click `AXButton "Save"`. The bucket card reads `200.00 EUR`, and `LEFT TO SPEND` is `TOTAL BALANCE − 200.00` (on 2026-10-18: `1 850.00 → 1 650.00`). Keep before and after screenshots.
- **Stored row.** Run `evidence.sh db <nn>-bucket-row "select e.id, a.name, a.account_type, d.amount_minor, d.note, src.name as linked_account from account a join event e on e.account_id = a.id join event_data d on d.id = e.latest_data_id left join bucket_event_link l on l.event_id = e.id left join account src on src.id = l.source_account_id where a.name = 'Verify Holiday' and e.deleted_at is null order by e.id"`. It shows `bucket` and `20000`.
- **Link an account (`bucket-link`).** Open the bucket's `UPDATE BALANCE` again. `Additional Balance` starts at `0` again. Each save is a new update, and only the links carry over, so re-type `200`, or the extra balance drops to 0. Click `AXButton "Link Account"`. A row appears with a `Select account to link` select and a `×`. Clicking the select doesn't open it in the background, so use the keyboard path. Click `AXTextField "Note"` to put the focus just before the select. Then `app_key` `tab` moves the focus to the select, `down` opens the list (`Verify Checking`, `Verify Savings`), `down` moves to the second entry, and `return` picks it. The highlight isn't painted in the background, so count the key presses. The row turns into `Verify Savings`, "Balance mirrors this account automatically.", `Unlink`. Keep `<nn>-bucket-link-form`, then click `AXButton "Save"`. The bucket reads `700.00 EUR` (200 extra + 500 linked), and `LEFT TO SPEND` drops by the linked balance (`1 650.00 → 1 150.00`). The stored-row query shows a new event with `20000` and `linked_account` `Verify Savings`.
- **Breakdown tooltip (`bucket-breakdown`).** The tooltip opens on hover only, and its trigger isn't focusable. Run `app_drag` with a `path` that starts on empty space inside the Buckets section and ends on the bucket's amount, for example `[[700,680],[600,672],[500,668],[420,667],[370,667],[368,666]]` for an amount at about `(368, 666)`. Then take a screenshot. It shows `Balance breakdown`, `Verify Savings 500.00 EUR`, and `Additional balance 200.00 EUR`. The drag may select some text along the path, which is harmless.
- **Check Left to Spend (`bucket-left-to-spend`).** On one screenshot, compare `LIQUID − (sum of bucket cards) = LEFT TO SPEND`. Use `TOTAL BALANCE` in place of `LIQUID` when no asset exists. On the dev DB on 2026-10-18 that was `1 850.00 − 700.00 = 1 150.00 EUR`. In demo mode on 2026-10-18 it was `12 851.84 − (2 860.00 + 8 441.84 + 450.00) = 1 100.00 EUR`.
- **Switch a bucket off (`bucket-switch`).** The switch is `AXCheckBox/AXSwitch "Count Verify Holiday in Left to Spend"`, 28×16, left of the card's `⋮`. A plain click toggles it. Keep a screenshot before and after: `LEFT TO SPEND` rises by the bucket's amount (on 2026-11-08: `1 093.00 → 1 893.00` with `Verify Holiday` at `800.00`), the amount turns grey with an `Off` pill, and the title, switch and `⋮` stay where they were. `evidence.sh db <nn>-switch "select name, bucket_kind, bucket_enabled from account where account_type = 'bucket'"` shows `0`. Click it again to switch it back on.
- **Breakdown (`bucket-switch-breakdown`).** With the bucket off, open `LEFT TO SPEND` by keyboard: click blank space above the metrics, then `tab` three times and `return`. Scroll the dialog. Under `NOT COUNTED` the bucket reads `Bucket total` and "Switched off on its card." with its real amount.
- **Past date (`bucket-switch-past`).** Pick a past day with the `Date` label and the calendar. Switching a bucket off there moves `LEFT TO SPEND` the same way. The switch has no date.

## Gotchas

- Last driven end to end on 2026-10-18, on the dev DB in run `20261018-213407-claude-keen-boyd-d370a7`: `bucket-create`, `bucket-extra`, `bucket-link` (keyboard path), `bucket-left-to-spend`, and `bucket-breakdown` (drag path). All five are verified, in the background, without display-scope tools.
- Run `20261108-195757-feat-income-budget-bucket` drove `bucket-switch` and `bucket-switch-breakdown` on `Verify Holiday`, and `bucket-switch-past` on the income budget card at 2026-10-15, in the background.
- The bucket's `UPDATE BALANCE` never shows the current extra amount. `Additional Balance` always starts at `0`, and saving with `0` sets the extra to 0 while keeping the links.
- Clicking an option in an open Radix select closes the list without choosing it. Choose with `down`/`up` and `return` instead.
- The `Select account to link` list offers only real accounts that no bucket links yet. Buckets and assets aren't listed. Once `Verify Savings` is linked to `Verify Holiday`, it can't be linked to a second bucket.
- Over-allocating an account is said to show an `Over-allocated` marker on the account card. That's useful as a negative check, but it hasn't been driven.
- `LEFT TO SPEND` only renders once at least one bucket exists, and `NET WORTH` and `LIQUID` replace `TOTAL BALANCE` only once an asset exists.
