#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION - Ministere du Numerique, Burkina Faso
# =============================================================================
# configure-transit.sh — TERROIR P0.B (Vault Transit envelope encryption)
#
# Active le secret engine `transit` (no-op si deja monte) et provisionne :
#   - terroir-pii-master   AES-256-GCM96, derived=true, exportable=false
#                          (KEK pour PII columns: NIN/CNIB, MSISDN, GPS, etc.)
#                          Auto-rotation : 90 jours (2160h)
#   - terroir-dek-master   AES-256-GCM96, derived=false, exportable=false
#                          (KEK pour wrap des DEK applicatifs photos/biometrie)
#                          Auto-rotation : 90 jours (2160h)
#
# Test final : encrypt -> decrypt round-trip "test-pii" sur terroir-pii-master
# avec context tenant=t_pilot|field=nin (derivation HKDF Vault).
#
# Audit : enable file audit device sur /vault/audit/transit.log (skip si exist).
#
# IDEMPOTENT — re-run safe ; n'ecrase ni les cles existantes ni la config audit.
# Cf. ADR-005 §"Schema envelope encryption" + ULTRAPLAN §4 P0.2.
# =============================================================================

set -euo pipefail

VAULT_ADDR="${VAULT_ADDR:-http://127.0.0.1:8200}"
export VAULT_ADDR

log()  { echo "[terroir-transit] $*"; }
warn() { echo "[terroir-transit] WARN: $*" >&2; }
err()  { echo "[terroir-transit] ERROR: $*" >&2; }

# ---- Sanity checks --------------------------------------------------------
command -v vault   >/dev/null 2>&1 || { err "binaire 'vault' introuvable"; exit 1; }
command -v openssl >/dev/null 2>&1 || { err "binaire 'openssl' introuvable"; exit 1; }
command -v jq      >/dev/null 2>&1 || { err "binaire 'jq' introuvable"; exit 1; }
command -v curl    >/dev/null 2>&1 || { err "binaire 'curl' introuvable"; exit 1; }
command -v base64  >/dev/null 2>&1 || { err "binaire 'base64' introuvable"; exit 1; }

if ! vault status >/dev/null 2>&1; then
  err "Vault injoignable ou scelle sur ${VAULT_ADDR}."
  err "Lance d'abord : podman-compose -f INFRA/vault/podman-compose.vault.yml up -d"
  err "Puis         : bash INFRA/vault/scripts/init.sh"
  exit 1
fi

if [[ -z "${VAULT_TOKEN:-}" ]]; then
  KEYS_FILE="${HOME}/.faso-vault-keys.json"
  if [[ -f "$KEYS_FILE" ]]; then
    VAULT_TOKEN="$(jq -r '.root_token' "$KEYS_FILE")"
    export VAULT_TOKEN
    log "VAULT_TOKEN recupere depuis $KEYS_FILE"
  else
    err "VAULT_TOKEN non defini et $KEYS_FILE introuvable."
    err "Exporte VAULT_TOKEN ou execute init.sh."
    exit 1
  fi
fi

# Verifier le token (lookup-self)
if ! vault token lookup >/dev/null 2>&1; then
  err "VAULT_TOKEN invalide ou expire."
  exit 1
fi

# ---- Helpers --------------------------------------------------------------
vault_api() {
  local method="$1" path="$2"
  shift 2
  curl -fsS -X "$method" \
    -H "X-Vault-Token: $VAULT_TOKEN" \
    -H 'Content-Type: application/json' \
    "$@" "${VAULT_ADDR}/v1/${path}"
}

engine_enabled() {
  local mount_path="$1"
  vault_api GET "sys/mounts" 2>/dev/null \
    | jq -e --arg p "${mount_path}/" '.[$p] // .data[$p] // empty' >/dev/null 2>&1
}

key_exists() {
  local mount="$1" key="$2"
  vault_api GET "${mount}/keys/${key}" >/dev/null 2>&1
}

# ---- 1. Activation secret engine `transit` --------------------------------
if engine_enabled "transit"; then
  log "Secret engine transit/ deja active (skip)"
else
  log "Activation secret engine transit/ ..."
  vault_api POST "sys/mounts/transit" \
    -d '{"type":"transit","description":"FASO/TERROIR encryption-as-a-service (PII envelope, DEK wrap)"}' \
    >/dev/null
fi

# ---- 2. Cle TERROIR PII master --------------------------------------------
# derived=true : derivation HKDF par context (tenant|field) -> isolation crypto
#                tenant != tenant impossible meme si meme plaintext.
# exportable=false : aucun export plaintext de la cle (compliance).
# allow_plaintext_backup=false : backup contient uniquement le ciphertext metadonnee.
PII_KEY="terroir-pii-master"
if key_exists "transit" "$PII_KEY"; then
  log "Cle transit/${PII_KEY} deja existante (skip create)"
else
  log "Creation cle transit/${PII_KEY} (AES-256-GCM96, derived=true) ..."
  vault_api POST "transit/keys/${PII_KEY}" -d '{
    "type": "aes256-gcm96",
    "derived": true,
    "exportable": false,
    "allow_plaintext_backup": false
  }' >/dev/null
fi

