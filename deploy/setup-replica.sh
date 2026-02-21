#!/usr/bin/env bash
# Set up a PostgreSQL streaming replica for darkreach
# Run on the replica server.
#
# Usage: ./setup-replica.sh <primary_host> <replicator_password>
set -euo pipefail

PRIMARY_HOST="${1:?Usage: $0 <primary_host> <replicator_password>}"
REPLICATOR_PASS="${2:?Usage: $0 <primary_host> <replicator_password>}"
PGDATA="${PGDATA:-/var/lib/postgresql/16/darkreach}"

echo "=== Setting up streaming replica ==="
echo "Primary: ${PRIMARY_HOST}"
echo "PGDATA:  ${PGDATA}"
echo ""

# Stop PostgreSQL if running
systemctl stop darkreach-postgresql 2>/dev/null || true

# Remove existing data directory
if [ -d "$PGDATA" ]; then
    echo "Removing existing PGDATA..."
    rm -rf "$PGDATA"
fi

# Base backup from primary
echo "Taking base backup from primary..."
PGPASSWORD="$REPLICATOR_PASS" pg_basebackup \
    -h "$PRIMARY_HOST" \
    -U replicator \
    -D "$PGDATA" \
    -Fp -Xs -P -R

# Copy replica config
cp "$(dirname "$0")/pg-replica.conf" "${PGDATA}/postgresql.conf"

# Set primary connection info in postgresql.auto.conf
cat >> "${PGDATA}/postgresql.auto.conf" <<EOF
primary_conninfo = 'host=${PRIMARY_HOST} port=5432 user=replicator password=${REPLICATOR_PASS} application_name=replica1'
EOF

# Ensure standby signal file exists (created by -R flag, but verify)
touch "${PGDATA}/standby.signal"

# Fix permissions
chown -R postgres:postgres "$PGDATA"
chmod 700 "$PGDATA"

# Start PostgreSQL
systemctl start darkreach-postgresql

# Wait for startup
sleep 3

# Verify replication status
echo ""
echo "=== Replica Status ==="
su - postgres -c "psql -c \"SELECT pg_is_in_recovery(), pg_last_wal_receive_lsn(), pg_last_wal_replay_lsn();\""
echo ""
echo "Replica is running. Set REPLICA_DATABASE_URL on the coordinator to enable read routing."
