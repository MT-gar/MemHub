#!/usr/bin/env bash
# Browser-mode dev loop: Rust API on :7337 (with the demo home) + Vite HMR on :1420 proxying /api.
set -euo pipefail
cd "$(dirname "$0")/.."
DEMO="${DEMO:-1}"
if [ "$DEMO" = "1" ]; then
  [ -d demo/home ] || ./scripts/demo.sh "$PWD/demo/home"
  export HOME="$PWD/demo/home" MEMHUB_USER_HOME="$PWD/demo/home" MEMHUB_HOME="$PWD/demo/home/.memhub"
fi
cargo build -p memhub-cli
./target/debug/memhub serve --port 7337 --allow-origin http://localhost:1420 --allow-origin http://127.0.0.1:1420 &
API=$!
trap 'kill $API' EXIT
(cd ui && npm run dev)
