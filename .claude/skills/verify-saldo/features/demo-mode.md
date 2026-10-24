# Demo mode

Demo mode shows the app on a seeded, in-memory database without touching the user's data. Entering or leaving it swaps the whole database and reloads the window, so nothing from the other database stays on screen: no cached balances, persons, or person filter, and no theme picked during the demo.

## Sub-features

- `demo-enter` starts demo mode and reloads onto the Dashboard with the amber `DEMO MODE` banner and only demo data.
- `demo-persons` lists exactly one person, `Personal`, on the Persons page, with no error toast.
- `demo-exit` leaves demo mode and reloads onto the Dashboard with only the dev DB's data.
- `demo-theme` keeps a theme picked in demo mode out of the real session.

## How to get to it (user POV)

- Dashboard of an empty DB → `Try Demo Mode`.
- Settings → Demo Mode → `Start Demo Mode`.
- Amber banner → `Exit Demo`.

## Driving it with computer use

Preconditions:

- Baseline from the [README](./README.md), starting on the dev DB. Record its persons and accounts first with `evidence.sh db 00-persons "select id, name, is_default from person"` and `evidence.sh db 00-accounts "select id, name, account_type from account where account_type in ('account','bucket','asset')"`, so real names can be told apart from demo names.
- Note the dev DB's theme: `evidence.sh db 00-theme "select value from app_setting where key = 'theme'"` (no row means `system`).

- **Real data first.** Open `AXLink "Persons"` and keep a screenshot of the real persons. If the dev DB has more than one person, pick one in the Dashboard person select (keyboard path in `SKILL.md` → Drive).
- **Enter (`demo-enter`).** Settings → `AXButton "Start Demo Mode"`. The window reloads. Wait until the sidebar `AXLink`s are back, then take a fresh screenshot. The banner reads `DEMO MODE`, the Dashboard shows the demo accounts (`Checking Account`, `Credit Card`, `BTC Wallet`, …) and none of the dev DB's account names, and no person filter is applied.
- **Persons (`demo-persons`).** Open `AXLink "Persons"`. One row, `Personal`, shows. No error text appears at the window's bottom edge.
- **Theme (`demo-theme`).** Settings → Display → `Theme` → pick the theme (`Light`, `Dark` or `System`) that differs from the dev DB's.
- **Exit (`demo-exit`).** Banner → `AXButton "Exit Demo"`. The window reloads onto the Dashboard without the banner. The dev DB's accounts are back, Persons lists the real persons, and the theme is the dev DB's again (compare with the `00-theme` row).
- **Nothing reached disk.** Re-run the `00-persons` and `00-accounts` queries. The rows are unchanged.

## Gotchas

- Not yet driven end to end. Added on 2026-10-24 with the reload-on-swap change; the first run should record which sub-features it drove here.
- No success toast shows after `Exit Demo`, because the reload drops it. The proof is the banner disappearing and the real data coming back.
- A reload resets the router to the Dashboard. Navigate again after each swap instead of expecting to stay on Settings.
