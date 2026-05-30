#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AURA_STORAGE_ROOT="${AURA_STORAGE_ROOT:-$ROOT_DIR/.aura-server}"
AURA_API_PORT="${AURA_API_PORT:-3000}"
AURA_WEB_PORT="${AURA_WEB_PORT:-5173}"

cd "$ROOT_DIR"

echo "=== Aura Hub dev stack ==="
echo "storage: $AURA_STORAGE_ROOT"

if [ ! -d web/node_modules ]; then
  echo "[setup] installing web dependencies"
  npm --prefix web install
fi

echo "[build] compiling Aura Rust workspace"
cargo build --workspace

echo "[run] starting Aura Hub API on :$AURA_API_PORT"
AURA_STORAGE_ROOT="$AURA_STORAGE_ROOT" AURA_API_PORT="$AURA_API_PORT" cargo run -p aura-server &
SERVER_PID=$!

echo "[run] starting Aura web UI on :$AURA_WEB_PORT"
VITE_AURA_API_URL="http://localhost:$AURA_API_PORT/api" \
  npm --prefix web run dev -- --host 0.0.0.0 --port "$AURA_WEB_PORT" &
WEB_PID=$!

cleanup() {
  echo
  echo "Stopping Aura services..."
  kill "$SERVER_PID" "$WEB_PID" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

cat <<EOF

Aura is running:
  Web UI: http://localhost:$AURA_WEB_PORT
  API:    http://localhost:$AURA_API_PORT/api

Press Ctrl+C to stop.
EOF

wait "$SERVER_PID" "$WEB_PID"
