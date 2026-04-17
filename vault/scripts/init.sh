#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Bootstrap Vault for FASO DIGITALISATION: init, unseal, enable engines, upload policies.
# Idempotent — safe to re-run; skips steps already done.

set -euo pipefail

VAULT_ADDR="${VAULT_ADDR:-http://127.0.0.1:8200}"
KEYS_FILE="${HOME}/.faso-vault-keys.json"
POLICIES_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../policies" && pwd)"

export VAULT_ADDR

log() { echo "[faso-vault-init] $*"; }

# Wait for Vault to respond (sealed state is fine).
log "Waiting for Vault at $VAULT_ADDR ..."
for i in $(seq 1 30); do
  if curl -fsS "${VAULT_ADDR}/v1/sys/health?uninitcode=200&sealedcode=200" >/dev/null 2>&1; then
    break
  fi
  sleep 2
done

# ---- INIT (5 keys, threshold 3) ------------------------------------------
INIT_STATUS=$(curl -fsS "${VAULT_ADDR}/v1/sys/init" | jq -r '.initialized')
if [[ "$INIT_STATUS" == "false" ]]; then
  log "Initialising Vault ..."
  INIT_JSON=$(curl -fsS -X POST "${VAULT_ADDR}/v1/sys/init" \
    -H 'Content-Type: application/json' \
    -d '{"secret_shares":5,"secret_threshold":3}')
  echo "$INIT_JSON" > "$KEYS_FILE"
  chmod 600 "$KEYS_FILE"
  log "Init keys + root token saved to $KEYS_FILE (chmod 600)"
else
  log "Vault already initialised — reusing $KEYS_FILE"
  [[ -f "$KEYS_FILE" ]] || { log "ERROR: $KEYS_FILE missing but Vault is initialised"; exit 1; }
fi

ROOT_TOKEN=$(jq -r '.root_token' "$KEYS_FILE")
UNSEAL_KEY_1=$(jq -r '.keys[0]' "$KEYS_FILE")
UNSEAL_KEY_2=$(jq -r '.keys[1]' "$KEYS_FILE")
UNSEAL_KEY_3=$(jq -r '.keys[2]' "$KEYS_FILE")

# ---- UNSEAL ---------------------------------------------------------------
SEALED=$(curl -fsS "${VAULT_ADDR}/v1/sys/seal-status" | jq -r '.sealed')
if [[ "$SEALED" == "true" ]]; then
  log "Unsealing with 3 of 5 keys ..."
  for k in "$UNSEAL_KEY_1" "$UNSEAL_KEY_2" "$UNSEAL_KEY_3"; do
    curl -fsS -X POST "${VAULT_ADDR}/v1/sys/unseal" \
      -H 'Content-Type: application/json' \
      -d "{\"key\":\"$k\"}" >/dev/null
  done
  log "Unsealed."
else
  log "Vault already unsealed."
fi

# ---- Authenticate with root token (bootstrap only) ------------------------
export VAULT_TOKEN="$ROOT_TOKEN"

vault_curl() {
  local method="$1" path="$2" data="${3:-}"
  if [[ -n "$data" ]]; then
    curl -fsS -X "$method" -H "X-Vault-Token: $VAULT_TOKEN" \
      -H 'Content-Type: application/json' \
      -d "$data" "${VAULT_ADDR}/v1/${path}"
  else
    curl -fsS -X "$method" -H "X-Vault-Token: $VAULT_TOKEN" \
      "${VAULT_ADDR}/v1/${path}"
  fi
}

enable_engine() {
  local path="$1" type="$2" options_json="${3:-{}}"
  if vault_curl GET "sys/mounts/${path}" >/dev/null 2>&1; then
    log "Engine ${path}/ already enabled (${type})"
  else
    log "Enabling ${type} engine at ${path}/"
    vault_curl POST "sys/mounts/${path}" \
      "{\"type\":\"${type}\",\"options\":${options_json}}" >/dev/null
  fi
}

enable_auth() {
  local path="$1" type="$2"
  if vault_curl GET "sys/auth/${path}" >/dev/null 2>&1; then
    log "Auth method ${path}/ already enabled (${type})"
  else
    log "Enabling auth method ${type} at ${path}/"
    vault_curl POST "sys/auth/${path}" \
      "{\"type\":\"${type}\"}" >/dev/null
  fi
}

# ---- Secrets engines ------------------------------------------------------
enable_engine "faso"     "kv"       '{"version":"2"}'
enable_engine "database" "database"
enable_engine "transit"  "transit"
enable_engine "pki"      "pki"      '{"default_lease_ttl":"8760h","max_lease_ttl":"87600h"}'

# ---- Transit keys (auth-ms JWT encryption, kaya persistence) --------------
for key in jwt-key pii-key persistence-key; do
  if ! vault_curl GET "transit/keys/${key}" >/dev/null 2>&1; then
    log "Creating transit key: ${key}"
    vault_curl POST "transit/keys/${key}" '{"type":"aes256-gcm96","exportable":false}' >/dev/null
  fi
done

# ---- Auth methods ---------------------------------------------------------
enable_auth "approle"    "approle"
# enable_auth "kubernetes" "kubernetes"   # uncomment when deploying to K8s
# enable_auth "jwt"        "jwt"          # uncomment for GitHub OIDC

# ---- Upload policies ------------------------------------------------------
for policy_file in "$POLICIES_DIR"/*.hcl; do
  name=$(basename "$policy_file" .hcl)
  log "Uploading policy: $name"
  # Vault policy API expects the HCL in a JSON-wrapped "policy" field.
  jq -Rs '{policy: .}' "$policy_file" | \
    curl -fsS -X POST -H "X-Vault-Token: $VAULT_TOKEN" \
      -H 'Content-Type: application/json' \
      --data @- "${VAULT_ADDR}/v1/sys/policies/acl/${name}" >/dev/null
done

# ---- AppRoles for each service --------------------------------------------
for svc in kaya armageddon auth-ms poulets-api notifier-ms bff kratos keto growthbook; do
  role_path="auth/approle/role/faso-${svc}"
  if ! vault_curl GET "${role_path}" >/dev/null 2>&1; then
    log "Creating AppRole for ${svc}"
    vault_curl POST "${role_path}" \
      "{\"token_policies\":\"faso-${svc}-read\",\"token_ttl\":\"1h\",\"token_max_ttl\":\"24h\"}" >/dev/null
  fi
done

# ---- Audit device ---------------------------------------------------------
if ! vault_curl GET "sys/audit" | jq -e '."file/"' >/dev/null 2>&1; then
  log "Enabling file audit device at /vault/logs/audit.log"
  vault_curl POST "sys/audit/file" \
    '{"type":"file","options":{"file_path":"/vault/logs/audit.log"}}' >/dev/null || true
fi

log "✓ Vault bootstrap complete."
log "   Root token: (see $KEYS_FILE — revoke after generating your own admin token)"
log "   UI:         ${VAULT_ADDR}/ui"
log ""
log "Next step: bash $(dirname "${BASH_SOURCE[0]}")/seed-secrets.sh"
