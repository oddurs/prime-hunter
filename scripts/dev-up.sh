#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEV_DIR="$ROOT/.dev"
BACKEND_PID_FILE="$DEV_DIR/backend.pid"
FRONTEND_PID_FILE="$DEV_DIR/frontend.pid"
BACKEND_LOG="$DEV_DIR/backend.log"
FRONTEND_LOG="$DEV_DIR/frontend.log"

mkdir -p "$DEV_DIR"

kill_pid_file() {
  local pid_file="$1"
  if [[ -f "$pid_file" ]]; then
    local pid
    pid="$(cat "$pid_file" || true)"
    if [[ -n "${pid:-}" ]] && kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
      sleep 0.5
      kill -9 "$pid" 2>/dev/null || true
    fi
    rm -f "$pid_file"
  fi
}

wait_for_http() {
  local url="$1"
  local label="$2"
  for _ in {1..80}; do
    if curl -fsS "$url" >/dev/null 2>&1; then
      echo "$label is ready: $url"
      return 0
    fi
    sleep 0.25
  done
  echo "Timed out waiting for $label at $url"
  return 1
}

echo "Stopping stale dev processes..."
kill_pid_file "$BACKEND_PID_FILE"
kill_pid_file "$FRONTEND_PID_FILE"
pkill -f "primehunt.*dashboard" 2>/dev/null || true
pkill -f "next dev" 2>/dev/null || true

echo "Starting backend on :8080..."
nohup bash -lc "cd '$ROOT' && cargo run -- dashboard --port 8080" \
  >"$BACKEND_LOG" 2>&1 < /dev/null &
BACKEND_PID=$!
echo "$BACKEND_PID" >"$BACKEND_PID_FILE"
wait_for_http "http://localhost:8080/api/status" "Backend"

echo "Starting frontend on :3000..."
nohup bash -lc "cd '$ROOT/frontend' && NEXT_PUBLIC_API_URL='http://localhost:8080' NEXT_PUBLIC_WS_URL='ws://localhost:8080/ws' npm run dev -- --port 3000" \
  >"$FRONTEND_LOG" 2>&1 < /dev/null &
FRONTEND_PID=$!
echo "$FRONTEND_PID" >"$FRONTEND_PID_FILE"
wait_for_http "http://localhost:3000" "Frontend"

echo
echo "Dev stack is up."
echo "Frontend: http://localhost:3000"
echo "Backend API: http://localhost:8080/api/status"
echo "Logs: $BACKEND_LOG, $FRONTEND_LOG"
