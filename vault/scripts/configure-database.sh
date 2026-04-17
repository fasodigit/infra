#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Configure Vault dynamic PostgreSQL credentials for FASO microservices.
# Each service gets its own role with minimum-privilege grants.

set -euo pipefail

VAULT_ADDR="${VAULT_ADDR:-http://127.0.0.1:8200}"
export VAULT_ADDR
[[ -n "${VAULT_TOKEN:-}" ]] || { echo "ERROR: export VAULT_TOKEN first (from ~/.faso-vault-keys.json)"; exit 1; }

PG_HOST="${PG_HOST:-postgres}"
PG_PORT="${PG_PORT:-5432}"
PG_SUPERUSER="${PG_SUPERUSER:-postgres}"
PG_SUPERPASS="${PG_SUPERPASS:-$(cat "$(dirname "${BASH_SOURCE[0]}")/../../docker/compose/secrets/postgres_password.txt")}"

log() { echo "[faso-vault-db] $*"; }

# ---- Register PostgreSQL connection --------------------------------------
log "Registering PostgreSQL connection ..."
curl -fsS -X POST -H "X-Vault-Token: $VAULT_TOKEN" \
  -H 'Content-Type: application/json' \
  -d @- "${VAULT_ADDR}/v1/database/config/faso-postgres" <<JSON
{
  "plugin_name": "postgresql-database-plugin",
  "allowed_roles": "auth-ms-readwrite,poulets-api-readwrite,notifier-ms-readwrite,kratos-readwrite,keto-readwrite",
  "connection_url": "postgresql://{{username}}:{{password}}@${PG_HOST}:${PG_PORT}/postgres?sslmode=disable",
  "username": "${PG_SUPERUSER}",
  "password": "${PG_SUPERPASS}"
}
JSON

# ---- Create one role per microservice (TTL 1h, max 24h) -------------------
create_role() {
  local role="$1" db="$2"
  log "Creating role: $role (database=$db)"
  curl -fsS -X POST -H "X-Vault-Token: $VAULT_TOKEN" \
    -H 'Content-Type: application/json' \
    -d "$(cat <<JSON
{
  "db_name": "faso-postgres",
  "default_ttl": "1h",
  "max_ttl": "24h",
  "creation_statements": [
    "CREATE ROLE \"{{name}}\" WITH LOGIN PASSWORD '{{password}}' VALID UNTIL '{{expiration}}';",
    "GRANT CONNECT ON DATABASE ${db} TO \"{{name}}\";",
    "GRANT USAGE ON SCHEMA public TO \"{{name}}\";",
    "GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO \"{{name}}\";",
    "GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO \"{{name}}\";",
    "ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT,INSERT,UPDATE,DELETE ON TABLES TO \"{{name}}\";",
    "ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT USAGE ON SEQUENCES TO \"{{name}}\";"
  ],
  "revocation_statements": [
    "REASSIGN OWNED BY \"{{name}}\" TO ${PG_SUPERUSER};",
    "DROP OWNED BY \"{{name}}\";",
    "DROP ROLE IF EXISTS \"{{name}}\";"
  ]
}
JSON
)" "${VAULT_ADDR}/v1/database/roles/${role}" >/dev/null
}

create_role "auth-ms-readwrite"     "auth_ms"
create_role "poulets-api-readwrite" "poulets"
create_role "notifier-ms-readwrite" "notifier_ms"
create_role "kratos-readwrite"      "kratos"
create_role "keto-readwrite"        "keto"

log "✓ Dynamic database credentials configured."
log "  Test:   vault read database/creds/auth-ms-readwrite"
