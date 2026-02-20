#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEV_DIR="$ROOT/.dev"
BACKEND_LOG="$DEV_DIR/backend.log"
FRONTEND_LOG="$DEV_DIR/frontend.log"
MODE="local"
REMOTE_BASE=""

if [[ "${1:-}" == "--remote" ]]; then
  if [[ $# -lt 2 ]]; then
    echo "Usage: $0 [--remote <dashboard_base_url>]"
    echo "Example: $0 --remote https://darkreach.example.com"
    exit 1
  fi
  MODE="remote"
  REMOTE_BASE="${2%/}"
fi

mkdir -p "$DEV_DIR"

cleanup() {
  local code=$?
  trap - EXIT INT TERM
  if [[ -n "${FRONTEND_PID:-}" ]] && kill -0 "$FRONTEND_PID" 2>/dev/null; then
    kill "$FRONTEND_PID" 2>/dev/null || true
  fi
  if [[ -n "${BACKEND_PID:-}" ]] && kill -0 "$BACKEND_PID" 2>/dev/null; then
    kill "$BACKEND_PID" 2>/dev/null || true
  fi
  if [[ -n "${FRONTEND_PID:-}" ]]; then
    wait "$FRONTEND_PID" 2>/dev/null || true
  fi
  if [[ -n "${BACKEND_PID:-}" ]]; then
    wait "$BACKEND_PID" 2>/dev/null || true
  fi
  exit "$code"
}

wait_for_http() {
  local url="$1"
  local label="$2"
  for _ in {1..100}; do
    if curl -fsS "$url" >/dev/null 2>&1; then
      echo "$label is ready: $url"
      return 0
    fi
    sleep 0.2
  done
  echo "Timed out waiting for $label at $url"
  return 1
}

trap cleanup EXIT INT TERM

pkill -f "darkreach.*dashboard" 2>/dev/null || true
pkill -f "next dev" 2>/dev/null || true

if [[ "$MODE" == "local" ]]; then
  echo "Starting backend (:7001)..."
  (cd "$ROOT" && cargo run -- dashboard --port 7001 >"$BACKEND_LOG" 2>&1) &
  BACKEND_PID=$!
  wait_for_http "http://localhost:7001/api/status" "Backend"

  echo "Starting frontend (:3001)..."
  (
    cd "$ROOT/frontend" && \
    NEXT_PUBLIC_API_URL="http://localhost:7001" \
    NEXT_PUBLIC_WS_URL="ws://localhost:7001/ws" \
    npm run dev -- --port 3001 >"$FRONTEND_LOG" 2>&1
  ) &
  FRONTEND_PID=$!
  wait_for_http "http://localhost:3001" "Frontend"

  echo "Dev stack ready:"
  echo "  Frontend: http://localhost:3001"
  echo "  Backend:  http://localhost:7001"
else
  if [[ ! "$REMOTE_BASE" =~ ^https?:// ]]; then
    echo "--remote value must start with http:// or https://"
    exit 1
  fi
  REMOTE_WS_BASE="${REMOTE_BASE/http:\/\//ws://}"
  REMOTE_WS_BASE="${REMOTE_WS_BASE/https:\/\//wss://}"
  echo "Starting frontend (:3001) with remote data source..."
  echo "  Remote dashboard: $REMOTE_BASE"
  echo "  Remote ws:        ${REMOTE_WS_BASE}/ws"
  (
    cd "$ROOT/frontend" && \
    DEV_PROXY_TARGET="$REMOTE_BASE" \
    NEXT_PUBLIC_API_URL="" \
    NEXT_PUBLIC_WS_URL="${REMOTE_WS_BASE}/ws" \
    npm run dev -- --port 3001 >"$FRONTEND_LOG" 2>&1
  ) &
  FRONTEND_PID=$!
  wait_for_http "http://localhost:3001" "Frontend"

  echo "Remote UI ready:"
  echo "  Frontend: http://localhost:3001"
  echo "  Data via: $REMOTE_BASE (/api + /ws proxy)"
fi

echo "Logs:"
echo "  $FRONTEND_LOG"
if [[ -n "${BACKEND_PID:-}" ]]; then
  echo "  $BACKEND_LOG"
fi
echo "Press Ctrl+C to stop."

while true; do
  if [[ -n "${BACKEND_PID:-}" ]] && ! kill -0 "$BACKEND_PID" 2>/dev/null; then
    echo "Backend exited; shutting down."
    break
  fi
  if [[ -n "${FRONTEND_PID:-}" ]] && ! kill -0 "$FRONTEND_PID" 2>/dev/null; then
    echo "Frontend exited; shutting down."
    break
  fi
  sleep 1
done
