#!/usr/bin/env bash
# Set up the test database by applying all migrations.
# Usage: ./scripts/test-db-setup.sh [DATABASE_URL]
#
# Defaults to: postgres://postgres:test@localhost:5433/primehunt_test
# (matching docker-compose.test.yml)

set -euo pipefail

DB_URL="${1:-postgres://postgres:test@localhost:5433/primehunt_test}"

echo "Setting up test database at: $DB_URL"

# Apply migrations in order, skipping Supabase-specific commands
for f in supabase/migrations/*.sql; do
    echo "  Applying $(basename "$f")..."
    # Remove Supabase-specific lines (ALTER PUBLICATION, RLS, POLICY)
    sed \
        -e '/ALTER PUBLICATION/d' \
        -e '/ENABLE ROW LEVEL SECURITY/d' \
        -e '/^CREATE POLICY/d' \
        "$f" | psql "$DB_URL" -q -v ON_ERROR_STOP=0 2>&1 | grep -v "^$" || true
done

echo "Test database ready."
