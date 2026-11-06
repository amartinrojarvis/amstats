#!/bin/bash
# Prepare a signed release locally. Signing key stays in the macOS login Keychain.
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/check.sh
./scripts/bundle.sh
python3 scripts/release.py
