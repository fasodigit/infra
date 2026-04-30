#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION - Ministere du Numerique, Burkina Faso
# =============================================================================
# configure-pki-terroir.sh — TERROIR P0.B (Vault PKI intermediate + EORI role)
#
# Provisionne :
#   - Mount pki-terroir/ (intermediate CA, max-lease-ttl 10 ans / 87600h)
#   - CSR intermediate genere (EC P-384, common_name "terroir-ca.faso.bf")
#   - Si pki/ root existe (init.sh) : sign-intermediate + set-signed
#     Sinon : fallback pki-terroir/root/generate/internal (standalone, WARN)
#   - URLs pour issuing_certificates / crl_distribution_points
#   - Role `eori-exporter` :
#       allowed_domains=exporters.terroir.faso.bf, EC P-384, max_ttl 1 an,
#       client_flag=true, code_signing_flag=true, server_flag=false
#   - Test : emission cert eori-test.exporters.terroir.faso.bf (24h)
#
# IDEMPOTENT — re-run safe ; ne re-genere pas l'intermediate si CA deja signed.
# Cf. ULTRAPLAN §4 P0.2 + §12 (EORI signature DDS).
# =============================================================================

set -euo pipefail

VAULT_ADDR="${VAULT_ADDR:-http://127.0.0.1:8200}"
export VAULT_ADDR

log()  { echo "[terroir-pki] $*"; }
warn() { echo "[terroir-pki] WARN: $*" >&2; }
err()  { echo "[terroir-pki] ERROR: $*" >&2; }

# ---- Sanity checks --------------------------------------------------------
command -v vault >/dev/null 2>&1 || { err "binaire 'vault' introuvable"; exit 1; }
command -v jq    >/dev/null 2>&1 || { err "binaire 'jq' introuvable"; exit 1; }
command -v curl  >/dev/null 2>&1 || { err "binaire 'curl' introuvable"; exit 1; }

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
    exit 1
  fi
fi

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

INT_MOUNT="pki-terroir"
ROOT_MOUNT="pki"  # init.sh monte le root sous "pki/"
CSR_FILE="/tmp/terroir-intermediate.csr"
SIGNED_FILE="/tmp/terroir-intermediate-cert.pem"
ROOT_CA_FILE="/tmp/faso-root-ca.crt"

# ---- 1. Activation secret engine pki-terroir ------------------------------
if engine_enabled "$INT_MOUNT"; then
  log "Mount ${INT_MOUNT}/ deja active (skip)"
else
  log "Activation mount ${INT_MOUNT}/ (PKI) ..."
  vault_api POST "sys/mounts/${INT_MOUNT}" -d '{
    "type": "pki",
    "description": "TERROIR intermediate CA (EORI exporter certs, EUDR DDS signing)",
    "config": {
      "max_lease_ttl": "87600h"
    }
  }' >/dev/null
fi

# Tune max-lease-ttl=10y (idempotent si deja set)
log "Tune ${INT_MOUNT}/ max_lease_ttl=87600h (10 ans) ..."
vault_api POST "sys/mounts/${INT_MOUNT}/tune" -d '{"max_lease_ttl":"87600h"}' >/dev/null

# ---- 2. Generation intermediate (skip si deja un certificat signed) -------
# Detection : si /v1/pki-terroir/cert/ca retourne un cert non vide, on skip.
# Note : 404 attendu si mount fraichement cree (pre-pipefail-tolerance).
SKIP_GEN=false
EXISTING_CA="$(vault_api GET "${INT_MOUNT}/cert/ca" 2>/dev/null | jq -r '.data.certificate // empty' 2>/dev/null || true)"
if [[ -n "$EXISTING_CA" && "$EXISTING_CA" != "null" ]]; then
  log "Intermediate ${INT_MOUNT}/ deja signe (skip generation/sign)"
  SKIP_GEN=true
fi

