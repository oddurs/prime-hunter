#!/usr/bin/env bash
# One-time setup for darkreach.test local dev environment.
# Installs Caddy + dnsmasq, configures DNS resolution for *.darkreach.test,
# and trusts Caddy's local CA certificate.
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info()  { echo -e "${GREEN}[setup]${NC} $*"; }
warn()  { echo -e "${YELLOW}[setup]${NC} $*"; }
error() { echo -e "${RED}[setup]${NC} $*" >&2; }

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# ── Preflight ───────────────────────────────────────────────────────
if [[ "$(uname -s)" != "Darwin" ]]; then
  error "This script is macOS-only (uses brew services + /etc/resolver)."
  exit 1
fi

if ! command -v brew &>/dev/null; then
  error "Homebrew is required. Install from https://brew.sh"
  exit 1
fi

# ── Install Caddy + dnsmasq ────────────────────────────────────────
info "Installing caddy and dnsmasq (idempotent)..."
brew install caddy dnsmasq 2>/dev/null || true

# ── Configure dnsmasq ──────────────────────────────────────────────
BREW_PREFIX="$(brew --prefix)"
DNSMASQ_CONF_DIR="$BREW_PREFIX/etc/dnsmasq.d"
DNSMASQ_MAIN="$BREW_PREFIX/etc/dnsmasq.conf"

mkdir -p "$DNSMASQ_CONF_DIR"

# Ensure conf-dir include is present in the main config
if ! grep -q "conf-dir=$DNSMASQ_CONF_DIR" "$DNSMASQ_MAIN" 2>/dev/null; then
  info "Adding conf-dir include to $DNSMASQ_MAIN"
  echo "conf-dir=$DNSMASQ_CONF_DIR/,*.conf" >> "$DNSMASQ_MAIN"
fi

info "Copying dnsmasq config to $DNSMASQ_CONF_DIR/darkreach-test.conf"
cp "$ROOT/dev/dnsmasq.conf" "$DNSMASQ_CONF_DIR/darkreach-test.conf"

info "Restarting dnsmasq (requires sudo)..."
sudo brew services restart dnsmasq

# ── Configure macOS resolver ───────────────────────────────────────
RESOLVER_DIR="/etc/resolver"
RESOLVER_FILE="$RESOLVER_DIR/test"

if [[ ! -f "$RESOLVER_FILE" ]] || ! grep -q "nameserver 127.0.0.1" "$RESOLVER_FILE" 2>/dev/null; then
  info "Creating $RESOLVER_FILE (requires sudo)..."
  sudo mkdir -p "$RESOLVER_DIR"
  echo "nameserver 127.0.0.1" | sudo tee "$RESOLVER_FILE" >/dev/null
else
  info "$RESOLVER_FILE already configured."
fi

# ── Verify DNS ─────────────────────────────────────────────────────
info "Verifying DNS resolution..."
sleep 1  # give dnsmasq a moment to start

RESOLVED="$(dig +short darkreach.test @127.0.0.1 2>/dev/null || true)"
if [[ "$RESOLVED" == "127.0.0.1" ]]; then
  info "DNS OK: darkreach.test -> 127.0.0.1"
else
  warn "DNS check returned '$RESOLVED' instead of 127.0.0.1"
  warn "Try: sudo brew services restart dnsmasq"
  warn "Then: dig +short darkreach.test @127.0.0.1"
fi

RESOLVED_SUB="$(dig +short app.darkreach.test @127.0.0.1 2>/dev/null || true)"
if [[ "$RESOLVED_SUB" == "127.0.0.1" ]]; then
  info "DNS OK: app.darkreach.test -> 127.0.0.1"
else
  warn "Subdomain DNS check returned '$RESOLVED_SUB'"
fi

# ── Trust Caddy CA ─────────────────────────────────────────────────
info "Installing Caddy's local CA certificate into system trust store..."
caddy trust 2>/dev/null || warn "caddy trust failed — you may need to run it manually"

# ── Summary ────────────────────────────────────────────────────────
echo ""
info "Setup complete!"
echo ""
echo "  Local URLs:"
echo "    https://app.darkreach.test  -> Next.js frontend (:3001)"
echo "    https://api.darkreach.test  -> Rust Axum backend (:7001)"
echo "    https://darkreach.test      -> redirects to app.darkreach.test"
echo ""
echo "  Start everything with:"
echo "    ./dev/dev.sh"
echo ""
