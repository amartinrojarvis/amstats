#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
plutil -lint resources/Info.plist
