#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEV_DIR="$ROOT/.dev"
BACKEND_PID_FILE="$DEV_DIR/backend.pid"
FRONTEND_PID_FILE="$DEV_DIR/frontend.pid"

show_proc() {
  local label="$1"
  local pid_file="$2"
  if [[ ! -f "$pid_file" ]]; then
    echo "$label: not running"
    return
  fi
  local pid
  pid="$(cat "$pid_file" || true)"
  if [[ -n "${pid:-}" ]] && kill -0 "$pid" 2>/dev/null; then
    echo "$label: running (pid $pid)"
  else
    echo "$label: stale pid file"
  fi
}

show_http() {
  local label="$1"
  local url="$2"
  if curl -fsS "$url" >/dev/null 2>&1; then
    echo "$label: OK ($url)"
  else
    echo "$label: DOWN ($url)"
  fi
}

show_proc "Backend" "$BACKEND_PID_FILE"
show_proc "Frontend" "$FRONTEND_PID_FILE"
show_http "Backend API" "http://localhost:8080/api/status"
show_http "Frontend" "http://localhost:3000"
