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
    psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
        SELECT 'CREATE DATABASE $database'
        WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = '$database')\gexec
        GRANT ALL PRIVILEGES ON DATABASE $database TO $POSTGRES_USER;
EOSQL
    echo "--- Database $database created successfully ---"
}

function create_role_with_password() {
    local role=$1
    local password=$2
    echo "--- Creating role: $role ---"
    psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
        DO \$\$
        BEGIN
            IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = '$role') THEN
                CREATE ROLE $role LOGIN PASSWORD '$password' CREATEDB;
            END IF;
        END
        \$\$;
EOSQL
    echo "--- Role $role created successfully ---"
}

function grant_db_to_role() {
    local database=$1
    local role=$2
    echo "--- Granting $database to $role ---"
    psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
        GRANT ALL PRIVILEGES ON DATABASE $database TO $role;
        ALTER DATABASE $database OWNER TO $role;
EOSQL
    # Also grant ownership of public schema so Flyway can create the audit schema
    psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$database" <<-EOSQL
        GRANT ALL ON SCHEMA public TO $role;
        ALTER SCHEMA public OWNER TO $role;
EOSQL
    echo "--- Granted $database to $role ---"
}

if [ -n "${POSTGRES_MULTIPLE_DATABASES:-}" ]; then
    echo "=== Multiple database creation requested: $POSTGRES_MULTIPLE_DATABASES ==="
    for db in $(echo "$POSTGRES_MULTIPLE_DATABASES" | tr ',' ' '); do
        create_database "$db"
    done
    echo "=== All databases created ==="
fi

# -----------------------------------------------------------------------------
# notifier-ms requires its own DB + role: POSTGRES_DB=notifier, USER=notifier.
# Password is shared (same /run/secrets/postgres_password content) so
# notifier-ms can read it from the same secret file mount.
# -----------------------------------------------------------------------------
NOTIFIER_PW=$(cat /run/secrets/postgres_password 2>/dev/null || echo "")
if [ -n "$NOTIFIER_PW" ]; then
    echo "=== Creating notifier database + role ==="
    create_database "notifier"
    create_role_with_password "notifier" "$NOTIFIER_PW"
    grant_db_to_role "notifier" "notifier"
    echo "=== notifier DB + role ready ==="
fi
