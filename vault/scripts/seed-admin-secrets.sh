#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# =============================================================================
# seed-admin-secrets.sh — Phase 4.b admin-UI / Stream D2 (Configs infra)
#
# Pousse dans Vault (KV v2 sur le mount "faso/") les secrets nécessaires
# au sous-système admin de auth-ms :
#   - HMAC pour digestion OTP
#   - Master secret TOTP (chiffrement seed AES-GCM)
#   - Pepper pour codes de récupération
#   - Master key break-glass
#   - WebAuthn RP id
#   - Bootstrap Redpanda
#   - Token interne Kratos → auth-ms (webhook auth)
#
# Idempotent — un re-run écrase les valeurs (acceptable pour bootstrap dev).
# Pour production : utiliser Vault transit + rotation ; ne JAMAIS relancer
# en prod sans plan de rotation.
# =============================================================================

set -euo pipefail

VAULT_ADDR="${VAULT_ADDR:-http://127.0.0.1:8200}"
export VAULT_ADDR

log() { echo "[faso-vault-admin-seed] $*"; }
err() { echo "[faso-vault-admin-seed] ERROR: $*" >&2; }

# ---- Sanity checks --------------------------------------------------------
if ! command -v vault >/dev/null 2>&1; then
  err "Le binaire 'vault' n'est pas dans le PATH."
  err "Installe : https://developer.hashicorp.com/vault/downloads"
  exit 1
fi

if ! command -v openssl >/dev/null 2>&1; then
  err "Le binaire 'openssl' est requis pour générer les secrets."
  exit 1
fi

# Probe Vault.
if ! vault status >/dev/null 2>&1; then
  err "Vault injoignable ou scellé sur ${VAULT_ADDR}."
  err "Lance : podman-compose -f INFRA/vault/podman-compose.vault.yml up -d"
  err "Puis  : bash INFRA/vault/scripts/init.sh"
  exit 1
fi

if [[ -z "${VAULT_TOKEN:-}" ]]; then
  KEYS_FILE="${HOME}/.faso-vault-keys.json"
  if [[ -f "$KEYS_FILE" ]] && command -v jq >/dev/null 2>&1; then
    VAULT_TOKEN="$(jq -r '.root_token' "$KEYS_FILE")"
    export VAULT_TOKEN
    log "VAULT_TOKEN récupéré depuis $KEYS_FILE"
  else
    err "VAULT_TOKEN non défini et $KEYS_FILE introuvable."
    err "Exporte VAULT_TOKEN ou exécute init.sh."
    exit 1
  fi
fi

# ---- Helpers --------------------------------------------------------------
rand_hex() {
  local bytes="${1:-32}"
  openssl rand -hex "$bytes"
}

put() {
  local path="$1"
  local key="$2"
  local value="$3"
  log "  vault kv put faso/${path} ${key}=****"
  vault kv put "faso/${path}" "${key}=${value}" >/dev/null
}

# ---- Seeds ----------------------------------------------------------------
log "Seed des secrets admin auth-ms ..."
put "auth-ms/otp-hmac-key"           "value" "$(rand_hex 32)"
put "auth-ms/totp-master-secret"     "value" "$(rand_hex 32)"
put "auth-ms/recovery-codes-pepper"  "value" "$(rand_hex 32)"
put "auth-ms/break-glass-master-key" "value" "$(rand_hex 32)"
put "auth-ms/webauthn-rp-id"         "value" "faso.bf"
put "auth-ms/redpanda-bootstrap"     "value" "redpanda:9092"
put "auth-ms/kratos-internal-token"  "value" "$(rand_hex 48)"
# Phase 4.b.4 — Magic-link channel-binding (HMAC-SHA256, signup admin + recovery self)
put "auth-ms/magic-link-hmac-key"    "value" "$(rand_hex 32)"
# Phase 4.b.6 — MaxMind GeoLite2 license key (free signup at maxmind.com/en/geolite2/signup).
# Placeholder seed — ops MUST replace with the real key BEFORE running
# scripts/download-geolite2.sh (cf. RUNBOOK §risk-scoring). Without a real key,
# GeoLite2-City.mmdb cannot be downloaded and the geo signal stays neutral
# (fail-open — RiskScoringService accepts missing DB by design).
put "auth-ms/maxmind-license-key"    "value" "__REPLACE_WITH_MAXMIND_KEY__"

# ---- TERROIR P0.B — secrets sous faso/terroir/ ----------------------------
# Idempotent : openssl rand est appelé à chaque run, ce qui régénère les
# valeurs eas-update-secret / apk-keystore-password — pour la prod, mettre
# en place une rotation contrôlée (cf. RUNBOOK-VAULT-TRANSIT-PKI.md).
log ""
log "Seed des secrets TERROIR ..."
put "terroir/eori-default-exporter-cert" "value" "placeholder-fill-on-real-exporter-onboarding"
put "terroir/maxmind-license-key"        "value" "${MAXMIND_LICENSE_KEY:-disabled}"
put "terroir/eas-update-secret"          "value" "$(openssl rand -hex 32)"
put "terroir/apk-keystore-password"      "value" "$(openssl rand -base64 32)"

log "OK — 9 secrets admin écrits sous faso/auth-ms/ + 4 secrets TERROIR sous faso/terroir/."
log ""
log "Vérification :"
log "  vault kv list faso/auth-ms/"
log "  vault kv list faso/terroir/"
log "  vault kv get  faso/auth-ms/otp-hmac-key"
