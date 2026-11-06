#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -m)" != arm64 ]]; then
  echo 'AMStats solo se compila para Apple Silicon.' >&2; exit 1
fi
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release --locked
./scripts/sparkle.sh
app="$PWD/dist/AMStats.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$app/Contents/Frameworks"
cp target/aarch64-apple-darwin/release/amstats "$app/Contents/MacOS/amstats"
cp resources/Info.plist "$app/Contents/Info.plist"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml)
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $version" "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" "$app/Contents/Info.plist"
ditto target/sparkle/Sparkle.framework "$app/Contents/Frameworks/Sparkle.framework"
cp target/sparkle/LICENSE "$app/Contents/Resources/Sparkle-LICENSE.txt"
if [[ ! -f resources/AppIcon.icns ]]; then swift scripts/icon.swift; fi
cp resources/AppIcon.icns "$app/Contents/Resources/AppIcon.icns"
# Stable local identity; pass SIGNING_IDENTITY for a Developer ID distribution.
identity="${SIGNING_IDENTITY:--}"
# Sign nested executables inside out. Ad hoc builds cannot use library validation.
framework="$app/Contents/Frameworks/Sparkle.framework/Versions/B"
options=(--force)
if [[ "$identity" != - ]]; then options+=(--options runtime --timestamp); fi
for component in "$framework/XPCServices/Downloader.xpc" \
  "$framework/XPCServices/Installer.xpc" "$framework/Autoupdate" \
  "$framework/Updater.app" "$app/Contents/Frameworks/Sparkle.framework" "$app"; do
  codesign "${options[@]}" --sign "$identity" "$component"
done
codesign --verify --deep --strict "$app"
if [[ "${1:-}" == --install ]]; then
  destination="$HOME/Applications/AMStats.app"
  mkdir -p "$HOME/Applications"
  if /usr/bin/pgrep -x amstats >/dev/null; then
    echo 'Cierra AMStats antes de instalar una nueva versión.' >&2; exit 1
  fi
  ditto "$app" "$destination"
  echo "Instalada: $destination"
else
  echo "Aplicación lista: $app"
fi
