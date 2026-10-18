#!/usr/bin/env bash
# Writes proof into this run's evidence folder, ~/Library/Logs/verify-saldo/<run-id>/,
# which cleanup.sh never deletes. Run with the sandbox disabled, because ~/Library/Logs
# is off limits inside it.
#
#   evidence.sh dir                     print the evidence folder
#   evidence.sh keep <name> <image>     copy an app_screenshot image to <name>.<ext>
#   evidence.sh db <name> <sql>         run read-only SQL on the dev DB, save to <name>.txt
#   evidence.sh note <text>             append a timestamped line to notes.md
set -euo pipefail
. "$(dirname "$0")/lib.sh"

load_run_env || { echo "[verify-saldo] no $RUN_ENV; start a run with launch.sh first" >&2; exit 3; }
mkdir -p "$EVIDENCE"

case "${1:-}" in
  dir)
    echo "$EVIDENCE"
    ;;
  keep)
    name="${2:?usage: evidence.sh keep <name> <image>}"
    image="${3:?usage: evidence.sh keep <name> <image>}"
    [ -s "$image" ] || { echo "[verify-saldo] no image at $image" >&2; exit 3; }
    cp "$image" "$EVIDENCE/$name.${image##*.}"
    echo "$EVIDENCE/$name.${image##*.}"
    ;;
  db)
    name="${2:?usage: evidence.sh db <name> <sql>}"
    sql="${3:?usage: evidence.sh db <name> <sql>}"
    { echo "-- $DEV_DB (read-only)"; echo "-- $sql"; sqlite3 -header -column "file:$DEV_DB?mode=ro" "$sql"; } > "$EVIDENCE/$name.txt"
    cat "$EVIDENCE/$name.txt"
    echo "$EVIDENCE/$name.txt"
    ;;
  note)
    shift
    echo "- $(date +%H:%M:%S) $*" >> "$EVIDENCE/notes.md"
    echo "$EVIDENCE/notes.md"
    ;;
  *)
    sed -n '2,10p' "$0"
    exit 2
    ;;
esac
