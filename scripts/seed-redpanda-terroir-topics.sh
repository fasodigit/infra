#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# =============================================================================
# seed-redpanda-terroir-topics.sh — Phase TERROIR P0.E
#
# Crée les topics Redpanda nécessaires aux flux TERROIR (membres, parcelles,
# récoltes, paiements, DDS/EUDR, USSD, audit, sync-conflicts, tenant lifecycle)
# avec partitions/replicas/retention adaptés.
#
# Idempotent : `rpk topic create` retourne 0 si topic déjà présent ;
# on détecte TOPIC_ALREADY_EXISTS sur stderr et on continue.
#
# Stratégie d'invocation rpk :
#   1. Si binaire `rpk` dans le PATH → l'utiliser directement
#      (avec --brokers $REDPANDA_BROKERS, défaut: 127.0.0.1:9092)
#   2. Sinon, fallback podman exec faso-redpanda rpk
#   3. Sinon, fallback docker exec faso-redpanda rpk
#
# Topics et rétentions :
#   - audit-pertinent (90 j)  : member.*, parcel.created/updated, harvest.*
#   - evidence EUDR (1 an)    : parcel.eudr.*, payment.*
#   - compliance légale (7 ans): dds.*, audit.event
#   - ops courtes (7-30 j)    : ussd.*, sync.conflict.*
#   - DLQ topics critiques    : 90 j
# =============================================================================

set -euo pipefail

REDPANDA_BROKERS="${REDPANDA_BROKERS:-127.0.0.1:9092}"
REDPANDA_CONTAINER="${REDPANDA_CONTAINER:-faso-redpanda}"

log()  { echo "[terroir-seed] $*"; }
err()  { echo "[terroir-seed] ERROR: $*" >&2; }
info() { echo "[terroir-seed]   $*"; }

# ---- Sélection runtime rpk --------------------------------------------------
if command -v rpk >/dev/null 2>&1; then
  RPK_CMD=(rpk --brokers "$REDPANDA_BROKERS")
  log "Utilise rpk local (brokers=$REDPANDA_BROKERS)"
elif command -v podman >/dev/null 2>&1 \
     && podman ps --format '{{.Names}}' | grep -qx "$REDPANDA_CONTAINER"; then
  RPK_CMD=(podman exec "$REDPANDA_CONTAINER" rpk)
  log "Fallback : podman exec ${REDPANDA_CONTAINER} rpk"
elif command -v docker >/dev/null 2>&1 \
     && docker ps --format '{{.Names}}' | grep -qx "$REDPANDA_CONTAINER"; then
  RPK_CMD=(docker exec "$REDPANDA_CONTAINER" rpk)
  log "Fallback : docker exec ${REDPANDA_CONTAINER} rpk (compat)"
else
  err "Ni 'rpk' dans le PATH ni conteneur '${REDPANDA_CONTAINER}' actif."
  err "Démarre Redpanda :"
  err "  podman-compose -f INFRA/docker/compose/podman-compose.yml up -d redpanda"
  exit 1
fi

# ---- Fonction create_topic --------------------------------------------------
# Usage : create_topic <name> <partitions> <replicas> <retention_ms>
create_topic() {
  local name="$1"
  local partitions="$2"
  local replicas="$3"
  local retention_ms="$4"
  info "topic ${name} (p=${partitions}, r=${replicas}, retention=${retention_ms}ms)"
  if "${RPK_CMD[@]}" topic create "$name" \
        --partitions "$partitions" \
        --replicas   "$replicas"   \
        --config "retention.ms=${retention_ms}" \
        --config "compression.type=zstd" >/tmp/.rpk-terroir-out 2>&1; then
    return 0
  fi
  # Tolère "TOPIC_ALREADY_EXISTS"
  if grep -qiE 'already.*exists|TOPIC_ALREADY_EXISTS' /tmp/.rpk-terroir-out; then
    info "  (déjà présent — skip)"
    return 0
  fi
  err "Échec création topic ${name} :"
  cat /tmp/.rpk-terroir-out >&2
  return 1
}