log "Configuration rotation 90 jours (2160h) sur ${PII_KEY} ..."
vault_api POST "transit/keys/${PII_KEY}/config" -d '{
  "min_decryption_version": 1,
  "min_encryption_version": 0,
  "deletion_allowed": false,
  "auto_rotate_period": "2160h"
}' >/dev/null

# ---- 3. Cle TERROIR DEK master (KEK pour wrap DEK applicatifs) -----------
# derived=false : meme cle pour tous les wraps (DEK photo/biometrie).
DEK_KEY="terroir-dek-master"
if key_exists "transit" "$DEK_KEY"; then
  log "Cle transit/${DEK_KEY} deja existante (skip create)"
else
  log "Creation cle transit/${DEK_KEY} (AES-256-GCM96, KEK envelope) ..."
  vault_api POST "transit/keys/${DEK_KEY}" -d '{
    "type": "aes256-gcm96",
    "derived": false,
    "exportable": false,
    "allow_plaintext_backup": false
  }' >/dev/null
fi

log "Configuration rotation 90 jours (2160h) sur ${DEK_KEY} ..."
vault_api POST "transit/keys/${DEK_KEY}/config" -d '{
  "min_decryption_version": 1,
  "min_encryption_version": 0,
  "deletion_allowed": false,
  "auto_rotate_period": "2160h"
}' >/dev/null

# ---- 4. Test round-trip encrypt/decrypt -----------------------------------
log "Test round-trip encrypt/decrypt sur ${PII_KEY} ..."

PLAINTEXT_RAW="test-pii"
PLAINTEXT_B64="$(printf '%s' "$PLAINTEXT_RAW" | base64 -w0 2>/dev/null || printf '%s' "$PLAINTEXT_RAW" | base64)"
CONTEXT_RAW="tenant=t_pilot|field=nin"
CONTEXT_B64="$(printf '%s' "$CONTEXT_RAW" | base64 -w0 2>/dev/null || printf '%s' "$CONTEXT_RAW" | base64)"

ENCRYPT_RESP="$(vault_api POST "transit/encrypt/${PII_KEY}" \
  -d "{\"plaintext\":\"${PLAINTEXT_B64}\",\"context\":\"${CONTEXT_B64}\"}")"

CIPHERTEXT="$(echo "$ENCRYPT_RESP" | jq -r '.data.ciphertext')"
if [[ -z "$CIPHERTEXT" || "$CIPHERTEXT" == "null" ]]; then
  err "round-trip KO : encrypt n'a pas retourne de ciphertext"
  err "reponse Vault : $ENCRYPT_RESP"
  exit 1
fi
log "  encrypt OK (ciphertext = ${CIPHERTEXT:0:32}...)"

DECRYPT_RESP="$(vault_api POST "transit/decrypt/${PII_KEY}" \
  -d "{\"ciphertext\":\"${CIPHERTEXT}\",\"context\":\"${CONTEXT_B64}\"}")"

DECRYPTED_B64="$(echo "$DECRYPT_RESP" | jq -r '.data.plaintext')"
DECRYPTED="$(printf '%s' "$DECRYPTED_B64" | base64 -d 2>/dev/null || printf '%s' "$DECRYPTED_B64" | base64 --decode)"

if [[ "$DECRYPTED" == "$PLAINTEXT_RAW" ]]; then
  log "  decrypt OK -> '$DECRYPTED'"
  log "  ROUND-TRIP : OK"
else
  err "round-trip KO : decrypted='$DECRYPTED' vs expected='$PLAINTEXT_RAW'"
  exit 1
fi

# ---- 5. Audit hook (file device dedie transit) ----------------------------
log "Verification audit device transit-audit ..."
if vault_api GET "sys/audit" 2>/dev/null | jq -e '."transit-audit/"' >/dev/null 2>&1; then
  log "Audit device transit-audit/ deja active (skip)"
else
  log "Activation audit device file -> /vault/audit/transit.log ..."
  if vault_api POST "sys/audit/transit-audit" -d '{
    "type": "file",
    "options": {"file_path": "/vault/audit/transit.log"}
  }' >/dev/null 2>&1; then
    log "  audit transit-audit/ active"
  else
    warn "echec activation audit (peut-etre /vault/audit/ non monte)"
    warn "fallback : vault audit enable -path=transit-audit file file_path=/tmp/vault-transit.log"
  fi
fi

# ---- 6. Recap -------------------------------------------------------------
log ""
log "TERROIR Vault Transit configure :"
log "  - transit/keys/${PII_KEY} (PII KEK, derived, rotation 90j)"
log "  - transit/keys/${DEK_KEY} (DEK wrap KEK, rotation 90j)"
log ""
log "Inspect :"
log "  vault list transit/keys"
log "  vault read transit/keys/${PII_KEY}"
log ""
log "Encrypt avec context (recommande pour PII) :"
log "  vault write transit/encrypt/${PII_KEY} \\"
log "    plaintext=\$(echo -n 'jean.dupont@example.bf' | base64) \\"
log "    context=\$(echo -n 'tenant=t_pilot|field=email' | base64)"
log ""
log "Rotation manuelle :"
log "  vault write -f transit/keys/${PII_KEY}/rotate"
log ""
log "Next : bash INFRA/vault/scripts/configure-pki-terroir.sh"
