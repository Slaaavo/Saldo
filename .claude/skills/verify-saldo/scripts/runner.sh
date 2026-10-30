#!/usr/bin/env bash
# `tauri dev --runner` target, started by launch.sh. Builds what `cargo run` would build,
# then starts that binary from inside "Saldo Dev.app". The plain debug binary has no
# bundle id, so computer use cannot see its window; inside the bundle it is
# com.teamsumbal.saldo.dev. tauri dev re-runs this on every Rust change.
set -euo pipefail
. "$(dirname "$0")/lib.sh"

[ "${1:-}" = run ] || { echo "[verify-saldo] runner expects 'run ...', got: $*" >&2; exit 2; }
shift
cargo_args=()
app_args=()
while [ $# -gt 0 ]; do
  if [ "$1" = -- ]; then
    shift
    app_args=("$@")
    break
  fi
  cargo_args+=("$1")
  shift
done

cargo build ${cargo_args[@]+"${cargo_args[@]}"}

profile=debug
for arg in ${cargo_args[@]+"${cargo_args[@]}"}; do
  [ "$arg" = --release ] && profile=release
done

mkdir -p "$BUNDLE/Contents/MacOS"
cat > "$BUNDLE/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleIdentifier</key>
	<string>$DEV_ID</string>
	<key>CFBundleName</key>
	<string>Saldo Dev</string>
	<key>CFBundleExecutable</key>
	<string>saldo</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
EOF
cp -f "${CARGO_TARGET_DIR:-$W/src-tauri/target}/$profile/saldo" "$APP_BIN"
# The installed app carries PDFium in Contents/Resources, which `saldo mcp` finds from the
# binary's path whatever its working directory.
mkdir -p "$BUNDLE/Contents/Resources"
cp -f "$W/src-tauri/libpdfium.dylib" "$BUNDLE/Contents/Resources/libpdfium.dylib"

echo "[verify-saldo] starting Saldo Dev (pid $$, bundle id $DEV_ID)"
cd "$W/src-tauri"
exec "$APP_BIN" ${app_args[@]+"${app_args[@]}"}
