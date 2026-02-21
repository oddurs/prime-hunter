#!/usr/bin/env bash
# Restore darkreach PostgreSQL from backup
# Usage: ./pg-restore.sh /var/backups/darkreach/darkreach_20260221.sql.gz
set -euo pipefail

BACKUP_FILE="${1:?Usage: $0 <backup_file>}"

if [ ! -f "$BACKUP_FILE" ]; then
    echo "Error: Backup file not found: ${BACKUP_FILE}"
    exit 1
fi

echo "[$(date)] Verifying backup integrity..."
pg_restore --list "$BACKUP_FILE" > /dev/null 2>&1 || {
    echo "Error: Backup file is corrupt or invalid"
    exit 1
}

echo "[$(date)] Restoring from ${BACKUP_FILE}..."
echo "WARNING: This will drop and recreate the darkreach database."
read -p "Continue? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 0
fi

# Drop and recreate database
psql -U postgres -c "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = 'darkreach' AND pid <> pg_backend_pid();" || true
psql -U postgres -c "DROP DATABASE IF EXISTS darkreach;"
psql -U postgres -c "CREATE DATABASE darkreach OWNER darkreach;"

# Restore
pg_restore -U darkreach -d darkreach \
    --verbose \
    --no-owner \
    --no-privileges \
    "$BACKUP_FILE" 2>&1

# Verify row counts
echo "[$(date)] Verifying restore..."
PRIMES=$(psql -U darkreach -d darkreach -t -c "SELECT COUNT(*) FROM primes;")
WORKERS=$(psql -U darkreach -d darkreach -t -c "SELECT COUNT(*) FROM workers;")
JOBS=$(psql -U darkreach -d darkreach -t -c "SELECT COUNT(*) FROM search_jobs;")
echo "  primes: ${PRIMES}"
echo "  workers: ${WORKERS}"
echo "  search_jobs: ${JOBS}"
echo "[$(date)] Restore complete"
