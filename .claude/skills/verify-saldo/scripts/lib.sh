# Shared paths and probes for the verify-saldo helpers. Source it, don't run it.

SKILL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPTS="$SKILL_DIR/scripts"
W="$(git -C "$SKILL_DIR" rev-parse --show-toplevel)"
MAIN_CHECKOUT="$(git -C "$W" worktree list --porcelain | awk '/^worktree /{print substr($0, 10); exit}')"
BRANCH="$(git -C "$W" rev-parse --abbrev-ref HEAD)"

STATE="$W/.verify-saldo"
RUN_ENV="$STATE/run.env"
LOG="$STATE/tauri-dev.log"
BUNDLE="$STATE/Saldo Dev.app"
APP_BIN="$BUNDLE/Contents/MacOS/saldo"

DEV_CONFIG="$W/src-tauri/tauri.dev.conf.json"
DEV_ID="$(/usr/bin/plutil -extract identifier raw -o - "$DEV_CONFIG")"
DEV_DATA="$HOME/Library/Application Support/$DEV_ID"
DEV_DB="$DEV_DATA/saldo.db"
REAL_DATA="$HOME/Library/Application Support/com.teamsumbal.saldo"

EVIDENCE_ROOT="$HOME/Library/Logs/verify-saldo"
PORT=1420

port_pid() { lsof -nP -t -iTCP:"$PORT" -sTCP:LISTEN 2>/dev/null | head -1; }
pid_cwd() { lsof -nP -a -p "$1" -d cwd -Fn 2>/dev/null | sed -n 's/^n//p' | head -1; }
app_pid() { [ -e "$APP_BIN" ] && lsof -nP -t "$APP_BIN" 2>/dev/null | head -1; }
strip_ansi() { sed $'s/\x1b\\[[0-9;?]*[A-Za-z]//g'; }
dev_db_migrations() { sqlite3 "file:$DEV_DB?mode=ro" "select filename from _migrations order by filename" 2>/dev/null; }
repo_migrations() { ls "$W/migrations" | grep '\.sql$' | sort; }
load_run_env() { [ -f "$RUN_ENV" ] && . "$RUN_ENV"; }
