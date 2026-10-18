#!/usr/bin/env bash
# Tears down the run launch.sh started from THIS worktree and keeps the evidence. It stops
# processes by pid, and only those whose executable or working directory is inside this
# worktree. It never kills by name and never touches another run or the installed app.
# It copies the launch log into the evidence folder, then removes the run state.
# Exit 0 when clean.
set -uo pipefail
. "$(dirname "$0")/lib.sh"

load_run_env || true
alive() { kill -0 "$1" 2> /dev/null; }
ours() {
  case "$(pid_cwd "$1")" in
    "$W" | "$W"/*) return 0 ;;
    *) return 1 ;;
  esac
}
stop() {
  local pid=$1 what=$2
  alive "$pid" || return 0
  kill -TERM "$pid" 2> /dev/null
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    alive "$pid" || break
    sleep 0.5
  done
  alive "$pid" && kill -KILL "$pid" 2> /dev/null
  echo "[verify-saldo] stopped $what (pid $pid)"
}

app="$(app_pid || true)"
[ -n "$app" ] && stop "$app" "Saldo Dev"
# tauri dev exits once the app is gone and takes Vite with it. Stop what is left of ours.
sleep 1
vite="$(port_pid || true)"
if [ -n "$vite" ]; then
  if ours "$vite"; then
    stop "$vite" "Vite on :$PORT"
  else
    echo "[verify-saldo] port $PORT is held by pid $vite in $(pid_cwd "$vite"), not ours; left alone"
  fi
fi
if [ -n "${LAUNCH_PID:-}" ] && alive "$LAUNCH_PID" && ours "$LAUNCH_PID"; then
  stop "$LAUNCH_PID" "tauri dev"
fi

if [ -n "${EVIDENCE:-}" ]; then
  mkdir -p "$EVIDENCE"
  [ -f "$LOG" ] && strip_ansi < "$LOG" > "$EVIDENCE/tauri-dev.log"
fi
rm -rf "$BUNDLE"
rm -f "$LOG" "$RUN_ENV"

clean=1
[ -z "$(app_pid || true)" ] || { echo "[verify-saldo] Saldo Dev is still running" >&2; clean=0; }
vite="$(port_pid || true)"
if [ -n "$vite" ] && ours "$vite"; then
  echo "[verify-saldo] our Vite still holds :$PORT" >&2
  clean=0
fi
if [ -n "${EVIDENCE:-}" ]; then
  echo "[verify-saldo] evidence kept in $EVIDENCE:"
  ls -1 "$EVIDENCE"
fi
[ "$clean" = 1 ] && echo "[verify-saldo] clean" && exit 0
exit 1
