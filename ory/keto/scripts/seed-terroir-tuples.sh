#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# =============================================================================
# seed-terroir-tuples.sh — Seed Keto avec les tuples TERROIR P0.4 (multi-tenancy).
#
# Cible : Keto Write API (port 4467) → namespaces "Tenant", "Cooperative".
# Idempotent : un PUT répété renvoie 200/201 sans dupliquer.
#
# Référence : INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md §4 P0.4
#             INFRA/terroir/docs/adr/ADR-006-multi-tenancy.md
#
# Usage :
#   export SEED_SA_AMINATA="<uuid-aminata>"          # défaut fourni
#   export SEED_COOP_PILOT="<uuid-coop-pilote>"      # défaut fourni
#   bash seed-terroir-tuples.sh
# =============================================================================

set -euo pipefail

KETO_WRITE_URL="${KETO_WRITE_URL:-http://localhost:4467}"
KETO_READ_URL="${KETO_READ_URL:-${KETO_WRITE_URL//4467/4466}}"

# UUID seedé en Phase 4.c suite (super-admin pilote tenant TERROIR)
AMINATA_UUID="${SEED_SA_AMINATA:-253ec814-1e10-44c7-b7a7-fd44581e4393}"
# UUID v4 fixe de la coopérative pilote
COOP_PILOT_UUID="${SEED_COOP_PILOT:-c0000001-0000-0000-0000-000000000001}"
# Slug stable du tenant pilote (= schema PG terroir_t_pilot)
TENANT_PILOT_ID="${SEED_TENANT_PILOT:-t_pilot}"

log() { echo "[keto-terroir-seed] $*"; }
err() { echo "[keto-terroir-seed] ERROR: $*" >&2; }

# Probe Keto avant d'écrire (évite 30s de timeout sur curl).
if ! curl -fsS --max-time 3 "${KETO_WRITE_URL}/health/ready" >/dev/null 2>&1; then
  err "Keto Write API injoignable sur ${KETO_WRITE_URL}/health/ready"
  err "Vérifie : podman ps | grep keto"
  exit 1
fi

write_tuple() {
  local ns=$1 obj=$2 rel=$3 sub=$4
  local payload
  if [[ "$sub" == *":"* ]]; then
    # Subject set : "Namespace:object#relation" or "Namespace:object"
    local sub_ns="${sub%%:*}"
    local sub_rest="${sub#*:}"
    local sub_obj sub_rel
    if [[ "$sub_rest" == *"#"* ]]; then
      sub_obj="${sub_rest%%#*}"
      sub_rel="${sub_rest#*#}"
      payload=$(printf '{"namespace":"%s","object":"%s","relation":"%s","subject_set":{"namespace":"%s","object":"%s","relation":"%s"}}' \
        "$ns" "$obj" "$rel" "$sub_ns" "$sub_obj" "$sub_rel")
    else
      sub_obj="$sub_rest"
      payload=$(printf '{"namespace":"%s","object":"%s","relation":"%s","subject_set":{"namespace":"%s","object":"%s","relation":""}}' \
        "$ns" "$obj" "$rel" "$sub_ns" "$sub_obj")
    fi
  else
    payload=$(printf '{"namespace":"%s","object":"%s","relation":"%s","subject_id":"%s"}' \
      "$ns" "$obj" "$rel" "$sub")
  fi

  curl -fsS -X PUT "${KETO_WRITE_URL}/admin/relation-tuples" \
    -H "Content-Type: application/json" \
    -d "$payload" >/dev/null
  echo "  ok ${ns}:${obj}#${rel}@${sub}"
}

log "Seed TERROIR tuples sur ${KETO_WRITE_URL} ..."
log ""

# -----------------------------------------------------------------------------
# Tenant pilote — Aminata super-admin + gestionnaire du tenant
# -----------------------------------------------------------------------------
log "Tenant '${TENANT_PILOT_ID}' :"
write_tuple "Tenant" "${TENANT_PILOT_ID}" "admin"        "${AMINATA_UUID}"
write_tuple "Tenant" "${TENANT_PILOT_ID}" "gestionnaire" "${AMINATA_UUID}"

# -----------------------------------------------------------------------------
# Cooperative pilote — rattachée au tenant via subject_set parent
# Aminata = secretary de la coop pilote (cumul rôle pour bootstrap)
# -----------------------------------------------------------------------------
log ""
log "Cooperative '${COOP_PILOT_UUID}' :"
write_tuple "Cooperative" "${COOP_PILOT_UUID}" "parent"    "Tenant:${TENANT_PILOT_ID}"
write_tuple "Cooperative" "${COOP_PILOT_UUID}" "secretary" "${AMINATA_UUID}"

log ""
log "Seed TERROIR tuples OK (4 tuples écrits)."
log ""
log "Vérification :"
log "  curl '${KETO_READ_URL}/relation-tuples?namespace=Tenant'"
log "  curl '${KETO_READ_URL}/relation-tuples?namespace=Cooperative'"
