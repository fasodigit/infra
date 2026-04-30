#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# =============================================================================
# seed-admin-tuples.sh — Seed Keto avec les tuples AdminRole initiaux.
# Stream D2 / Phase 4.b admin-UI FASO.
#
# Cible : Keto Write API (port 4467) → namespace "AdminRole".
# Idempotent : un PUT répété renvoie 201/200 sans dupliquer.
#
# UUIDs des super-admins fournis via variables d'environnement :
#   SEED_SA_AMINATA      UUID Kratos identité Aminata
#   SEED_SA_SOULEYMANE   UUID Kratos identité Souleymane
#
# Usage :
#   export SEED_SA_AMINATA="aminata-uuid"
#   export SEED_SA_SOULEYMANE="souleymane-uuid"
#   bash seed-admin-tuples.sh
# =============================================================================

set -euo pipefail

KETO_WRITE_URL="${KETO_WRITE_URL:-http://127.0.0.1:4467}"
NAMESPACE="${KETO_ADMIN_NAMESPACE:-AdminRole}"
OBJECT="${KETO_ADMIN_OBJECT:-platform}"

SEED_SA_AMINATA="${SEED_SA_AMINATA:-}"
SEED_SA_SOULEYMANE="${SEED_SA_SOULEYMANE:-}"

log() { echo "[keto-admin-seed] $*"; }
err() { echo "[keto-admin-seed] ERROR: $*" >&2; }

require_env() {
  local name="$1"
  local value
  value="$(eval "echo \${${name}:-}")"
  if [[ -z "$value" ]]; then
    err "Variable d'environnement obligatoire manquante: $name"
    err "Exporte un UUID Kratos pour ce super-admin avant de relancer."
    exit 1
  fi
}

require_env "SEED_SA_AMINATA"
require_env "SEED_SA_SOULEYMANE"

# Probe Keto avant d'écrire (évite 30s de timeout sur curl).
if ! curl -fsS --max-time 3 "${KETO_WRITE_URL}/health/ready" >/dev/null 2>&1; then
  err "Keto Write API injoignable sur ${KETO_WRITE_URL}/health/ready"
  err "Vérifie : podman ps | grep keto"
  exit 1
fi

put_tuple() {
  local relation="$1"
  local subject_id="$2"
  log "  PUT ${NAMESPACE}:${OBJECT}#${relation}@${subject_id}"
  curl -fsS -X PUT \
    -H 'Content-Type: application/json' \
    -d "{
      \"namespace\": \"${NAMESPACE}\",
      \"object\":    \"${OBJECT}\",
      \"relation\":  \"${relation}\",
      \"subject_id\": \"${subject_id}\"
    }" \
    "${KETO_WRITE_URL}/admin/relation-tuples" >/dev/null
}

log "Seed des super-admins AdminRole sur ${KETO_WRITE_URL} ..."
put_tuple "super_admin" "${SEED_SA_AMINATA}"
put_tuple "super_admin" "${SEED_SA_SOULEYMANE}"

log "OK — 2 tuples super_admin écrits."

# ----------------------------------------------------------------------------
# Delta amendment 2026-04-30 §1 — seed all 30 capabilities for SUPER-ADMINs.
# Tuple shape : Capability:<key>#granted@<userId>
# ----------------------------------------------------------------------------
CAPABILITY_NS="Capability"
CAPABILITY_RELATION="granted"

CAPABILITIES=(
  "users:invite"
  "users:suspend"
  "users:reactivate"
  "users:manage:any_dept"
  "users:manage:own_dept"
  "users:mfa:reset"
  "roles:grant_admin"
  "roles:grant_manager"
  "roles:revoke"
  "sessions:list"
  "sessions:revoke"
  "sessions:revoke_all"
  "devices:list"
  "devices:revoke"
  "audit:view"
  "audit:export"
  "settings:read"
  "settings:write_otp"
  "settings:write_device_trust"
  "settings:write_session"
  "settings:write_mfa"
  "settings:write_grant"
  "settings:write_break_glass"
  "settings:write_audit"
  "break_glass:activate"
  "recovery:initiate_for_user"
  "recovery:complete"
  "self:password_change"
  "self:passkey_manage"
  "self:totp_manage"
  "self:recovery_codes_regenerate"
)

put_capability_tuple() {
  local capability_key="$1"
  local subject_id="$2"
  log "  PUT ${CAPABILITY_NS}:${capability_key}#${CAPABILITY_RELATION}@${subject_id}"
  curl -fsS -X PUT \
    -H 'Content-Type: application/json' \
    -d "{
      \"namespace\": \"${CAPABILITY_NS}\",
      \"object\":    \"${capability_key}\",
      \"relation\":  \"${CAPABILITY_RELATION}\",
      \"subject_id\": \"${subject_id}\"
    }" \
    "${KETO_WRITE_URL}/admin/relation-tuples" >/dev/null
}

log ""
log "Seed des ${#CAPABILITIES[@]} capacités fines pour les SUPER-ADMINs ..."
for sa in "${SEED_SA_AMINATA}" "${SEED_SA_SOULEYMANE}"; do
  for cap in "${CAPABILITIES[@]}"; do
    put_capability_tuple "${cap}" "${sa}"
  done
done

log "OK — $(( ${#CAPABILITIES[@]} * 2 )) tuples Capability écrits."
log ""
log "Vérification :"
log "  curl '${KETO_WRITE_URL//4467/4466}/relation-tuples?namespace=${NAMESPACE}&object=${OBJECT}'"
log "  curl '${KETO_WRITE_URL//4467/4466}/relation-tuples?namespace=${CAPABILITY_NS}'"
