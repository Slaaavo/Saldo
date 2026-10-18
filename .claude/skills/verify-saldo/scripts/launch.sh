#!/usr/bin/env bash
# Starts Saldo for verification: Vite on :1420 plus the Rust backend, running as
# "Saldo Dev" (com.teamsumbal.saldo.dev) on its own app-data folder, never the real DB.
# Stays in the foreground until the app quits. Start it as a background task with the
# sandbox disabled; its output goes to .verify-saldo/tauri-dev.log.
set -euo pipefail
. "$(dirname "$0")/lib.sh"

refuse() {
  echo "[verify-saldo] launch refused: $*" >&2
  exit 3
}

held_by="$(port_pid || true)"
[ -z "$held_by" ] || refuse "port $PORT is held by pid $held_by (cwd $(pid_cwd "$held_by")). Another run owns it. Wait for it to end; never stop it."
running="$(app_pid || true)"
[ -z "$running" ] || refuse "Saldo Dev from this worktree is already running (pid $running). Run cleanup.sh first."
[ ! -f "$RUN_ENV" ] || refuse "$RUN_ENV is left from an earlier run. Run cleanup.sh first."
[ -d "$W/node_modules/.pnpm" ] || refuse "node_modules is missing. Run 'pnpm install --frozen-lockfile' in $W with the sandbox disabled."

if [ ! -f "$W/src-tauri/libpdfium.dylib" ]; then
  cp -p "$MAIN_CHECKOUT/src-tauri/libpdfium.dylib" "$W/src-tauri/libpdfium.dylib"
  echo "[verify-saldo] copied libpdfium.dylib from $MAIN_CHECKOUT"
fi

mkdir -p "$STATE"
# Every worktree shares the dev DB. When another branch applied migrations this branch
# doesn't have, set the DB aside; the app creates a fresh one on startup.
if [ -f "$DEV_DB" ]; then
  dev_db_migrations > "$STATE/applied.txt"
  repo_migrations > "$STATE/repo.txt"
  foreign="$(comm -23 "$STATE/applied.txt" "$STATE/repo.txt" | tr '\n' ' ')"
  rm -f "$STATE/applied.txt" "$STATE/repo.txt"
  [ -z "$foreign" ] || "$SCRIPTS/reset-dev-db.sh" "it has migrations this branch lacks: $foreign"
fi

run_id="$(date +%Y%m%d-%H%M%S)-$(echo "$BRANCH" | tr '/' '-')"
evidence="$EVIDENCE_ROOT/$run_id"
mkdir -p "$evidence"
cat > "$RUN_ENV" <<EOF
RUN_ID='$run_id'
EVIDENCE='$evidence'
LAUNCH_PID='$$'
EOF
echo "[verify-saldo] run $run_id, evidence in $evidence, log in $LOG"

exec > "$LOG" 2>&1
echo "[verify-saldo] run $run_id on $BRANCH @ $(git -C "$W" rev-parse --short HEAD)"
cd "$W"
exec pnpm tauri dev --config src-tauri/tauri.dev.conf.json --runner "$SCRIPTS/runner.sh"
