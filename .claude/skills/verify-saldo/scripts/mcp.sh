#!/usr/bin/env bash
# Runs `saldo mcp` from this run's Saldo Dev bundle through a scripted MCP session and saves the
# transcript as evidence. Run with the sandbox disabled.
#
#   mcp.sh <name> <steps.json>     transcript goes to <evidence>/<name>.txt
set -euo pipefail
. "$(dirname "$0")/lib.sh"

load_run_env || { echo "[verify-saldo] no $RUN_ENV; start a run with launch.sh first" >&2; exit 3; }
name="${1:?usage: mcp.sh <name> <steps.json>}"
steps="${2:?usage: mcp.sh <name> <steps.json>}"
[ -x "$APP_BIN" ] || { echo "[verify-saldo] no Saldo Dev binary at $APP_BIN" >&2; exit 3; }
mkdir -p "$EVIDENCE"

set +e
python3 -I "$SCRIPTS/mcp-drive.py" "$steps" --expect-db "$DEV_DB" -- "$APP_BIN" mcp > "$EVIDENCE/$name.txt" 2>&1
code=$?
set -e
grep -E '"(isError|code|message|batchId|alreadyUndone|databasePath)"' "$EVIDENCE/$name.txt" || true
echo "[verify-saldo] exit $code, transcript $EVIDENCE/$name.txt"
exit $code
