#!/usr/bin/env bash
# Blocks until the run that launch.sh started is ready to drive, or fails fast.
# Ready means the runner started Saldo Dev and that process holds the dev DB open, which
# happens after startup migrations. Exit 0 ready, 1 failed (prints the log tail),
# 2 timed out. Usage: wait-ready.sh [timeout-seconds, default 900]
set -uo pipefail
. "$(dirname "$0")/lib.sh"

deadline=$(($(date +%s) + ${1:-900}))
alive() { lsof -nP -p "$1" > /dev/null 2>&1; }

while :; do
  if load_run_env; then
    if strip_ansi < "$LOG" 2> /dev/null | grep -a -E '^error(\[E[0-9]+\])?:|panicked at|ELIFECYCLE|is already in use' > /dev/null; then
      strip_ansi < "$LOG" | tail -40
      exit 1
    fi
    pid="$(app_pid || true)"
    if [ -n "$pid" ] && lsof -nP -p "$pid" -Fn 2> /dev/null | grep -xF "n$DEV_DB" > /dev/null; then
      echo "[verify-saldo] ready: Saldo Dev pid $pid, bundle id $DEV_ID, DB $DEV_DB"
      echo "[verify-saldo] evidence folder: $EVIDENCE"
      exit 0
    fi
    if [ -z "$pid" ] && ! alive "$LAUNCH_PID"; then
      echo "[verify-saldo] launch.sh (pid $LAUNCH_PID) exited before the app came up"
      strip_ansi < "$LOG" 2> /dev/null | tail -40
      exit 1
    fi
  fi
  if [ "$(date +%s)" -ge "$deadline" ]; then
    echo "[verify-saldo] not ready after ${1:-900}s"
    [ -f "$RUN_ENV" ] || echo "[verify-saldo] no $RUN_ENV: launch.sh refused or never ran; read its task output"
    strip_ansi < "$LOG" 2> /dev/null | tail -20
    exit 2
  fi
  sleep 2
done
