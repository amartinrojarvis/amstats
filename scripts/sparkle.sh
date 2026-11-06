#!/bin/bash
# Pinned upstream distribution. Keep version and SHA-256 together when upgrading.
set -euo pipefail
cd "$(dirname "$0")/.."
version=2.10.1
sha256=1a856d2a9edb8e628de88121d7be04cf0e2167f8641650b78fca00b14244cf27
archive="target/sparkle-download/Sparkle-$version.tar.xz"
mkdir -p target/sparkle-download target/sparkle
if [[ ! -f "$archive" ]]; then
  curl --fail --location --proto '=https' --tlsv1.2 --retry 3 \
    "https://github.com/sparkle-project/Sparkle/releases/download/$version/Sparkle-$version.tar.xz" \
    --output "$archive"
fi
echo "$sha256  $archive" | shasum -a 256 --check
tar -xf "$archive" -C target/sparkle
