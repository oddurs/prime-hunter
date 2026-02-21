#!/usr/bin/env bash
# Migrate darkreach database from Supabase to self-hosted PostgreSQL
# Usage: ./migrate-supabase-to-selfhosted.sh <supabase_url> <selfhosted_url>
set -euo pipefail

SUPABASE_URL="${1:?Usage: $0 <supabase_database_url> <selfhosted_database_url>}"
SELFHOSTED_URL="${2:?Usage: $0 <supabase_database_url> <selfhosted_database_url>}"
DUMP_FILE="/tmp/darkreach_migration_$(date +%Y%m%d_%H%M%S).sql"

echo "=== Darkreach: Supabase → Self-Hosted Migration ==="
echo ""
echo "Source:      ${SUPABASE_URL%%@*}@..."
echo "Destination: ${SELFHOSTED_URL%%@*}@..."
echo ""

# Step 1: Dump from Supabase
echo "[1/5] Dumping from Supabase..."
pg_dump "$SUPABASE_URL" \
    --format=plain \
    --no-owner \
    --no-privileges \
    --no-comments \
    --exclude-schema=supabase_migrations \
    --exclude-schema=extensions \
    --exclude-schema=auth \
    --exclude-schema=storage \
    --exclude-schema=realtime \
    --exclude-schema=_realtime \
    --exclude-schema=supabase_functions \
    --exclude-schema=graphql \
    --exclude-schema=graphql_public \
    --exclude-schema=pgsodium \
    --exclude-schema=vault \
    > "$DUMP_FILE" 2>&1

DUMP_SIZE=$(du -h "$DUMP_FILE" | cut -f1)
echo "   Dump complete: ${DUMP_FILE} (${DUMP_SIZE})"

# Step 2: Get source row counts for verification
echo "[2/5] Counting source rows..."
SRC_PRIMES=$(psql "$SUPABASE_URL" -t -c "SELECT COUNT(*) FROM public.primes;" | tr -d ' ')
SRC_JOBS=$(psql "$SUPABASE_URL" -t -c "SELECT COUNT(*) FROM public.search_jobs;" | tr -d ' ')
SRC_BLOCKS=$(psql "$SUPABASE_URL" -t -c "SELECT COUNT(*) FROM public.work_blocks;" | tr -d ' ')
echo "   Source: primes=${SRC_PRIMES}, jobs=${SRC_JOBS}, blocks=${SRC_BLOCKS}"

# Step 3: Restore to self-hosted
echo "[3/5] Restoring to self-hosted..."
psql "$SELFHOSTED_URL" -f "$DUMP_FILE" > /dev/null 2>&1

# Step 4: Verify row counts match
echo "[4/5] Verifying row counts..."
DST_PRIMES=$(psql "$SELFHOSTED_URL" -t -c "SELECT COUNT(*) FROM public.primes;" | tr -d ' ')
DST_JOBS=$(psql "$SELFHOSTED_URL" -t -c "SELECT COUNT(*) FROM public.search_jobs;" | tr -d ' ')
DST_BLOCKS=$(psql "$SELFHOSTED_URL" -t -c "SELECT COUNT(*) FROM public.work_blocks;" | tr -d ' ')
echo "   Destination: primes=${DST_PRIMES}, jobs=${DST_JOBS}, blocks=${DST_BLOCKS}"

ERRORS=0
if [ "$SRC_PRIMES" != "$DST_PRIMES" ]; then
    echo "   ERROR: primes mismatch (${SRC_PRIMES} vs ${DST_PRIMES})"
    ERRORS=$((ERRORS + 1))
fi
if [ "$SRC_JOBS" != "$DST_JOBS" ]; then
    echo "   ERROR: search_jobs mismatch (${SRC_JOBS} vs ${DST_JOBS})"
    ERRORS=$((ERRORS + 1))
fi
if [ "$SRC_BLOCKS" != "$DST_BLOCKS" ]; then
    echo "   ERROR: work_blocks mismatch (${SRC_BLOCKS} vs ${DST_BLOCKS})"
    ERRORS=$((ERRORS + 1))
fi

# Step 5: Cleanup
echo "[5/5] Cleanup..."
rm -f "$DUMP_FILE"

if [ "$ERRORS" -gt 0 ]; then
    echo ""
    echo "MIGRATION COMPLETED WITH ${ERRORS} ERROR(S) — verify manually before cutover"
    exit 1
else
    echo ""
    echo "MIGRATION SUCCESSFUL — update DATABASE_URL to point to self-hosted"
fi
