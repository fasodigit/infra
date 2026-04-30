#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION - Ministere du Numerique, Burkina Faso
# ============================================================
# Setup Vault AppRole auth method for FASO microservices.
#
# Creates one AppRole per service with a read-only policy scoped
# to its own KV path (faso/<service>/*) and database creds role.
#
# Prerequisites:
#   - Vault unsealed and VAULT_TOKEN set (root or admin policy)
#   - KV v2 engine mounted at "faso/"
#   - Database engine mounted at "database/"
#
# Usage:
#   export VAULT_TOKEN=$(jq -r .root_token ~/.faso-vault-keys.json)
#   bash vault/scripts/setup-approle.sh
# ============================================================

set -euo pipefail

VAULT_ADDR="${VAULT_ADDR:-http://127.0.0.1:8200}"
export VAULT_ADDR

[[ -n "${VAULT_TOKEN:-}" ]] || {
  echo "ERROR: export VAULT_TOKEN first (from ~/.faso-vault-keys.json)"
  exit 1
}

log() { echo "[faso-vault-approle] $*"; }

vault_api() {
  local method="$1" path="$2"
  shift 2
  curl -fsS -X "$method" \
    -H "X-Vault-Token: $VAULT_TOKEN" \
    -H 'Content-Type: application/json' \
    "$@" "${VAULT_ADDR}/v1/${path}"
}

# ---- Enable AppRole auth method (idempotent) --------------------------------
log "Enabling AppRole auth method ..."
vault_api POST "sys/auth/approle" \
  -d '{"type":"approle","description":"FASO service AppRole authentication"}' \
  2>/dev/null || log "  (already enabled)"

# ---- Service definitions ----------------------------------------------------
# Format: service_name:kv_path:db_role
SERVICES=(
  "auth-ms:faso/auth-ms:auth-ms-readwrite"
  "poulets-api:faso/poulets-api:poulets-api-readwrite"
  "notifier-ms:faso/notifier-ms:notifier-ms-readwrite"
)

create_policy() {
  local service="$1" kv_path="$2" db_role="$3"
  local policy_name="faso-${service}-policy"

  log "Creating policy: ${policy_name}"

  # The legacy ${db_role} positional argument now refers to the *base* role
  # name (without -runtime/-flyway suffix). The policy grants read on both
  # paths because Vault Agent injects two separate sidecar templates.
  local runtime_role="${db_role%-role}-runtime-role"
  local flyway_role="${db_role%-role}-flyway-role"

  local policy_hcl
  policy_hcl=$(cat <<HCL
# Auto-generated policy for ${service}
# Read-only access to its own KV path + database dynamic creds

# KV v2 — read secrets
path "faso/data/${service}/*" {
  capabilities = ["read", "list"]
}
path "faso/metadata/${service}/*" {
  capabilities = ["read", "list"]
}

# Database dynamic credentials — runtime (DML, long-lived HikariCP pool)
path "database/creds/${runtime_role}" {
  capabilities = ["read"]
}

# Database dynamic credentials — flyway (DDL, one-shot at boot)
path "database/creds/${flyway_role}" {
  capabilities = ["read"]
}

# Transit encryption/decryption (PII and financial keys)
path "transit/encrypt/faso-pii" {
  capabilities = ["update"]
}
path "transit/decrypt/faso-pii" {
  capabilities = ["update"]
}
path "transit/encrypt/faso-financial" {
  capabilities = ["update"]
}
path "transit/decrypt/faso-financial" {
  capabilities = ["update"]
}

# Token self-management
path "auth/token/lookup-self" {
  capabilities = ["read"]
}
path "auth/token/renew-self" {
  capabilities = ["update"]
}
HCL
)

  vault_api PUT "sys/policies/acl/${policy_name}" \
    -d "$(jq -n --arg policy "$policy_hcl" '{"policy": $policy}')" >/dev/null
}

create_approle() {
  local service="$1" policy_name="faso-${1}-policy"

  log "Creating AppRole: ${service}"

  # Create the role with policy attached
  vault_api POST "auth/approle/role/${service}" \
    -d "$(cat <<JSON
{
  "token_policies": ["default", "${policy_name}"],
  "token_ttl": "1h",
  "token_max_ttl": "4h",
  "secret_id_ttl": "720h",
  "secret_id_num_uses": 0,
  "token_num_uses": 0,
  "bind_secret_id": true
}
JSON
)" >/dev/null

  # Fetch and display the role-id (not secret — that is generated on demand)
  local role_id
  role_id=$(vault_api GET "auth/approle/role/${service}/role-id" | jq -r '.data.role_id')
  log "  role-id for ${service}: ${role_id}"

  # Generate a wrapped secret-id (response-wrapping with short TTL)
  log "  Generating wrapped secret-id (TTL=120s) ..."
  local wrap_response
  wrap_response=$(curl -fsS -X POST \
    -H "X-Vault-Token: $VAULT_TOKEN" \
    -H "X-Vault-Wrap-TTL: 120s" \
    -H 'Content-Type: application/json' \
    "${VAULT_ADDR}/v1/auth/approle/role/${service}/secret-id")
  local wrap_token
  wrap_token=$(echo "$wrap_response" | jq -r '.wrap_info.token')
  log "  wrapping-token: ${wrap_token}"
  log "  (unwrap within 120s via: VAULT_TOKEN=${wrap_token} vault unwrap)"
  echo ""
}

# ---- Main loop ---------------------------------------------------------------
for entry in "${SERVICES[@]}"; do
  IFS=':' read -r service kv_path db_role <<< "$entry"
  create_policy "$service" "$kv_path" "$db_role"
  create_approle "$service"
done

log "AppRole setup complete."
log ""
log "To login as a service (example auth-ms):"
log "  ROLE_ID=\$(vault read -field=role_id auth/approle/role/auth-ms/role-id)"
log "  SECRET_ID=\$(vault write -field=secret_id -f auth/approle/role/auth-ms/secret-id)"
log "  vault write auth/approle/login role_id=\$ROLE_ID secret_id=\$SECRET_ID"
