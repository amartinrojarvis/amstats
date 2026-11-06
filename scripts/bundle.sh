#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -m)" != arm64 ]]; then
  echo 'AMStats solo se compila para Apple Silicon.' >&2; exit 1
fi
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release --locked
app="$PWD/dist/AMStats.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/aarch64-apple-darwin/release/amstats "$app/Contents/MacOS/amstats"
cp resources/Info.plist "$app/Contents/Info.plist"
if [[ ! -f resources/AppIcon.icns ]]; then swift scripts/icon.swift; fi
cp resources/AppIcon.icns "$app/Contents/Resources/AppIcon.icns"
# Stable local identity; pass SIGNING_IDENTITY for a Developer ID distribution.
identity="${SIGNING_IDENTITY:--}"
codesign --force --options runtime --sign "$identity" "$app"
codesign --verify --strict "$app"
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
