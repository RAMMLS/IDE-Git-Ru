#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="${COMPOSE_FILE:-$ROOT_DIR/docker-compose.yml}"
API_URL="${AURA_API_URL:-http://localhost:3000/api/repos}"
WEB_URL="${AURA_WEB_URL:-http://localhost:8080/}"
WAIT_ATTEMPTS="${WAIT_ATTEMPTS:-30}"
WAIT_SECONDS="${WAIT_SECONDS:-2}"

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

wait_for_url() {
  local name="$1"
  local url="$2"

  for ((i = 1; i <= WAIT_ATTEMPTS; i++)); do
    if curl -fsS "$url" >/dev/null 2>&1; then
      echo "[ok] $name is reachable: $url"
      return
    fi
    sleep "$WAIT_SECONDS"
  done

  echo "[error] $name did not become ready in time: $url" >&2
  exit 1
}

require_command docker
require_command curl

if ! docker compose version >/dev/null 2>&1; then
  echo "Docker Compose v2 is required" >&2
  exit 1
fi

cd "$ROOT_DIR"

echo "==> Deploying Aura stack"
echo "Project root: $ROOT_DIR"
echo "Compose file: $COMPOSE_FILE"

docker compose -f "$COMPOSE_FILE" up --build -d

echo "==> Waiting for services"
wait_for_url "Aura API" "$API_URL"
wait_for_url "Aura Web" "$WEB_URL"

echo
echo "Aura is deployed:"
echo "  Web UI: $WEB_URL"
echo "  API:    $API_URL"
echo
echo "Useful commands:"
echo "  docker compose -f \"$COMPOSE_FILE\" ps"
echo "  docker compose -f \"$COMPOSE_FILE\" logs -f"
echo "  docker compose -f \"$COMPOSE_FILE\" down"
