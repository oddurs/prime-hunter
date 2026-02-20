#!/usr/bin/env bash
# Start the full darkreach dev stack: Caddy + Rust backend + Next.js frontend.
# All traffic goes through Caddy with HTTPS on *.darkreach.test.
#
# Usage:
#   ./dev/dev.sh                    # local backend + frontend
#   ./dev/dev.sh --remote <url>     # local frontend, remote backend
set -euo pipefail

# ── Colors ──────────────────────────────────────────────────────────
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
BLUE='\033[0;34m'
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
DIM='\033[2m'
NC='\033[0m'

info()  { echo -e "${GREEN}[dev]${NC} $*"; }
warn()  { echo -e "${YELLOW}[dev]${NC} $*"; }
error() { echo -e "${RED}[dev]${NC} $*" >&2; }

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEV_DIR="$ROOT/.dev"
mkdir -p "$DEV_DIR"

MODE="local"
REMOTE_BASE=""

if [[ "${1:-}" == "--remote" ]]; then
  if [[ $# -lt 2 ]]; then
    echo "Usage: $0 [--remote <dashboard_base_url>]"
    exit 1
  fi
  MODE="remote"
  REMOTE_BASE="${2%/}"
fi

# ── PID tracking ────────────────────────────────────────────────────
CADDY_PID=""
API_PID=""
APP_PID=""

cleanup() {
  local code=$?
  trap - EXIT INT TERM

  info "Shutting down..."

  # SIGTERM first
  for pid_var in CADDY_PID API_PID APP_PID; do
    local pid="${!pid_var:-}"
    if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
    fi
  done

  # Wait briefly, then SIGKILL stragglers
  sleep 1
  for pid_var in CADDY_PID API_PID APP_PID; do
    local pid="${!pid_var:-}"
    if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
      kill -9 "$pid" 2>/dev/null || true
    fi
  done

  # Wait for all children
  wait 2>/dev/null || true
  info "Stopped."
  exit "$code"
}

trap cleanup EXIT INT TERM

# ── Dependency checks ───────────────────────────────────────────────
check_cmd() {
  if ! command -v "$1" &>/dev/null; then
    error "Missing required command: $1"
    error "Run ./dev/setup.sh first, or install $1 manually."
    exit 1
  fi
}

check_cmd caddy
check_cmd cargo
check_cmd node
check_cmd npm

# ── DNS resolution check ───────────────────────────────────────────
RESOLVED="$(dig +short darkreach.test @127.0.0.1 2>/dev/null || true)"
if [[ "$RESOLVED" != "127.0.0.1" ]]; then
  error "DNS for darkreach.test does not resolve to 127.0.0.1 (got: '$RESOLVED')"
  error "Run ./dev/setup.sh to configure dnsmasq."
  exit 1
fi
info "DNS OK: *.darkreach.test -> 127.0.0.1"

# ── Port conflict detection ────────────────────────────────────────
check_port() {
  local port="$1"
  local label="$2"
  if lsof -nP -iTCP:"$port" -sTCP:LISTEN &>/dev/null; then
    error "Port $port ($label) is already in use:"
    lsof -nP -iTCP:"$port" -sTCP:LISTEN 2>/dev/null | head -5
    exit 1
  fi
}

check_port 443  "Caddy HTTPS"
check_port 3001 "Next.js frontend"
if [[ "$MODE" == "local" ]]; then
  check_port 7001 "Rust backend"
fi

# ── Start Caddy ─────────────────────────────────────────────────────
info "Starting Caddy reverse proxy..."
PROJECT_ROOT="$ROOT" caddy run --config "$ROOT/dev/Caddyfile" >"$DEV_DIR/caddy.log" 2>&1 &
CADDY_PID=$!

# Brief wait for Caddy to bind
sleep 1
if ! kill -0 "$CADDY_PID" 2>/dev/null; then
  error "Caddy failed to start. Check $DEV_DIR/caddy.log"
  exit 1
fi
info "Caddy running (pid $CADDY_PID)"

# ── Start Rust backend ──────────────────────────────────────────────
if [[ "$MODE" == "local" ]]; then
  info "Starting Rust backend on :7001 (first build may take a few minutes)..."
  (cd "$ROOT" && cargo run -- dashboard --port 7001) \
    > >(while IFS= read -r line; do echo -e "${CYAN}[api]${NC} $line"; done) \
    2> >(while IFS= read -r line; do echo -e "${CYAN}[api]${NC} $line"; done >&2) &
  API_PID=$!

  # Wait for backend (up to 3 minutes for cold compile)
  info "Waiting for backend (up to 3 min)..."
  for i in $(seq 1 180); do
    if curl -sf "http://localhost:7001/api/status" >/dev/null 2>&1; then
      info "Backend ready after ${i}s"
      break
    fi
    if ! kill -0 "$API_PID" 2>/dev/null; then
      error "Backend process died. Check output above."
      exit 1
    fi
    if [[ "$i" -eq 180 ]]; then
      error "Backend did not respond within 3 minutes."
      exit 1
    fi
    sleep 1
  done
fi

# ── Start Next.js frontend ─────────────────────────────────────────
if [[ "$MODE" == "local" ]]; then
  info "Starting Next.js frontend on :3001..."
  (
    cd "$ROOT/frontend" && \
    NEXT_PUBLIC_API_URL="https://api.darkreach.test" \
    NEXT_PUBLIC_WS_URL="wss://api.darkreach.test/ws" \
    npm run dev
  ) > >(while IFS= read -r line; do echo -e "${BLUE}[app]${NC} $line"; done) \
    2> >(while IFS= read -r line; do echo -e "${BLUE}[app]${NC} $line"; done >&2) &
  APP_PID=$!
else
  if [[ ! "$REMOTE_BASE" =~ ^https?:// ]]; then
    error "--remote value must start with http:// or https://"
    exit 1
  fi
  REMOTE_WS="${REMOTE_BASE/http:\/\//ws://}"
  REMOTE_WS="${REMOTE_WS/https:\/\//wss://}"

  info "Starting Next.js frontend on :3001 (remote: $REMOTE_BASE)..."
  (
    cd "$ROOT/frontend" && \
    DEV_PROXY_TARGET="$REMOTE_BASE" \
    NEXT_PUBLIC_API_URL="" \
    NEXT_PUBLIC_WS_URL="${REMOTE_WS}/ws" \
    npm run dev
  ) > >(while IFS= read -r line; do echo -e "${BLUE}[app]${NC} $line"; done) \
    2> >(while IFS= read -r line; do echo -e "${BLUE}[app]${NC} $line"; done >&2) &
  APP_PID=$!
fi

# Wait for frontend
info "Waiting for frontend..."
for i in $(seq 1 30); do
  if curl -sf "http://localhost:3001" >/dev/null 2>&1; then
    info "Frontend ready after ${i}s"
    break
  fi
  if ! kill -0 "$APP_PID" 2>/dev/null; then
    error "Frontend process died. Check output above."
    exit 1
  fi
  if [[ "$i" -eq 30 ]]; then
    warn "Frontend did not respond within 30s (may still be compiling)."
  fi
  sleep 1
done

# ── End-to-end HTTPS check ──────────────────────────────────────────
sleep 1
if curl -sk "https://api.darkreach.test/api/status" >/dev/null 2>&1; then
  info "HTTPS end-to-end check passed"
elif [[ "$MODE" == "local" ]]; then
  warn "HTTPS check failed — Caddy may still be obtaining internal certs."
  warn "Try: curl -sk https://api.darkreach.test/api/status"
fi

# ── Ready ────────────────────────────────────────────────────────────
echo ""
info "Dev stack ready!"
echo ""
echo -e "  ${GREEN}App:${NC}  https://app.darkreach.test"
echo -e "  ${GREEN}API:${NC}  https://api.darkreach.test"
echo -e "  ${GREEN}WS:${NC}   wss://api.darkreach.test/ws"
echo ""
echo -e "  ${DIM}Caddy log: $DEV_DIR/caddy.log${NC}"
echo ""
echo "  Press Ctrl+C to stop all services."
echo ""

# ── Monitor loop ─────────────────────────────────────────────────────
while true; do
  if [[ -n "$CADDY_PID" ]] && ! kill -0 "$CADDY_PID" 2>/dev/null; then
    error "Caddy exited unexpectedly."
    break
  fi
  if [[ "$MODE" == "local" && -n "$API_PID" ]] && ! kill -0 "$API_PID" 2>/dev/null; then
    error "Backend exited unexpectedly."
    break
  fi
  if [[ -n "$APP_PID" ]] && ! kill -0 "$APP_PID" 2>/dev/null; then
    error "Frontend exited unexpectedly."
    break
  fi
  sleep 2
done