# ---- Constantes de rétention (ms) ------------------------------------------
readonly D7=$((   7 * 86400000))   #   7 jours  — ops courtes USSD / OTP
readonly D30=$((  30 * 86400000))  #  30 jours  — sync conflicts / OTP vérifié
readonly D90=$((  90 * 86400000))  #  90 jours  — audit opérationnel
readonly D365=$((365 * 86400000))  #   1 an     — evidence EUDR / paiements
readonly D2555=$((2555 * 86400000)) # ~7 ans    — Loi 010-2004 BF / DDS / audit

# =============================================================================
# MEMBER EVENTS (audit opérationnel 90 j)
# =============================================================================
log "=== Member events (90 j) ==="
create_topic "terroir.member.created"       3 1 "$D90"
create_topic "terroir.member.updated"       3 1 "$D90"
create_topic "terroir.member.deleted"       1 1 "$D90"

# =============================================================================
# PARCEL EVENTS
# =============================================================================
log "=== Parcel events ==="
create_topic "terroir.parcel.created"           3 1 "$D90"
create_topic "terroir.parcel.updated"           3 1 "$D90"
create_topic "terroir.parcel.eudr.validated"    3 1 "$D365"   # evidence 1 an
create_topic "terroir.parcel.eudr.rejected"     1 1 "$D365"

# =============================================================================
# HARVEST EVENTS (audit opérationnel 90 j)
# =============================================================================
log "=== Harvest events (90 j) ==="
create_topic "terroir.harvest.lot.recorded"     3 1 "$D90"

# =============================================================================
# PAYMENT EVENTS (long retention pour reconciliation 1 an)
# =============================================================================
log "=== Payment events (1 an) ==="
create_topic "terroir.payment.initiated"        3 1 "$D365"
create_topic "terroir.payment.completed"        3 1 "$D365"
create_topic "terroir.payment.failed"           1 1 "$D365"

# =============================================================================
# DDS EVENTS (compliance EUDR — Loi 010-2004 BF ~7 ans)
# =============================================================================
log "=== DDS / EUDR compliance events (7 ans) ==="
create_topic "terroir.dds.generated"            1 1 "$D2555"
create_topic "terroir.dds.submitted"            1 1 "$D2555"
create_topic "terroir.dds.rejected"             1 1 "$D2555"

# =============================================================================
# TENANT LIFECYCLE (1 an)
# =============================================================================
log "=== Tenant lifecycle (1 an) ==="
create_topic "terroir.tenant.provisioned"       1 1 "$D365"

# =============================================================================
# CONFIGURATION CENTER (~30 j) — feature flags / settings runtime
# =============================================================================
log "=== Settings changed (30 j) ==="
create_topic "terroir.settings.changed"         1 1 "$D30"

# =============================================================================
# AUDIT CONSOLIDÉ (~7 ans — conformité BF)
# =============================================================================
log "=== Audit consolidé (7 ans) ==="
create_topic "terroir.audit.event"              3 1 "$D2555"

# =============================================================================
# SYNC CONFLICTS (opérationnel court 30 j)
# =============================================================================
log "=== Sync conflict events (30 j) ==="
create_topic "terroir.sync.conflict.detected"   1 1 "$D30"
create_topic "terroir.sync.conflict.resolved"   1 1 "$D30"

# =============================================================================
# USSD EVENTS
# =============================================================================
log "=== USSD events ==="
create_topic "terroir.ussd.session.started"     3 1 "$D7"
create_topic "terroir.ussd.session.ended"       3 1 "$D7"
create_topic "terroir.ussd.otp.sent"            3 1 "$D7"
create_topic "terroir.ussd.otp.verified"        3 1 "$D30"

# =============================================================================
# DLQ pour topics critiques (90 j)
# =============================================================================
log "=== Dead-Letter Queues (90 j) ==="
for t in \
  terroir.payment.initiated \
  terroir.dds.submitted \
  terroir.tenant.provisioned \
  terroir.parcel.eudr.validated
do
  create_topic "${t}.dlq" 1 1 "$D90"
done

# =============================================================================
log "TERROIR topics OK"
log ""
log "Inspect :"
log "  ${RPK_CMD[*]} topic list | grep terroir"
