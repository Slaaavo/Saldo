#!/usr/bin/env bash
# Read-only check: is there a Saldo Dev instance from THIS worktree worth driving?
# Exit 0 = yes. Exit 1 = no, and the FAIL lines say why. Changes nothing.
set -uo pipefail
. "$(dirname "$0")/lib.sh"

healthy=1
say() { printf '%-5s %s\n' "$1" "$2"; }
fail() {
  say FAIL "$1"
  healthy=0
}

say info "worktree $W ($BRANCH @ $(git -C "$W" rev-parse --short HEAD))"
if load_run_env; then
  say info "run $RUN_ID, evidence folder $EVIDENCE"
else
  say info "no run.env: launch.sh has not started a run in this worktree"
fi

vite="$(port_pid || true)"
if [ -z "$vite" ]; then
  fail "nothing listens on :$PORT (Vite is down)"
else
  cwd="$(pid_cwd "$vite")"
  case "$cwd" in
    "$W" | "$W"/*) say ok "Vite on :$PORT belongs to this worktree (pid $vite)" ;;
    *) fail "port $PORT belongs to pid $vite in $cwd, another run. Don't drive it or stop it" ;;
  esac
fi

app="$(app_pid || true)"
if [ -z "$app" ]; then
  fail "Saldo Dev from this worktree is not running"
else
  say ok "Saldo Dev pid $app, bundle id $DEV_ID"
  dbs="$(lsof -nP -p "$app" -Fn 2> /dev/null | sed -n 's/^n//p' | grep -E '/saldo\.db$' || true)"
  if [ -z "$dbs" ]; then
    fail "Saldo Dev has no saldo.db open yet (startup still running?)"
  fi
  while IFS= read -r db; do
    [ -n "$db" ] || continue
    case "$db" in
      "$DEV_DATA"/*) say ok "DB: $db" ;;
      *) fail "Saldo Dev holds a DB outside the dev folder: $db. Stop driving and run cleanup.sh" ;;
    esac
  done <<< "$dbs"
fi

lsof -nP -c saldo -a -d txt -Fpn 2> /dev/null | awk '/^p/{pid=substr($0,2)} /^n.*\/saldo$/{print pid "\t" substr($0,2)}' | sort -u |
  while IFS="$(printf '\t')" read -r pid exe; do
    case "$exe" in
      "$APP_BIN") ;;
      /Applications/*) say info "installed Saldo is running (pid $pid). It uses the real DB; leave it alone" ;;
      *) say info "another Saldo process (pid $pid): $exe. Not ours; leave it alone" ;;
    esac
  done

if [ -f "$DEV_DB" ]; then
  applied="$(dev_db_migrations | wc -l | tr -d ' ')"
  inrepo="$(repo_migrations | wc -l | tr -d ' ')"
  if [ "$applied" = "$inrepo" ]; then
    say ok "dev DB has all $inrepo migrations"
  else
    say warn "dev DB has $applied applied migrations, this branch has $inrepo"
  fi
fi

[ -f "$W/src-tauri/libpdfium.dylib" ] && say ok "libpdfium.dylib present" || say warn "libpdfium.dylib missing; eKasa import will fail"

if [ "$healthy" = 1 ]; then
  say ok "worth driving: computer-use app $DEV_ID"
  exit 0
fi
exit 1