if [[ "$SKIP_GEN" == "false" ]]; then
  log "Generation CSR intermediate (EC P-384) ..."
  CSR_RESP="$(vault_api POST "${INT_MOUNT}/intermediate/generate/internal" -d '{
    "common_name": "terroir-ca.faso.bf Intermediate CA",
    "issuer_name": "terroir-intermediate-2026",
    "organization": "FASO DIGITALISATION",
    "country": "BF",
    "key_type": "ec",
    "key_bits": 384
  }')"
  CSR="$(echo "$CSR_RESP" | jq -r '.data.csr')"
  if [[ -z "$CSR" || "$CSR" == "null" ]]; then
    err "echec generation CSR : $CSR_RESP"
    exit 1
  fi
  printf '%s\n' "$CSR" > "$CSR_FILE"
  chmod 600 "$CSR_FILE"
  log "  CSR ecrit dans $CSR_FILE"

  # ---- 3. Signature par root CA (pki/) si dispo, sinon standalone ---------
  if engine_enabled "$ROOT_MOUNT"; then
    ROOT_CA_TEST="$(vault_api GET "${ROOT_MOUNT}/cert/ca" 2>/dev/null | jq -r '.data.certificate // empty' 2>/dev/null || true)"
    if [[ -n "$ROOT_CA_TEST" && "$ROOT_CA_TEST" != "null" ]]; then
      log "Root CA detectee dans ${ROOT_MOUNT}/ -> sign-intermediate ..."
      SIGN_RESP="$(vault_api POST "${ROOT_MOUNT}/root/sign-intermediate" -d "$(jq -n \
        --arg csr "$CSR" \
        '{csr:$csr, format:"pem_bundle", ttl:"43800h", common_name:"terroir-ca.faso.bf Intermediate CA"}')")"
      SIGNED="$(echo "$SIGN_RESP" | jq -r '.data.certificate')"
      if [[ -z "$SIGNED" || "$SIGNED" == "null" ]]; then
        err "echec sign-intermediate : $SIGN_RESP"
        exit 1
      fi
      printf '%s\n' "$SIGNED" > "$SIGNED_FILE"
      chmod 600 "$SIGNED_FILE"
      log "  cert intermediate signe ecrit dans $SIGNED_FILE"

      log "Set-signed intermediate dans ${INT_MOUNT}/ ..."
      vault_api POST "${INT_MOUNT}/intermediate/set-signed" -d "$(jq -n \
        --arg cert "$SIGNED" '{certificate:$cert}')" >/dev/null
      log "  intermediate operational (chained to root)"
    else
      warn "Mount ${ROOT_MOUNT}/ active mais sans cert root -> fallback standalone"
      SIGN_INTERMEDIATE_STANDALONE=true
    fi
  else
    warn "Mount ${ROOT_MOUNT}/ absent -> fallback standalone intermediate"
    SIGN_INTERMEDIATE_STANDALONE=true
  fi

  if [[ "${SIGN_INTERMEDIATE_STANDALONE:-false}" == "true" ]]; then
    warn "Standalone intermediate, integrate to root CA later"
    log "Generation root self-signed dans ${INT_MOUNT}/ (fallback) ..."
    vault_api POST "${INT_MOUNT}/root/generate/internal" -d '{
      "common_name": "terroir-ca.faso.bf Intermediate CA (standalone)",
      "ttl": "43800h",
      "key_type": "ec",
      "key_bits": 384,
      "country": "BF",
      "organization": "FASO DIGITALISATION"
    }' >/dev/null
  fi
fi

# ---- 4. Configuration URLs (issuing + CRL) --------------------------------
log "Configuration URLs CRL/CA distribution sur ${INT_MOUNT}/ ..."
vault_api POST "${INT_MOUNT}/config/urls" -d "{
  \"issuing_certificates\": \"${VAULT_ADDR}/v1/${INT_MOUNT}/ca\",
  \"crl_distribution_points\": \"${VAULT_ADDR}/v1/${INT_MOUNT}/crl\"
}" >/dev/null

# ---- 5. Role EORI exportateur ---------------------------------------------
log "Creation/maj role ${INT_MOUNT}/roles/eori-exporter ..."
vault_api POST "${INT_MOUNT}/roles/eori-exporter" -d '{
  "allowed_domains": "exporters.terroir.faso.bf",
  "allow_subdomains": true,
  "allow_glob_domains": false,
  "allow_bare_domains": false,
  "max_ttl": "8760h",
  "ttl": "8760h",
  "key_type": "ec",
  "key_bits": 384,
  "server_flag": false,
  "client_flag": true,
  "code_signing_flag": true
}' >/dev/null

# ---- 6. Test : emission cert eori-test.exporters.terroir.faso.bf ----------
log "Test : emission cert eori-test.exporters.terroir.faso.bf (TTL 24h) ..."
ISSUE_RESP="$(vault_api POST "${INT_MOUNT}/issue/eori-exporter" -d '{
  "common_name": "eori-test.exporters.terroir.faso.bf",
  "ttl": "24h"
}')"

CERT="$(echo "$ISSUE_RESP" | jq -r '.data.certificate // empty')"
PRIV_KEY="$(echo "$ISSUE_RESP" | jq -r '.data.private_key // empty')"
SERIAL="$(echo "$ISSUE_RESP" | jq -r '.data.serial_number // empty')"

if [[ -z "$CERT" || "$CERT" == "null" || -z "$SERIAL" ]]; then
  err "echec emission cert EORI : $ISSUE_RESP"
  exit 1
fi
log "  cert emis OK"
log "  serial    : $SERIAL"
log "  cert      : ${CERT:0:64}..."
[[ -n "$PRIV_KEY" && "$PRIV_KEY" != "null" ]] && log "  priv_key  : (present, ${#PRIV_KEY} chars)"

# ---- Recap ----------------------------------------------------------------
log ""
log "TERROIR Vault PKI configure :"
log "  - mount  ${INT_MOUNT}/ (intermediate CA, EC P-384, max-lease 10y)"
log "  - role   ${INT_MOUNT}/roles/eori-exporter (EC P-384, code_signing+client)"
log ""
log "Inspect :"
log "  vault list ${INT_MOUNT}/roles"
log "  vault read ${INT_MOUNT}/cert/ca"
log "  vault read ${INT_MOUNT}/cert/${SERIAL}"
log ""
log "Emission cert EORI exportateur :"
log "  vault write ${INT_MOUNT}/issue/eori-exporter \\"
log "    common_name=\"acme.exporters.terroir.faso.bf\" ttl=8760h"
log ""
log "Revocation :"
log "  vault write ${INT_MOUNT}/revoke serial_number=<serial>"
