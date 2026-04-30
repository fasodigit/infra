#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION - Ministere du Numerique, Burkina Faso
# ============================================================
# Setup Vault Database secrets engine with per-service PostgreSQL roles.
#
# Each FASO microservice gets TWO roles for least-privilege:
#
#   <service>-runtime-role  -> DML only (SELECT/INSERT/UPDATE/DELETE)
#                              Default TTL 1h, used by the live service for
#                              every connection in the HikariCP pool. Cannot
#                              CREATE/ALTER/DROP tables — schema is frozen
#                              at runtime.
#
#   <service>-flyway-role   -> DDL + DML (one-shot migrations)
#                              Default TTL 30m, used ONLY by the
#                              `mvn flyway:migrate` step in the deploy
#                              pipeline (or the embedded Flyway at boot when
#                              SPRING_FLYWAY_USER is the dynamic-creds
#                              dynamic-creds path). Has CREATE/ALTER on the
#                              schema.
#
# Services:
#   - auth-ms-{runtime,flyway}-role     -> auth_ms DB
#   - poulets-api-{runtime,flyway}-role -> poulets_db DB
#   - notifier-ms-{runtime,flyway}-role -> notifier DB
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
  "allowed_roles": "auth-ms-runtime-role,auth-ms-flyway-role,poulets-api-runtime-role,poulets-api-flyway-role,notifier-ms-runtime-role,notifier-ms-flyway-role",
  "connection_url": "postgresql://{{username}}:{{password}}@${PG_HOST}:${PG_PORT}/postgres?sslmode=disable",
  "username": "${PG_SUPERUSER}",
  "password": "${PG_SUPERPASS}",
  "password_policy": "",
  "verify_connection": true
}
JSON
)" >/dev/null

# ---- Create runtime role (DML only) ------------------------------------------
# Used by the running service via HikariCP. NO DDL grants — schema is frozen
# during runtime. This blocks table creation/dropping by a compromised app.
create_runtime_role() {
  local role="$1" database="$2" schema="${3:-public}"
  log "Creating runtime role: ${role} (database=${database}, schema=${schema})"

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
    "GRANT USAGE ON SCHEMA audit TO \"{{name}}\";",
    "GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA ${schema} TO \"{{name}}\";",
    "GRANT INSERT ON ALL TABLES IN SCHEMA audit TO \"{{name}}\";",
    "GRANT SELECT ON ALL TABLES IN SCHEMA audit TO \"{{name}}\";",
    "GRANT USAGE ON ALL SEQUENCES IN SCHEMA ${schema} TO \"{{name}}\";",
    "GRANT USAGE ON ALL SEQUENCES IN SCHEMA audit TO \"{{name}}\";",
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

# ---- Create flyway role (DDL + DML, one-shot) --------------------------------
# Used ONLY by the deploy pipeline's `mvn flyway:migrate` (or the embedded
# Flyway at boot when SPRING_FLYWAY_USER is set to the dynamic-creds path).
# Has CREATE/ALTER on the schema so migrations can run. Short TTL (30m).
create_flyway_role() {
  local role="$1" database="$2" schema="${3:-public}"
  log "Creating flyway role: ${role} (database=${database}, schema=${schema})"

  vault_api POST "database/roles/${role}" \
    -d "$(cat <<JSON
{
  "db_name": "faso-postgres",
  "default_ttl": "30m",
  "max_ttl": "1h",
  "creation_statements": [
    "CREATE ROLE \"{{name}}\" WITH LOGIN PASSWORD '{{password}}' VALID UNTIL '{{expiration}}';",
    "GRANT CONNECT ON DATABASE ${database} TO \"{{name}}\";",
    "GRANT USAGE, CREATE ON SCHEMA ${schema} TO \"{{name}}\";",
    "GRANT CREATE ON DATABASE ${database} TO \"{{name}}\";",
    "GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA ${schema} TO \"{{name}}\";",
    "GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA ${schema} TO \"{{name}}\";",
    "ALTER DEFAULT PRIVILEGES IN SCHEMA ${schema} GRANT ALL PRIVILEGES ON TABLES TO \"{{name}}\";",
    "ALTER DEFAULT PRIVILEGES IN SCHEMA ${schema} GRANT ALL PRIVILEGES ON SEQUENCES TO \"{{name}}\";"
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

# ---- Provision both roles per service ----------------------------------------
# auth-ms
create_runtime_role "auth-ms-runtime-role" "auth_ms" "public"
create_flyway_role  "auth-ms-flyway-role"  "auth_ms" "public"

# poulets-api
create_runtime_role "poulets-api-runtime-role" "poulets_db" "public"
create_flyway_role  "poulets-api-flyway-role"  "poulets_db" "public"

# notifier-ms
create_runtime_role "notifier-ms-runtime-role" "notifier" "public"
create_flyway_role  "notifier-ms-flyway-role"  "notifier" "public"

log ""
log "Database secrets engine configured."
log ""
log "Runtime roles  : default TTL 1h  | DML only       (used by HikariCP)"
log "Flyway  roles  : default TTL 30m | DDL + DML       (used at deploy)"
log ""
log "Application config — point Spring at TWO dynamic creds paths:"
log "  spring.datasource.username/password         -> database/creds/<svc>-runtime-role"
log "  spring.flyway.user/password                 -> database/creds/<svc>-flyway-role"
log ""
log "Test:"
log "  vault read database/creds/auth-ms-runtime-role"
log "  vault read database/creds/auth-ms-flyway-role"
