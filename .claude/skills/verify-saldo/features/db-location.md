# Database location

Settings → Data Storage shows the current database file and lets the user move the database to another folder, switch to a database that already exists there, start a fresh one, or reset to the default location. Every successful change reloads the window, so no data, filter or theme from the other database survives on screen.

## Sub-features

- `dbloc-move` copies the current database into an empty folder, switches to it, and deletes the old files.
- `dbloc-switch` opens an existing `saldo.db` in the chosen folder.
- `dbloc-fresh` creates a new empty database in the chosen folder.
- `dbloc-reset` returns to the default location, with the same move/fresh choice when no database is there.

## How to get to it (user POV)

- Sidebar → `Settings` → `Data Storage` → `Change...` (folder picker), or `Reset to Default` when a custom location is active.
- Disabled in demo mode.

## Driving it with computer use

Preconditions:

- Baseline from the [README](./README.md), on the dev DB.
- Display-scope control approved by the user for the native folder panel.
- An empty target folder under `$TMPDIR` or the session scratchpad. Never point it at the real Saldo folder.

- **Record the data.** Run `evidence.sh db <nn>-before "select (select count(*) from event), (select count(*) from event_data), (select count(*) from account), (select max(id) from event)"`.
- **Move (`dbloc-move`).** `Settings`, `End` to scroll, `Change...`. In the panel press `cmd+shift+g`, type the empty folder's path, `return`, `Open`. The `Database not found` dialog offers `Cancel`, `Move current database` and `Start fresh`. Click `Move current database`. The window reloads onto the Dashboard and the person filter is back on `All Persons`.
- **Check it.** `Settings` shows the new path at once, with `Reset to Default`. On disk the folder holds `saldo.db` (plus `-wal`/`-shm` while open) and no `saldo.db.tmp`; the old dev `saldo.db`, `-wal` and `-shm` are gone; `db-location.json` in the dev app-data folder names the new folder. The counts query against the new file matches the one before.
- **Reset (`dbloc-reset`).** Click `Reset to Default`, then `Move current database`. The window reloads; Settings shows the default dev path marked `(default)`; `db-location.json` is gone and the target folder is empty. `doctor.sh` is green again.

## Gotchas

- Last driven on 2026-10-25, on the dev DB, in run `20261025-081534-worktree-open-review-fixes`: `dbloc-move` and `dbloc-reset` with the move choice. `dbloc-switch` and `dbloc-fresh` were not driven.
- While the database lives in the custom folder, `doctor.sh` reports the DB outside the dev folder. Reset before ending the run, or the next run of any worktree opens that folder.
- No success toast shows after a change, because the window reloads.
