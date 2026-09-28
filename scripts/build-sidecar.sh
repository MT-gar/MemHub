#!/usr/bin/env bash
# Build the memhub CLI in release mode and copy it into the Tauri sidecar folder
# with the host target triple suffix (what `externalBin` expects).
set -euo pipefail
cd "$(dirname "$0")/.."
TRIPLE="${1:-$(rustc -vV | sed -n 's/^host: //p')}"
cargo build --release -p memhub-cli
EXT=""; case "$TRIPLE" in *windows*) EXT=".exe";; esac
mkdir -p apps/desktop/src-tauri/binaries
cp "target/release/memhub$EXT" "apps/desktop/src-tauri/binaries/memhub-$TRIPLE$EXT"
echo "sidecar → apps/desktop/src-tauri/binaries/memhub-$TRIPLE$EXT"
