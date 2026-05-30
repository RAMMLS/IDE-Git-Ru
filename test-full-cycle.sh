#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AURA_API_BASE_URL="${AURA_API_BASE_URL:-http://localhost:3000}"
AURA_BIN="${AURA_BIN:-$ROOT_DIR/target/debug/aura}"
AURA_E2E_MODE="${AURA_E2E_MODE:-auto}"
AURA_SKIP_HEALTHCHECK="${AURA_SKIP_HEALTHCHECK:-0}"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/aura-full-cycle-XXXXXX")"
REMOTE_ID="full-cycle-$(date +%s)"
REMOTE_URL="${AURA_API_BASE_URL%/}/api/transport/repos/${REMOTE_ID}"
SOURCE_REPO="$TMP_ROOT/source"
CLONE_REPO="$TMP_ROOT/clone"
TEST_FILE="README.md"
TEST_CONTENT="hello from aura full cycle"

cleanup() {
  rm -rf "$TMP_ROOT"
}
trap cleanup EXIT

resolve_compose_network() {
  local container_id
  container_id="$(docker compose ps -q aura-server 2>/dev/null || true)"
  if [[ -z "$container_id" ]]; then
    return 1
  fi

  docker inspect \
    -f '{{range $name, $_ := .NetworkSettings.Networks}}{{println $name}}{{end}}' \
    "$container_id" \
    | head -n 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

ensure_aura_bin() {
  if [[ -x "$AURA_BIN" ]]; then
    return
  fi

  echo "Building Aura CLI..."
  cargo build -p aura-control --bin aura --manifest-path "$ROOT_DIR/Cargo.toml"
}

wait_for_server() {
  if [[ "$AURA_SKIP_HEALTHCHECK" == "1" ]]; then
    return
  fi

  local attempts=30
  local delay=2

  for ((i = 1; i <= attempts; i++)); do
    if curl -fsS "${AURA_API_BASE_URL%/}/api/repos" >/dev/null 2>&1; then
      return
    fi
    sleep "$delay"
  done

  echo "Aura API is not reachable at ${AURA_API_BASE_URL%/}" >&2
  exit 1
}

run_aura() {
  "$AURA_BIN" "$@"
}

if [[ "$AURA_E2E_MODE" == "auto" ]]; then
  if command -v docker >/dev/null 2>&1 && docker compose ps aura-server >/dev/null 2>&1; then
    COMPOSE_NETWORK="$(resolve_compose_network || true)"
    if [[ -n "$COMPOSE_NETWORK" ]]; then
      echo "==> Running full cycle inside docker network $COMPOSE_NETWORK"
      docker run --rm \
        --network "$COMPOSE_NETWORK" \
        -v "$ROOT_DIR:/workspace" \
        -w /workspace \
        -e AURA_E2E_MODE=host \
        -e AURA_API_BASE_URL=http://aura-server:3000 \
        -e CARGO_TARGET_DIR=/tmp/aura-target \
        -e AURA_BIN=/tmp/aura-target/debug/aura \
        -e AURA_SKIP_HEALTHCHECK=1 \
        rust:1.86-bookworm \
        bash -c "./test-full-cycle.sh"
      exit 0
    fi
  fi
fi

echo "==> Verifying prerequisites"
require_command curl
require_command cargo
ensure_aura_bin
wait_for_server

echo "==> Preparing source repository in $SOURCE_REPO"
mkdir -p "$SOURCE_REPO"
run_aura init "$SOURCE_REPO"
printf '%s\n' "$TEST_CONTENT" > "$SOURCE_REPO/$TEST_FILE"
run_aura -C "$SOURCE_REPO" add "$TEST_FILE"
run_aura -C "$SOURCE_REPO" commit -m "full cycle smoke"
run_aura -C "$SOURCE_REPO" remote add origin "$REMOTE_URL"

echo "==> Pushing commit to Aura Hub via Docker API"
run_aura -C "$SOURCE_REPO" push origin main

echo "==> Cloning hosted repository into $CLONE_REPO"
run_aura clone "$REMOTE_URL" "$CLONE_REPO"

echo "==> Validating clone contents"
if [[ ! -f "$CLONE_REPO/$TEST_FILE" ]]; then
  echo "Expected file $TEST_FILE is missing in clone" >&2
  exit 1
fi

if [[ "$(cat "$CLONE_REPO/$TEST_FILE")" != "$TEST_CONTENT" ]]; then
  echo "Cloned file contents do not match source repository" >&2
  exit 1
fi

if [[ ! -d "$CLONE_REPO/.aura" ]]; then
  echo "Cloned repository is missing .aura metadata" >&2
  exit 1
fi

echo "==> Full cycle succeeded"
echo "Remote: $REMOTE_URL"
echo "Source: $SOURCE_REPO"
echo "Clone:  $CLONE_REPO"
