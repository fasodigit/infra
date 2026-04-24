#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION - Ministere du Numerique, Burkina Faso
# ============================================================
# Setup Vault Transit secrets engine for FASO PII and financial
# data encryption.
#
# Creates two encryption keys:
#   - faso-pii        : PII fields (email, phone, address, NIN)
#   - faso-financial   : financial data (amounts, account numbers)
#
# Key type: aes256-gcm96 (AES-256 in GCM mode)
# Supports key rotation without re-encryption (Vault handles
# versioned keys transparently).
#
# Prerequisites:
#   - Vault unsealed, VAULT_TOKEN set
#
# Usage:
#   export VAULT_TOKEN=$(jq -r .root_token ~/.faso-vault-keys.json)
#   bash vault/scripts/setup-transit.sh
# ============================================================

set -euo pipefail

VAULT_ADDR="${VAULT_ADDR:-http://127.0.0.1:8200}"
export VAULT_ADDR

[[ -n "${VAULT_TOKEN:-}" ]] || {
  echo "ERROR: export VAULT_TOKEN first (from ~/.faso-vault-keys.json)"
  exit 1
}

log() { echo "[faso-vault-transit] $*"; }

vault_api() {
  local method="$1" path="$2"
  shift 2
  curl -fsS -X "$method" \
    -H "X-Vault-Token: $VAULT_TOKEN" \
    -H 'Content-Type: application/json' \
    "$@" "${VAULT_ADDR}/v1/${path}"
}

# ---- Enable Transit secrets engine (idempotent) -----------------------------
log "Enabling Transit secrets engine ..."
vault_api POST "sys/mounts/transit" \
  -d '{"type":"transit","description":"FASO encryption-as-a-service for PII and financial data"}' \
  2>/dev/null || log "  (already enabled)"

# ---- Create encryption keys -------------------------------------------------

create_key() {
  local name="$1" purpose="$2"
  log "Creating transit key: ${name} (${purpose})"

  vault_api POST "transit/keys/${name}" \
    -d '{
      "type": "aes256-gcm96",
      "exportable": false,
      "allow_plaintext_backup": false,
      "deletion_allowed": false
    }' 2>/dev/null || log "  (key ${name} already exists)"

  # Configure key to support rotation without re-encryption
  # min_decryption_version=1 means all versions can decrypt
  vault_api POST "transit/keys/${name}/config" \
    -d '{
      "min_decryption_version": 1,
      "min_encryption_version": 0,
      "deletion_allowed": false,
      "auto_rotate_period": "720h"
    }' >/dev/null
}

create_key "faso-pii" "PII fields (email, phone, address, NIN)"
create_key "faso-financial" "Financial data (amounts, account numbers, IBAN)"

log ""
log "Transit engine configured."
log ""
log "Encrypt example:"
log "  vault write transit/encrypt/faso-pii plaintext=\$(echo -n 'user@example.bf' | base64)"
log ""
log "Decrypt example:"
log "  vault write transit/decrypt/faso-pii ciphertext=vault:v1:..."
log ""
log "Rotate key (does NOT require re-encrypting existing data):"
log "  vault write -f transit/keys/faso-pii/rotate"
log ""
log "Auto-rotation: every 30 days (720h)"
