# Ledger filters and View in ledger

The Ledger page lists every event and narrows the list by date range, by account, bucket or asset, and by event type. A dashboard card's menu opens the Ledger already filtered to that card's account. `Clear filters` goes back to the full list, and a filter that matches nothing says so.

## Sub-features

- `ledger-type-chips` narrows the list with the type chips, which wrap onto a second line instead of running off the page.
- `ledger-clear` shows `Clear filters` while any ledger filter is set and resets all of them.
- `ledger-empty-filtered` shows `No events match these filters.` when the filters leave nothing.
- `ledger-view-from-card` opens the Ledger filtered to one account from its dashboard card.
- `ledger-split-toggle` opens a split entry's legs from the toggle under its card.

## How to get to it (user POV)

- Sidebar → `Ledger` → the `From` and `To` date pickers, the `Portfolio Item` filter, and the row of type chips (`All`, `Balance Updates`, `Cashflows`, `Transfers`, `Revenue`, `Expenses`, `Prepaid Expenses`, `Unmatched cashflows (n)`).
- Dashboard → a card's `⋮` menu → `View in ledger`. An income budget card has no such item.
- Ledger → a row with a `Split ×n` badge → the round chevron on the card's bottom edge.

## Driving it with computer use

Preconditions:

- Baseline from the [README](./README.md). Demo mode is enough for every sub-feature except `ledger-split-toggle`, which needs a split entry (the dev DB had one on `Verify Checking`, 24 October 2026).

- **Type chips (`ledger-type-chips`, `ledger-empty-filtered`).** Open `AXLink "Ledger"`. In a 1200-px-wide window `Unmatched cashflows (n)` sits on a second chip line. Click `AXButton "Transfers"`. In demo mode the list is replaced by `No events match these filters.`, and `AXButton "Clear filters"` appears right of `Portfolio Item`.
- **Clear (`ledger-clear`).** Click `AXButton "Clear filters"`. The `All` chip is active again, the full list is back, and the button is gone.
- **From a card (`ledger-view-from-card`).** On the Dashboard, click the title of an account card, then `app_key` `tab`, `return`, `down`, `return`. The menu lists `Edit`, `View in ledger`, `Manage Linked Assets`, `Delete`. The Ledger opens with the account's name on the `Portfolio Item` button, only that account's rows, `Clear filters` shown, and the sidebar's `Ledger` link highlighted.
- **Split toggle (`ledger-split-toggle`).** On the Ledger, click `AXButton "Cashflows"` and scroll to the `Split ×3` row. The chevron sits on the card's bottom edge at its horizontal center, with the pointer anywhere. Click it. Rows `Split 1`, `Split 2`, … open under the card.

## Gotchas

- Last driven end to end on 2026-11-13, run `20261113-164107-ux-quick-wins`: `ledger-type-chips`, `ledger-clear`, `ledger-empty-filtered` and `ledger-view-from-card` (the `Credit Card` account) in demo mode, `ledger-split-toggle` on the dev DB. `ledger-view-from-card` wasn't driven for a bucket or an asset.
- The header's person filter is not a ledger filter. `Clear filters` leaves it alone, and changing it clears the account selection.
- With exactly one account selected, the `Portfolio Item` button reads that account's name. With several it reads `Portfolio Item (n)`.
- A new page opens at the top. If a page seems to open scrolled, take a second screenshot before you conclude anything: a background window can keep painting an old frame.
