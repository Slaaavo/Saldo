# Buckets

Buckets earmark money that sits in real accounts, such as an emergency fund or holiday savings. A bucket's balance is its own additional balance plus the balances of the accounts linked to it. Left to Spend is Liquid minus all bucket balances, so allocating to a bucket lowers Left to Spend without moving money.

## Sub-features

- `bucket-create` adds a bucket from the Buckets section.
- `bucket-extra` sets a bucket's additional balance with `UPDATE BALANCE`.
- `bucket-link` links an account to a bucket so the bucket mirrors that account's balance.
- `bucket-left-to-spend` shows the `LEFT TO SPEND` metric once a bucket exists, equal to Liquid minus buckets.
- `bucket-breakdown` shows the balance breakdown tooltip (additional balance, linked accounts, tagged cashflows) on a bucket card.

## How to get to it (user POV)

- Dashboard → Buckets → `Add Bucket`.
- Dashboard → a bucket card → `UPDATE BALANCE`, with its `Linked Accounts` section and `Link Account`.
- Dashboard → hero metrics → `LEFT TO SPEND`.

## Driving it with computer use

Preconditions:

- Baseline from the [README](./README.md). On the dev DB, an account such as `Verify Checking` already has a balance (see [Balance update](./balance-update.md)). In demo mode, the seeded buckets `Emergency Fund`, `Retirement Fund` and `Holiday Savings` exist.

- **Create a bucket.** On the dashboard, click `AXButton "Add Bucket"`. A `Create Bucket` dialog opens. Click `AXTextField "Bucket Name"`, type `Verify Holiday`, and click `AXButton "Create"`. A `Verify Holiday` card appears under Buckets, and `LEFT TO SPEND` appears in the hero metrics.
- **Allocate an additional balance.** Click the bucket card's `AXButton "UPDATE BALANCE"`. The dialog's amount field is named `Additional Balance`. Click it, type `200`, and click `AXButton "Save"`. The bucket card reads `200.00 EUR`, and `LEFT TO SPEND` drops by `200.00` against `LIQUID`. Keep before and after screenshots.
- **Stored row.** Run `evidence.sh db <nn>-bucket-row "select a.name, a.account_type, d.amount_minor, d.event_date from account a join event e on e.account_id = a.id join event_data d on d.id = e.latest_data_id where a.name = 'Verify Holiday' and e.deleted_at is null"`. It shows `bucket` and `20000`.
- **Link an account (`bucket-link`).** In the same dialog, click `AXButton "Link Account"`. A row appears with a `Select account to link` pop-up. That select refuses background clicks. Ask the user before using display-scope tools to pick the account. After saving, the bucket's balance includes the linked account's balance.
- **Check Left to Spend.** Compare `LIQUID − (sum of bucket cards) = LEFT TO SPEND` on one screenshot. In demo mode on 2026-10-18 that was `12 851.84 − (2 860.00 + 8 441.84 + 450.00) = 1 100.00 EUR`.

## Gotchas

- Last driven: nothing in this file has been driven end to end yet. On 2026-10-18 the run read the demo seed's bucket cards and the hero metrics, which confirms `bucket-left-to-spend` by arithmetic only.
- The `Create Bucket` dialog reuses the account dialog. Its name field is `Bucket Name`.
- `Link Account` and its select are pop-up driven. Plan for display-scope tools or a keyboard path, and record the step as not verified if neither is available.
- Over-allocating an account shows an `Over-allocated` marker on the account card, which is a useful negative check.
- `LEFT TO SPEND` only renders once at least one bucket exists, and `NET WORTH` and `LIQUID` replace `TOTAL BALANCE` only once an asset exists.
