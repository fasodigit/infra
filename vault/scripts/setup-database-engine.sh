#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION - Ministere du Numerique, Burkina Faso
# ============================================================
# Setup Vault Database secrets engine with per-service PostgreSQL roles.
#
# Each FASO microservice gets a dedicated role with minimum-privilege
# grants scoped to its own schema/database:
#   - auth-ms-role       -> auth_ms database, auth schema
#   - poulets-api-role   -> poulets_db database, poulets schema
#   - notifier-ms-role   -> notifier database, notifier schema
#
# Default TTL: 1h  |  Max TTL: 24h
#
# Prerequisites:
#   - Vault unsealed, VAULT_TOKEN set
#   - PostgreSQL reachable from Vault container
#
# Usage:
#   export VAULT_TOKEN=$(jq -r .root_token ~/.faso-vault-keys.json)
#   bash vault/scripts/setup-database-engine.sh
# ============================================================

set -euo pipefail

VAULT_ADDR="${VAULT_ADDR:-http://127.0.0.1:8200}"
export VAULT_ADDR

[[ -n "${VAULT_TOKEN:-}" ]] || {
  echo "ERROR: export VAULT_TOKEN first (from ~/.faso-vault-keys.json)"
  exit 1
}

PG_HOST="${PG_HOST:-postgres}"
PG_PORT="${PG_PORT:-5432}"
PG_SUPERUSER="${PG_SUPERUSER:-postgres}"
PG_SUPERPASS="${PG_SUPERPASS:-$(cat "$(dirname "${BASH_SOURCE[0]}")/../../docker/compose/secrets/postgres_password.txt" 2>/dev/null || echo 'changeme')}"

log() { echo "[faso-vault-db-engine] $*"; }

vault_api() {
  local method="$1" path="$2"
  shift 2
  curl -fsS -X "$method" \
    -H "X-Vault-Token: $VAULT_TOKEN" \
    -H 'Content-Type: application/json' \
    "$@" "${VAULT_ADDR}/v1/${path}"
}

# ---- Enable database secrets engine (idempotent) ----------------------------
log "Enabling database secrets engine ..."
vault_api POST "sys/mounts/database" \
  -d '{"type":"database","description":"FASO PostgreSQL dynamic credentials"}' \
  2>/dev/null || log "  (already enabled)"

# ---- Configure PostgreSQL connection ----------------------------------------
log "Configuring PostgreSQL connection at ${PG_HOST}:${PG_PORT} ..."
vault_api POST "database/config/faso-postgres" \
  -d "$(cat <<JSON
{
  "plugin_name": "postgresql-database-plugin",
  "allowed_roles": "auth-ms-role,poulets-api-role,notifier-ms-role",
  "connection_url": "postgresql://{{username}}:{{password}}@${PG_HOST}:${PG_PORT}/postgres?sslmode=disable",
  "username": "${PG_SUPERUSER}",
  "password": "${PG_SUPERPASS}",
  "password_policy": "",
  "verify_connection": true
}
JSON
)" >/dev/null

# ---- Create per-service roles ------------------------------------------------
create_role() {
  local role="$1" database="$2" schema="${3:-public}"
  log "Creating role: ${role} (database=${database}, schema=${schema})"

  vault_api POST "database/roles/${role}" \
    -d "$(cat <<JSON
{
  "db_name": "faso-postgres",
  "default_ttl": "1h",
  "max_ttl": "24h",
  "creation_statements": [
    "CREATE ROLE \"{{name}}\" WITH LOGIN PASSWORD '{{password}}' VALID UNTIL '{{expiration}}';",
    "GRANT CONNECT ON DATABASE ${database} TO \"{{name}}\";",
    "GRANT USAGE ON SCHEMA ${schema} TO \"{{name}}\";",
    "GRANT CREATE ON SCHEMA ${schema} TO \"{{name}}\";",
    "GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA ${schema} TO \"{{name}}\";",
    "GRANT USAGE ON ALL SEQUENCES IN SCHEMA ${schema} TO \"{{name}}\";",
    "ALTER DEFAULT PRIVILEGES IN SCHEMA ${schema} GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO \"{{name}}\";",
    "ALTER DEFAULT PRIVILEGES IN SCHEMA ${schema} GRANT USAGE ON SEQUENCES TO \"{{name}}\";"
  ],
  "revocation_statements": [
    "REASSIGN OWNED BY \"{{name}}\" TO ${PG_SUPERUSER};",
    "DROP OWNED BY \"{{name}}\";",
    "DROP ROLE IF EXISTS \"{{name}}\";"
  ],
  "renew_statements": [
    "ALTER ROLE \"{{name}}\" VALID UNTIL '{{expiration}}';"
  ]
}
JSON
)" >/dev/null
}

# auth-ms: operates on auth_ms database, public schema
create_role "auth-ms-role" "auth_ms" "public"

# poulets-api: operates on poulets_db database, public schema
create_role "poulets-api-role" "poulets_db" "public"

# notifier-ms: operates on notifier database, public schema
create_role "notifier-ms-role" "notifier" "public"

log ""
log "Database secrets engine configured."
log "  Default TTL: 1h | Max TTL: 24h"
log ""
log "Test dynamic credentials:"
log "  vault read database/creds/auth-ms-role"
log "  vault read database/creds/poulets-api-role"
log "  vault read database/creds/notifier-ms-role"
