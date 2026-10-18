---
name: verify-saldo
description: Launch the Saldo desktop app (Tauri 2, macOS) as an isolated "Saldo Dev" build on its own database, drive its window with computer use the way a user does, and save evidence. Use it to prove a Saldo change works in the running app (slavo-mode-mini's Done), or when asked to run, check, or screenshot Saldo.
---

# Verify Saldo

This skill proves Saldo behavior in the real desktop window. The checks go through real Tauri IPC into the Rust backend and SQLite. Playwright, the built-in browser, and anything else that opens `http://localhost:1420` in a browser can't do this: on macOS, Tauri IPC is an in-process URL scheme, so a browser page never reaches Rust.

A verification run looks like this:

1. `launch.sh` builds the branch and starts it as **Saldo Dev**, with bundle id `com.teamsumbal.saldo.dev`, its own app-data folder, and its own `saldo.db`.
2. You drive the "Saldo" window of `com.teamsumbal.saldo.dev` with the computer-use `app_*` tools.
3. `evidence.sh` saves screenshots, DB rows, and notes to `~/Library/Logs/verify-saldo/<run-id>/`.
4. You quit the app, and `cleanup.sh` tears down what the run started and keeps the evidence.

Read the [feature map](features/README.md) and pick the feature file that matches the change before you drive anything.

## Rules

- **Never drive the installed app.** `/Applications/saldo.app` is `com.teamsumbal.saldo` and holds the user's real finances. Never request computer-use access to it, and never act on its windows. This skill only ever touches `com.teamsumbal.saldo.dev`.
- **One run per machine.** Vite holds port 1420 with `strictPort`, so only one `tauri dev` can run at a time. If the port is taken, another session owns it. Wait for it to free up, and never stop a run you didn't launch.
- **Kill by pid, never by name.** `cleanup.sh` stops only processes whose executable or working directory is inside this worktree.
- **Sandbox.** These helpers must run with `dangerouslyDisableSandbox: true`: `launch.sh` (it starts a GUI app and writes `~/Library`), `evidence.sh` and `cleanup.sh` (they write `~/Library/Logs` and send signals), and `reset-dev-db.sh` (it writes `~/Library/Application Support`). `doctor.sh` and `wait-ready.sh` are read-only and work either way.
- **The real DB is out of scope.** The dev app never opens the real `saldo.db` or follows the real `db-location.json`, because both belong to the `com.teamsumbal.saldo` folder. The backup rule in slavo-mode-mini applies only to plain `pnpm tauri dev`, which uses the real identifier, and this skill never runs that.

## Launch

**Set up a fresh worktree once.** Run `pnpm install --frozen-lockfile` with the sandbox disabled. Inside the sandbox, pnpm 12 rejects the proxy's TLS certificate (`OSStatus -26276`) and can leave `node_modules` empty. `launch.sh` copies the gitignored `src-tauri/libpdfium.dylib` from the main checkout when it is missing.

**Start the app.** Use the Bash tool with `run_in_background: true` and `dangerouslyDisableSandbox: true`. Keep the task id, because stopping that task stops the whole run.

```bash
.claude/skills/verify-saldo/scripts/launch.sh
```

`launch.sh` refuses (exit 3) when port 1420 is taken, when this worktree's Saldo Dev is already up, or when `.verify-saldo/run.env` is left from an earlier run. Read its task output for the reason. To wait for another session's run to end, start this as a background Bash task. You get a notification when it exits.

```bash
until ! lsof -nP -iTCP:1420 -sTCP:LISTEN > /dev/null; do sleep 10; done
```

**Wait until it's ready.** Run this in the foreground with a 600000 ms tool timeout:

```bash
.claude/skills/verify-saldo/scripts/wait-ready.sh 600
```

Exit 0 prints `ready: Saldo Dev pid <pid>` and the evidence folder. Exit 1 means the build or launch failed, and the script prints the log tail. Exit 2 means it timed out. The first build in a fresh worktree compiles the Rust backend: about 40 s with a warm cargo cache, several minutes cold. The full log is in `.verify-saldo/tauri-dev.log`.

How it works: `launch.sh` runs `pnpm tauri dev --config src-tauri/tauri.dev.conf.json --runner scripts/runner.sh`.

- `tauri.dev.conf.json` sets the identifier to `com.teamsumbal.saldo.dev`.
- The plain debug binary has no bundle id, so computer use can't see its window. `runner.sh` therefore builds what `cargo run` would build, copies the binary into `.verify-saldo/Saldo Dev.app`, and execs it from there with `src-tauri/` as the working directory. eKasa import loads `./libpdfium.dylib` from that directory.
- Vite and teardown on quit work as in normal `tauri dev`. Frontend edits hot-reload in the open window. A Rust edit should make `tauri dev` re-run the runner, which rebuilds and restarts the app; this hasn't been exercised yet. After one, run `wait-ready.sh` and take a fresh screenshot.

**Grant computer use once per session.** Call `request_access` with `apps: ["com.teamsumbal.saldo.dev"]`. The user approves the dialog. The app is only listed while it runs, so launch it first.

## Doctor

Run this first, and again whenever anything looks off. It is read-only.

```bash
.claude/skills/verify-saldo/scripts/doctor.sh
```

Exit 0 ends with `worth driving: computer-use app com.teamsumbal.saldo.dev`. It confirms that Vite on :1420 belongs to this worktree, that Saldo Dev is running from this worktree's bundle, and that the app holds only DB files under `~/Library/Application Support/com.teamsumbal.saldo.dev/`. It also reports dev DB migrations against `migrations/`, `libpdfium.dylib`, and any other Saldo processes it sees (the installed app, other worktrees), which you leave alone. A `FAIL` line names the problem. A DB outside the dev folder means stop driving and run cleanup.

## Drive

Use the background `app_*` tools on app `com.teamsumbal.saldo.dev`. The window is titled "Saldo" (find it with `app_list_windows`). Every `app_screenshot` returns an AX summary in which each line reads `[index] Role [x,y w×h] "name"`, in the full-resolution coordinate frame. Accessible names are the English UI strings in `src/i18n/locales/en.yml`, so prefer them over guessed positions.

These are the rules that worked on 2026-10-18:

- **Click by coordinate at the center of the element's AX bounds.** An `element_index` click on web content is refused ("belongs to a different page ... AXWebArea"). For `[716,446 122×40] "Add Account"`, click `[777, 466]`.
- **Type by clicking the field first**, then `app_type` with no `element_index` or coordinate. Typing at an `element_index` is refused. Batch the steps: `click field → type → click next field → type → screenshot`. Inputs start empty. The `0` in amount fields is a placeholder.
- **Pop-up controls are refused in the background.** That covers every `AXPopUpButton` and `AXComboBox`: date pickers, Radix selects, the Ledger `Import` menu, and the `⋮` menu on cards. Click the control's visible `<label>` instead. Clicking the "Date" label next to the dashboard date picker opens its calendar. Inside an open popover, buttons such as month arrows and day cells click normally. A select whose value is already right (for example, the account in Update Balance opened from a card) needs no click. When no label works, the step needs the display-scope `computer_batch` tools. Those take over the user's screen and need their separate approval, so ask first.
- **Popovers can paint late.** If the AX element count jumps but the screenshot shows nothing new, take another screenshot. While a calendar is open, the AX walk is truncated, so use screenshot coordinates.
- **Navigate with the sidebar `AXLink`s** (Dashboard, Ledger, Persons, Tax Models, Partners, Import Profiles, Settings, FX Rates, Units). The page title in the header confirms where you are. The sidebar highlight can stay on Dashboard in a background window. Don't treat it as proof of navigation.
- **Demo mode moves everything down.** Its amber banner (`DEMO MODE` … `Exit Demo`) shifts the layout about 53 px. Take a fresh screenshot before any coordinate click after entering or leaving demo mode.
- **Scroll** with `app_scroll` at a point inside the page, `dy` 5 to 20.
- **Quit** with `app_menu` path `["Saldo Dev", "Quit Saldo Dev"]`.

**Data modes.** Pick one per check and say which in the evidence.

- **Dev DB (default):** `~/Library/Application Support/com.teamsumbal.saldo.dev/saldo.db`. It is persistent, disposable, and shared by every worktree's runs. A fresh one shows "No accounts yet" with `Try Demo Mode` and `Add Account`. Mutations land on disk, so `evidence.sh db` can prove them. `launch.sh` sets the DB aside when another branch applied migrations this branch lacks. Run `reset-dev-db.sh` while the app is down to start from a fresh DB.
- **Demo mode:** a seeded, in-memory DB. Enter it with `Try Demo Mode` on an empty dashboard, or with Settings → Demo Mode → `Start Demo Mode`. It has accounts, buckets, an asset and BTC rates for the last days of the three previous months. The seed is in `src-tauri/src/demo.rs`. Leave with `Exit Demo`, which discards everything. Its rows never reach disk, so the proof is a second view inside the app.
- Settings → Data Storage → "Current database file" shows `…/com.teamsumbal.saldo.dev/saldo.db`. That is the in-app proof that a run is isolated.

## Evidence

Everything goes to `~/Library/Logs/verify-saldo/<run-id>/`. `<run-id>` is `<YYYYMMDD-HHMMSS>-<branch>`. `wait-ready.sh` and `doctor.sh` print the folder, and `evidence.sh dir` repeats it. The folder survives cleanup and worktree removal. Run every `evidence.sh` call with the sandbox disabled.

```bash
.claude/skills/verify-saldo/scripts/evidence.sh keep 03-after-save "<image path>"
.claude/skills/verify-saldo/scripts/evidence.sh db 04-balance-row "select ... from ..."
.claude/skills/verify-saldo/scripts/evidence.sh note "balance-update: saved 1250.50 from the dashboard card"
```

- **Screenshots.** For a proof screenshot, call `app_screenshot` with `scale: 1`. Its result includes `[Image: source: <path>.jpg]`. Pass that path to `evidence.sh keep`. `screencapture` can't capture the window, because the Claude process has no Screen Recording permission.
- **DB rows.** `evidence.sh db` runs read-only SQL against the dev DB and saves the query with its output. Read events through `event.latest_data_id`, and filter `deleted_at IS NULL`. Money is integer minor units.
- **Notes.** Add one `note` per step: the feature ID, the entry point, the action, and the result you saw.
- `cleanup.sh` adds `tauri-dev.log`.

Proof standards:

- Exercise the real user path: clicks and typing in the window. Never call `invoke`, internal setters, or test-only endpoints.
- Capture the action and the resulting state: before, the filled form, and after. The final screen alone isn't enough.
- Verify side effects alongside what's visible. On the dev DB, query the rows. In demo mode, open a second view, such as the Ledger page after a dashboard edit.
- A path you couldn't drive is reported as not verified, with the step that blocked it. Never report it as verified through a different entry point.
- Every external system here is real: OXR FX fetches go to the network and use the user's quota. The dev DB only fetches when an `oxr_app_id` is saved in it, and a fresh one has none. Set a key only when the check is about fetching.

## Cleanup

1. Quit with `app_menu` `["Saldo Dev", "Quit Saldo Dev"]`. `tauri dev` then exits and takes Vite with it, and the launch task finishes with exit 0.
2. Run this with the sandbox disabled:

   ```bash
   .claude/skills/verify-saldo/scripts/cleanup.sh
   ```

   It stops what is still running from this worktree, by pid. It copies the log into the evidence folder, removes `.verify-saldo/` run state, verifies that port 1420 and the bundle are free, lists the evidence it kept, and prints `clean`.
3. If the app hangs, `cleanup.sh` alone also works, and so does `TaskStop` on the launch task.

Run cleanup after every failed attempt too, so a broken run doesn't hold port 1420 or block the next launch. Cleanup never deletes the evidence folder or the dev DB.

## Helpers

All of these live in `.claude/skills/verify-saldo/scripts/` and run from any directory in the worktree.

| Helper | Invocation | Sandbox |
|---|---|---|
| `launch.sh` | background task: `launch.sh` | off |
| `wait-ready.sh` | `wait-ready.sh [timeout-s]` | either |
| `doctor.sh` | `doctor.sh` | either |
| `evidence.sh` | `evidence.sh dir`, `keep <name> <image>`, `db <name> <sql>`, `note <text>` | off |
| `cleanup.sh` | `cleanup.sh` | off |
| `reset-dev-db.sh` | `reset-dev-db.sh [reason]` (app must be down) | off |
| `runner.sh` | started by `launch.sh` through `tauri dev --runner`; don't run it by hand | n/a |
| `lib.sh` | sourced by the others | n/a |
