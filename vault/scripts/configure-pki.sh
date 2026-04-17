#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Configure Vault PKI root + intermediate for FASO trust domain.
# SPIRE server can consume this intermediate as an UpstreamAuthority.

set -euo pipefail

VAULT_ADDR="${VAULT_ADDR:-http://127.0.0.1:8200}"
export VAULT_ADDR
[[ -n "${VAULT_TOKEN:-}" ]] || { echo "ERROR: export VAULT_TOKEN first"; exit 1; }

log() { echo "[faso-vault-pki] $*"; }

# ---- Root CA (self-signed, 10 years) --------------------------------------
log "Generating root CA for trust domain faso.gov.bf ..."
curl -fsS -X POST -H "X-Vault-Token: $VAULT_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "common_name": "FASO DIGITALISATION Root CA",
    "ttl": "87600h",
    "key_type": "ec",
    "key_bits": 256,
    "country": "BF",
    "organization": "FASO DIGITALISATION"
  }' \
  "${VAULT_ADDR}/v1/pki/root/generate/internal" | jq -r '.data.certificate' > /tmp/faso-root-ca.crt

log "✓ Root CA written to /tmp/faso-root-ca.crt"

# ---- URLs for CRL / CA distribution --------------------------------------
curl -fsS -X POST -H "X-Vault-Token: $VAULT_TOKEN" \
  -H 'Content-Type: application/json' \
  -d "{
    \"issuing_certificates\": \"${VAULT_ADDR}/v1/pki/ca\",
    \"crl_distribution_points\": \"${VAULT_ADDR}/v1/pki/crl\"
  }" "${VAULT_ADDR}/v1/pki/config/urls" >/dev/null

# ---- Roles per service (ARMAGEDDON / KAYA / auth-ms / ...) ----------------
create_pki_role() {
  local role="$1" domains="$2"
  log "Creating PKI role: $role (allowed_domains=$domains)"
  curl -fsS -X POST -H "X-Vault-Token: $VAULT_TOKEN" \
    -H 'Content-Type: application/json' \
    -d "{
      \"allowed_domains\": \"${domains}\",
      \"allow_subdomains\": true,
      \"allow_ip_sans\": true,
      \"allowed_uri_sans\": \"spiffe://faso.gov.bf/*\",
      \"max_ttl\": \"720h\",
      \"ttl\": \"72h\",
      \"key_type\": \"ec\",
      \"key_bits\": 256
    }" "${VAULT_ADDR}/v1/pki/roles/${role}" >/dev/null
}

create_pki_role "armageddon" "armageddon.faso.gov.bf,gateway.faso.gov.bf"
create_pki_role "kaya"       "kaya.faso.gov.bf,cache.faso.gov.bf"
create_pki_role "auth-ms"    "auth.faso.gov.bf"
create_pki_role "poulets"    "poulets.faso.gov.bf"
create_pki_role "spire"      "faso.gov.bf,spiffe.faso.gov.bf"

log "✓ PKI bootstrapped. Issue a cert:"
log "  vault write pki/issue/armageddon common_name=gateway.faso.gov.bf ttl=24h"
