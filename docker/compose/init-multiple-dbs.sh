#!/bin/bash
# =============================================================================
# Initialize multiple PostgreSQL databases for FASO DIGITALISATION
# =============================================================================
# This script is executed by the official postgres Docker image on first run.
# It creates separate databases for each service that needs its own schema.
# =============================================================================

set -e
set -u

function create_database() {
    local database=$1
    echo "--- Creating database: $database ---"
    psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" <<-EOSQL
        SELECT 'CREATE DATABASE $database'
        WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = '$database')\gexec
        GRANT ALL PRIVILEGES ON DATABASE $database TO $POSTGRES_USER;
EOSQL
    echo "--- Database $database created successfully ---"
}

if [ -n "${POSTGRES_MULTIPLE_DATABASES:-}" ]; then
    echo "=== Multiple database creation requested: $POSTGRES_MULTIPLE_DATABASES ==="
    for db in $(echo "$POSTGRES_MULTIPLE_DATABASES" | tr ',' ' '); do
        create_database "$db"
    done
    echo "=== All databases created ==="
fi
