#!/usr/bin/env bash
set -euo pipefail

# Deploy darkreach worker(s) to a remote server.
#
# Sets up a Hetzner cloud server as a fleet worker:
#   1. System hardening (swap, UFW, kernel tuning for compute)
#   2. Installs Rust toolchain + GMP
#   3. Clones repo, builds release binary (target-cpu=native for GMP perf)
#   4. Copies .env with DATABASE_URL for Supabase access
#   5. Creates per-instance systemd worker services
#   6. Starts N worker instances (one per vCPU)
#
# Usage: ./deploy/worker-deploy.sh <user@host> <coordinator-url> [--workers N]
#
# Example:
#   ./deploy/worker-deploy.sh root@178.156.158.184 http://178.156.211.107 --workers 4

if [ $# -lt 2 ]; then
    echo "Usage: $0 <user@host> <coordinator-url> [--workers N]"
    echo ""
    echo "Example:"
    echo "  $0 root@178.156.158.184 http://178.156.211.107 --workers 4"
    exit 1
fi

SERVER="$1"
COORDINATOR_URL="$2"
NUM_WORKERS=4  # Default: one per vCPU on ccx23

# Parse optional args
shift 2
while [ $# -gt 0 ]; do
    case "$1" in
        --workers) NUM_WORKERS="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
REPO_URL="$(cd "$PROJECT_DIR" && git remote get-url origin 2>/dev/null || echo '')"
BRANCH="$(cd "$PROJECT_DIR" && git branch --show-current)"

echo "=== Darkreach Worker Deploy ==="
echo "Server:      $SERVER"
echo "Coordinator: $COORDINATOR_URL"
echo "Workers:     $NUM_WORKERS"
echo "Branch:      $BRANCH"
echo ""

# ---------------------------------------------------------------
# Step 1: System hardening (swap, UFW, kernel tuning)
# ---------------------------------------------------------------
echo "==> [1/6] System hardening"

ssh "$SERVER" bash -s <<'REMOTE_HARDEN'
set -euo pipefail

echo "--- Swap file (4GB for 16GB RAM server) ---"
if [ ! -f /swapfile ]; then
    fallocate -l 4G /swapfile
    chmod 600 /swapfile
    mkswap /swapfile
    swapon /swapfile
    echo '/swapfile none swap sw 0 0' >> /etc/fstab
    echo "  Swap created (4GB)"
else
    echo "  Swap already exists"
fi
swapon --show

echo "--- UFW firewall ---"
apt-get update -qq
apt-get install -y -qq ufw
ufw default deny incoming
ufw default allow outgoing
ufw allow 22/tcp comment "SSH"
echo "y" | ufw enable || true
ufw status

echo "--- Kernel tuning (compute-optimized) ---"
cat > /etc/sysctl.d/99-darkreach-worker.conf <<'SYSCTL'
# Low swappiness: keep compute data in RAM, only swap under pressure
vm.swappiness = 10
# Increase max memory map areas for large GMP allocations
vm.max_map_count = 262144
# TCP keepalive for coordinator connection stability
net.ipv4.tcp_keepalive_time = 60
net.ipv4.tcp_keepalive_intvl = 10
net.ipv4.tcp_keepalive_probes = 6
SYSCTL
sysctl --system >/dev/null 2>&1
echo "  Kernel params applied"

echo "--- Journald log management ---"
mkdir -p /etc/systemd/journald.conf.d
cat > /etc/systemd/journald.conf.d/darkreach.conf <<'JOURNALD'
[Journal]
SystemMaxUse=500M
SystemMaxFileSize=50M
MaxRetentionSec=1week
Compress=yes
JOURNALD
systemctl restart systemd-journald
echo "  Journald configured (500MB cap, 1 week retention)"
REMOTE_HARDEN

# ---------------------------------------------------------------
# Step 2: Install dependencies (Rust, GMP)
# ---------------------------------------------------------------
echo ""
echo "==> [2/6] Installing dependencies"

ssh "$SERVER" bash -s <<'REMOTE_DEPS'
set -euo pipefail

echo "--- Rust toolchain ---"
if ! command -v cargo &>/dev/null; then
    echo "  Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    echo "  Rust installed: $(rustc --version)"
else
    source "$HOME/.cargo/env" 2>/dev/null || true
    echo "  Rust already installed: $(rustc --version)"
fi

echo "--- Build dependencies (GMP, pkg-config, git) ---"
apt-get install -y -qq build-essential libgmp-dev libssl-dev m4 git pkg-config
echo "  Build deps installed"
REMOTE_DEPS

# ---------------------------------------------------------------
# Step 3: Clone repo + build release binary
# ---------------------------------------------------------------
echo ""
echo "==> [3/6] Building release binary"

ssh "$SERVER" bash -s -- "$REPO_URL" "$BRANCH" <<'REMOTE_BUILD'
set -euo pipefail
REPO_URL="$1"
BRANCH="$2"
INSTALL_DIR="/opt/darkreach"
BIN_DIR="/usr/local/bin"

source "$HOME/.cargo/env" 2>/dev/null || true

echo "--- Fetching code ---"
if [ -d "${INSTALL_DIR}" ]; then
    cd "${INSTALL_DIR}"
    git fetch origin
    git checkout "${BRANCH}"
    git reset --hard "origin/${BRANCH}"
else
    git clone "${REPO_URL}" "${INSTALL_DIR}"
    cd "${INSTALL_DIR}"
    git checkout "${BRANCH}"
fi

echo "--- Building (release, target-cpu=native for GMP perf) ---"
# native is correct for x86 Hetzner servers (Apple Silicon bug doesn't apply)
RUSTFLAGS="-C target-cpu=native" cargo build --release

echo "--- Installing binary ---"
cp target/release/darkreach "${BIN_DIR}/darkreach"
chmod +x "${BIN_DIR}/darkreach"
echo "  Installed: $(darkreach --help | head -1)"
REMOTE_BUILD

# ---------------------------------------------------------------
# Step 4: Copy .env for DATABASE_URL
# ---------------------------------------------------------------
echo ""
echo "==> [4/6] Configuring environment"

if [ -f "$PROJECT_DIR/.env" ]; then
    scp "$PROJECT_DIR/.env" "$SERVER:/opt/darkreach/.env"
    ssh "$SERVER" "chmod 600 /opt/darkreach/.env"
    echo "  .env deployed"
else
    echo "  WARNING: No .env file found at $PROJECT_DIR/.env"
    echo "  Workers need DATABASE_URL to log results to Supabase."
    echo "  Create /opt/darkreach/.env on the server manually."
fi

# ---------------------------------------------------------------
# Step 5: Create systemd worker services
# ---------------------------------------------------------------
echo ""
echo "==> [5/6] Creating $NUM_WORKERS worker service(s)"

ssh "$SERVER" bash -s -- "$COORDINATOR_URL" "$NUM_WORKERS" <<'REMOTE_SERVICES'
set -euo pipefail
COORDINATOR_URL="$1"
NUM_WORKERS="$2"

# Install the worker template unit with the actual coordinator URL
cat > /etc/systemd/system/darkreach-worker@.service <<UNIT
[Unit]
Description=Darkreach Worker (%i)
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
EnvironmentFile=/opt/darkreach/.env
# Each worker instance gets a unique ID (%H=hostname, %i=instance number).
# Workers connect to the coordinator to claim work blocks from search jobs.
# --threads 1: one compute thread per instance, run N instances for N vCPUs.
ExecStart=/usr/local/bin/darkreach --coordinator ${COORDINATOR_URL} --worker-id %H-%i --threads 1 --checkpoint /opt/darkreach/darkreach-%i.checkpoint
WorkingDirectory=/opt/darkreach
Restart=always
RestartSec=10

# Resource isolation: prevent any single worker from consuming all RAM
MemoryMax=3G

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=darkreach-worker-%i

[Install]
WantedBy=multi-user.target
UNIT

systemctl daemon-reload

# Enable and start each worker instance
for i in $(seq 1 "$NUM_WORKERS"); do
    systemctl enable "darkreach-worker@${i}"
    systemctl start "darkreach-worker@${i}"
    echo "  Worker @${i} started"
done
REMOTE_SERVICES

# ---------------------------------------------------------------
# Step 6: Verification
# ---------------------------------------------------------------
echo ""
echo "==> [6/6] Verification"

# Give workers a moment to start up and register
sleep 3

ssh "$SERVER" bash -s -- "$NUM_WORKERS" "$COORDINATOR_URL" <<'VERIFY'
set -uo pipefail
NUM_WORKERS="$1"
COORDINATOR_URL="$2"
PASS=0
FAIL=0

check() {
    local label="$1"
    shift
    if eval "$@" >/dev/null 2>&1; then
        echo "  [PASS] $label"
        PASS=$((PASS + 1))
    else
        echo "  [FAIL] $label"
        FAIL=$((FAIL + 1))
    fi
}

check "Swap active"           "swapon --show | grep -q swapfile"
check "UFW enabled"           "ufw status | grep -q 'Status: active'"
check "Binary installed"      "darkreach --help"
check ".env exists"           "test -f /opt/darkreach/.env"
check "Coordinator reachable" "curl -sf --max-time 5 ${COORDINATOR_URL}/api/status"

for i in $(seq 1 "$NUM_WORKERS"); do
    check "Worker @${i} running" "systemctl is-active darkreach-worker@${i}"
done

echo ""
echo "  Results: $PASS passed, $FAIL failed"
VERIFY

echo ""
echo "=== Worker Deploy Complete ==="
echo "Server:      $SERVER"
echo "Coordinator: $COORDINATOR_URL"
echo "Workers:     $NUM_WORKERS instances"
echo ""
echo "Useful commands:"
echo "  ssh $SERVER journalctl -u 'darkreach-worker@*' -f       # Follow all worker logs"
echo "  ssh $SERVER journalctl -u darkreach-worker@1 -f         # Follow worker 1"
echo "  ssh $SERVER systemctl status 'darkreach-worker@*'       # Status of all workers"
echo "  ssh $SERVER systemctl restart darkreach-worker@1        # Restart worker 1"
echo "  curl ${COORDINATOR_URL}/api/fleet                       # Check fleet status"
