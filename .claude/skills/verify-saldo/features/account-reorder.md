# Account reorder

The user drags accounts into a new order in the Reorder Accounts dialog on the Dashboard. With the header's person filter set to one person, the dialog lists only that person's accounts, and saving swaps them among the positions they already hold in the full list, so other persons' accounts keep their places in the All Persons view.

## Sub-features

- `reorder-all` reorders every account with the person filter on All Persons.
- `reorder-person` reorders one person's accounts under the person filter without moving other persons' accounts.

## How to get to it (user POV)

- Dashboard → Accounts section → the up-down arrows button left of `Add Account` → `Reorder Accounts` dialog → drag → `Save Order`.
- The person filter (`All Persons` select in the header) appears once a second person exists. Persons are created on the Persons page.

## Driving it with computer use

Preconditions:

- Baseline from the [README](./README.md), on the dev DB.
- Two persons, each with at least two accounts. Create `Verify B` on Persons (`Add Person`, type `Name`, `Create Person`), then with the header filter on `Verify B` add `Verify B1` and `Verify B2` through `Add Account` (the `Person` select is preset by the filter).

- **Record the slots.** Run `evidence.sh db <nn>-sort-before "select a.id, a.name, p.name as person, a.sort_order from account a left join person p on p.id = a.person_id where a.account_type = 'account' order by a.sort_order, a.id"`.
- **Filter to one person.** Click blank space in the Dashboard title row, then `app_key` `tab`, `down`, `down` per person below `All Persons`, and `return`. The select reads `Verify B`, and only that person's cards show.
- **Reorder (`reorder-person`).** Click the up-down arrows button. `Reorder Accounts` lists the person's accounts, each with a `drag handle`. Drag the second handle above the first with `app_drag` along a `path` of about eight points, moving a few pixels at a time. The list shows the new order. Click `Save Order`.
- **Check the full list.** Switch the filter back to `All Persons` (`tab`, `down`, `up` to the top, `return`). The other person's accounts sit where they were, and the reordered accounts trade places inside their own positions. Rerun the `sort-before` query as `sort-after`: only the two reordered accounts changed `sort_order`, and they swapped values.

## Gotchas

- Last driven end to end on 2026-10-25, on the dev DB, run `20261025-081534-worktree-open-review-fixes`: `reorder-person` with `Verify B2` dragged above `Verify B1`. `reorder-all` wasn't driven.
- A drag with only two points doesn't move the item, because the list starts a drag only after the pointer travels a few pixels.
- Before the fix, saving under a filter wrote positions 0..n for that person's accounts, which collided with other persons' accounts and moved them in the All Persons view.
