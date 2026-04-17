#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Seed Vault KV v2 with secrets from Docker compose secrets/ directory.
# Idempotent — running twice overwrites existing values (acceptable for bootstrap).

set -euo pipefail

VAULT_ADDR="${VAULT_ADDR:-http://127.0.0.1:8200}"
SECRETS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../docker/compose/secrets" && pwd)"

if [[ -z "${VAULT_TOKEN:-}" ]]; then
  KEYS_FILE="${HOME}/.faso-vault-keys.json"
  [[ -f "$KEYS_FILE" ]] || { echo "ERROR: set VAULT_TOKEN or run init.sh first"; exit 1; }
  VAULT_TOKEN=$(jq -r '.root_token' "$KEYS_FILE")
  export VAULT_TOKEN
fi
export VAULT_ADDR

log() { echo "[faso-vault-seed] $*"; }

kv_put() {
  local path="$1" key="$2" value="$3"
  log "  PUT faso/${path} ${key}=****"
  curl -fsS -X POST -H "X-Vault-Token: $VAULT_TOKEN" \
    -H 'Content-Type: application/json' \
    -d "{\"data\":{\"${key}\":\"${value}\"}}" \
    "${VAULT_ADDR}/v1/faso/data/${path}" >/dev/null
}

kv_put_file() {
  local path="$1" key="$2" file="$3"
  if [[ -f "$file" ]]; then
    local value
    value=$(tr -d '\n\r' < "$file")
    kv_put "$path" "$key" "$value"
  else
    log "  SKIP faso/${path} (file missing: $file)"
  fi
}

# ---- ORY secrets (from docker/compose/secrets/) ---------------------------
log "Seeding ORY secrets ..."
kv_put_file "ory/kratos"    "cookie_secret"   "$SECRETS_DIR/kratos_cookie_secret.txt"
kv_put_file "ory/kratos"    "cipher_secret"   "$SECRETS_DIR/kratos_cipher_secret.txt"
kv_put_file "ory/keto"      "secret"          "$SECRETS_DIR/keto_secret.txt"
kv_put_file "postgres"      "password"        "$SECRETS_DIR/postgres_password.txt"

# ---- KAYA -----------------------------------------------------------------
log "Seeding KAYA secrets ..."
kv_put "kaya/auth"          "password"        "$(openssl rand -base64 32 | tr -d '=+/')"
kv_put "kaya/functions"     "signing_key"     "$(openssl rand -base64 48 | tr -d '=+/')"

# ---- ARMAGEDDON -----------------------------------------------------------
log "Seeding ARMAGEDDON secrets ..."
kv_put "armageddon/admin"   "token"                   "$(openssl rand -base64 32 | tr -d '=+/')"
kv_put "armageddon/github"  "webhook_secret"          "$(openssl rand -base64 40 | tr -d '=+/')"

# ---- auth-ms --------------------------------------------------------------
log "Seeding auth-ms secrets ..."
kv_put "auth-ms/jwt"        "encryption_key_b64"      "$(openssl rand -base64 32)"
kv_put "auth-ms/grpc"       "service_token"           "$(openssl rand -base64 32 | tr -d '=+/')"

# ---- poulets-api ----------------------------------------------------------
log "Seeding poulets-api secrets ..."
kv_put "poulets-api/grpc"   "service_token"           "$(openssl rand -base64 32 | tr -d '=+/')"

# ---- notifier-ms ----------------------------------------------------------
log "Seeding notifier-ms secrets ..."
kv_put "notifier-ms/smtp"   "username"                "${SMTP_USERNAME:-mailhog}"
kv_put "notifier-ms/smtp"   "password"                "${SMTP_PASSWORD:-empty}"

# ---- BFF ------------------------------------------------------------------
log "Seeding BFF secrets ..."
kv_put "bff/session"        "cookie_secret"           "$(openssl rand -base64 48 | tr -d '=+/')"
kv_put "bff/nextauth"       "secret"                  "$(openssl rand -base64 48 | tr -d '=+/')"

# ---- GrowthBook -----------------------------------------------------------
log "Seeding GrowthBook secrets ..."
kv_put_file "growthbook"    "jwt_secret"      "$SECRETS_DIR/growthbook_jwt_secret.txt" 2>/dev/null || \
  kv_put "growthbook" "jwt_secret" "$(openssl rand -base64 32 | tr -d '=+/')"
kv_put_file "growthbook"    "encryption_key"  "$SECRETS_DIR/growthbook_encryption_key.txt" 2>/dev/null || \
  kv_put "growthbook" "encryption_key" "$(openssl rand -base64 32 | tr -d '=+/')"

log "✓ Seeded $(curl -fsS -H "X-Vault-Token: $VAULT_TOKEN" \
  "${VAULT_ADDR}/v1/faso/metadata?list=true" 2>/dev/null | jq -r '.data.keys | length') top-level paths."
log ""
log "Inspect:"
log "  vault kv list faso/"
log "  vault kv get faso/kaya/auth"
