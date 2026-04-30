#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# =============================================================================
# seed-crypto-peppers.sh — Phase 4.b.3 (Argon2id + HMAC pepper)
#
# Pousse dans Vault (KV v2 sur le mount "faso/") les trois peppers utilisés
# par CryptographicHashService (auth-ms) pour le pattern HMAC-SHA256 + Argon2id :
#
#   faso/auth-ms/password-pepper-v1   — pepper hash mots de passe utilisateur
#   faso/auth-ms/otp-pepper-v1        — pepper OTP 8 chiffres
#   faso/auth-ms/recovery-pepper-v1   — pepper codes de récupération XXXX-XXXX
#
# Chaque pepper est 32 octets random (hex 64 chars). Rotation : créer
# password-pepper-v2 / otp-pepper-v2 / recovery-pepper-v2 et bumper
# admin.crypto.pepper-version dans application.yml ; les anciens peppers
# restent en lecture le temps de la migration lazy-rehash.
#
# IDEMPOTENT — un re-run écrase les valeurs. NE PAS LANCER en prod sans plan
# de rotation : tous les hashs OTP / recovery codes existants seraient
# invalidés.
# =============================================================================

set -euo pipefail

VAULT_ADDR="${VAULT_ADDR:-http://127.0.0.1:8200}"
export VAULT_ADDR

log() { echo "[faso-vault-crypto-pepper-seed] $*"; }
err() { echo "[faso-vault-crypto-pepper-seed] ERROR: $*" >&2; }

# ---- Sanity checks --------------------------------------------------------
if ! command -v vault >/dev/null 2>&1; then
  err "Le binaire 'vault' n'est pas dans le PATH."
  err "Installe : https://developer.hashicorp.com/vault/downloads"
  exit 1
fi
if ! command -v openssl >/dev/null 2>&1; then
  err "Le binaire 'openssl' est requis pour générer les peppers."
  exit 1
fi

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

# ---- Seeds ----------------------------------------------------------------
log "Seed des peppers crypto v1 (32 octets each) sous faso/auth-ms/ ..."

vault kv put faso/auth-ms/password-pepper-v1 value="$(rand_hex 32)" >/dev/null
log "  ✓ faso/auth-ms/password-pepper-v1 (HMAC pour Argon2id passwords)"

vault kv put faso/auth-ms/otp-pepper-v1 value="$(rand_hex 32)" >/dev/null
log "  ✓ faso/auth-ms/otp-pepper-v1 (HMAC pour OTP 8 chiffres)"

vault kv put faso/auth-ms/recovery-pepper-v1 value="$(rand_hex 32)" >/dev/null
log "  ✓ faso/auth-ms/recovery-pepper-v1 (HMAC pour codes récupération)"

log "OK — 3 peppers crypto écrits."
log ""
log "Vérification :"
log "  vault kv list faso/auth-ms/"
log "  vault kv get  faso/auth-ms/password-pepper-v1"
log ""
log "Wiring auth-ms (application-vault.yml) : les peppers seront mappés"
log "automatiquement vers les properties admin.crypto.{password,otp,recovery}-pepper"
log "via Spring Cloud Vault (default-context: auth-ms)."
