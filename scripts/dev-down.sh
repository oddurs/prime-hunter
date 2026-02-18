#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEV_DIR="$ROOT/.dev"
BACKEND_PID_FILE="$DEV_DIR/backend.pid"
FRONTEND_PID_FILE="$DEV_DIR/frontend.pid"

stop_pid_file() {
  local label="$1"
  local pid_file="$2"
  if [[ ! -f "$pid_file" ]]; then
    echo "$label: not running (no pid file)"
    return
  fi

  local pid
  pid="$(cat "$pid_file" || true)"
  if [[ -z "${pid:-}" ]]; then
    rm -f "$pid_file"
    echo "$label: stale pid file removed"
    return
  fi

  if kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    sleep 0.4
    if kill -0 "$pid" 2>/dev/null; then
      kill -9 "$pid" 2>/dev/null || true
    fi
    echo "$label: stopped pid $pid"
  else
    echo "$label: pid $pid already dead"
  fi
  rm -f "$pid_file"
}

stop_pid_file "Backend" "$BACKEND_PID_FILE"
stop_pid_file "Frontend" "$FRONTEND_PID_FILE"
pkill -f "primehunt.*dashboard" 2>/dev/null || true
pkill -f "next dev" 2>/dev/null || true
