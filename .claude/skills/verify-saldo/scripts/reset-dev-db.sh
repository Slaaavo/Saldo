#!/usr/bin/env bash
# Moves the dev DB (saldo.db, -wal, -shm in the com.teamsumbal.saldo.dev folder) aside
# with a .bak-<YYYYMMDD-HHMMSS> suffix, so the next launch starts from a fresh, migrated
# DB. It never touches the real DB. Refuses while any process has the dev DB open.
# Usage: reset-dev-db.sh [reason]
set -euo pipefail
. "$(dirname "$0")/lib.sh"

[ -f "$DEV_DB" ] || { echo "[verify-saldo] no dev DB at $DEV_DB, nothing to reset"; exit 0; }
holders="$(lsof -nP -t "$DEV_DB" 2>/dev/null | tr '\n' ' ' || true)"
[ -z "$holders" ] || { echo "[verify-saldo] dev DB is open by pid(s) $holders; quit Saldo Dev first" >&2; exit 3; }

stamp="$(date +%Y%m%d-%H%M%S)"
for f in "$DEV_DB" "$DEV_DB-wal" "$DEV_DB-shm"; do
  [ -f "$f" ] && mv "$f" "$f.bak-$stamp"
done
echo "[verify-saldo] moved the dev DB aside as $DEV_DB.bak-$stamp${1:+ ($1)}"
