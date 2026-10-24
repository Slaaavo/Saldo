# Saldo verification map

This folder is the maintained source for verifying what a Saldo user can do. Read the index, then use the matching feature file as the recipe. The harness, the launch, and the interaction rules are in [`../SKILL.md`](../SKILL.md).

## Baseline preconditions

- Saldo Dev was started with `launch.sh`, and `wait-ready.sh` returned 0.
- `doctor.sh` exits 0 and reports the DB under `~/Library/Application Support/com.teamsumbal.saldo.dev/`.
- Computer use is granted for `com.teamsumbal.saldo.dev`, and only that app. Never drive `com.teamsumbal.saldo`.
- The run states its data mode: the dev DB (persistent, checkable with `evidence.sh db`) or demo mode (seeded, in memory, discarded on `Exit Demo`).
- Only drive an instance this run started.

## Driving conventions

- Each recipe starts from the baseline unless its preconditions say otherwise.
- Find elements by AX role and accessible name from `app_screenshot`, then click the center of their bounds. Names are the English strings in `src/i18n/locales/en.yml`.
- Take a screenshot after every state change, and use its coordinates for the next click only.
- Click a field before typing into it.
- Pop-up controls (`AXPopUpButton`, `AXComboBox`) don't open from a background click. Click their visible label, use the keyboard path (focus just before the trigger, then `tab`, `down`, `return`), take an entry point whose select is preset, or ask the user before using display-scope tools. `SKILL.md` → Drive has the details.
- Mutations on the dev DB persist, so give fixture names a `Verify` prefix to keep them recognizable.

## Proof and skip reporting

- Save screenshots of the action and of the result: `app_screenshot` with `scale: 1`, then `evidence.sh keep`.
- For dev DB mutations, add an `evidence.sh db` query that shows the stored row.
- For demo-mode mutations, show a second view inside the app, such as the Ledger page or another date.
- Write an `evidence.sh note` for every step: the feature ID, the entry point, the action, and the observed result.
- Report an unreachable path with the attempted step and the unmet precondition. Don't report it as verified through another entry point.

## Feature entry contract

Each feature file starts with an H1 title and one paragraph describing the user-visible behavior. It then has exactly four H2 sections in this order:

1. `Sub-features` lists short IDs with one line for each behavior.
2. `How to get to it (user POV)` lists every user entry point.
3. `Driving it with computer use` starts with `Preconditions:`, then uses labeled bullets that pair each user action with a tool call and an observable result.
4. `Gotchas` lists traps that waste or invalidate a run. Its first bullet records which sub-features were last driven end to end, and when.

Keep implementation details out of the map. Name only user paths, stable handles, required state, tool calls, and observable proof.

## Features

- [Balance update](./balance-update.md): set an account's balance from its card, from Bulk Balance Update, or by editing a ledger row.
- [Dashboard snapshot by date](./dashboard-snapshot.md): metrics and card balances as of the date and the person picked in the header.
- [Buckets](./buckets.md): create a bucket, allocate to it, link an account, and read Left to Spend.
- [CSV import](./csv-import.md): import cashflows from a bank CSV through the import wizard.
- [FX rates](./fx-rates.md): list stored rates, edit one by hand, and fetch rates for a date.
- [Demo mode](./demo-mode.md): enter and leave the seeded demo database without real data, persons, or theme leaking across.
