#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# =============================================================================
# seed-redpanda-admin-topics.sh — Phase 4.b admin-UI / Stream D2
#
# Crée les topics Redpanda nécessaires aux flux admin (OTP, rôles, devices,
# break-glass, settings) avec partitions/replicas/retention adaptés.
#
# Idempotent : `rpk topic create` retourne 0 si topic déjà présent
# (avec un warning), donc on ignore ce cas.
#
# Stratégie d'invocation rpk :
#   1. Si binaire `rpk` dans le PATH → l'utiliser directement
#      (avec --brokers $REDPANDA_BROKERS, défaut: 127.0.0.1:9092)
#   2. Sinon, fallback : `podman exec redpanda rpk topic create ...`
# =============================================================================

set -euo pipefail

REDPANDA_BROKERS="${REDPANDA_BROKERS:-127.0.0.1:9092}"
REDPANDA_CONTAINER="${REDPANDA_CONTAINER:-redpanda}"

log() { echo "[redpanda-admin-seed] $*"; }
err() { echo "[redpanda-admin-seed] ERROR: $*" >&2; }

# ---- Sélection runtime rpk ------------------------------------------------
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

create_topic() {
  local name="$1"
  local partitions="$2"
  local replicas="$3"
  local retention_ms="$4"
  log "  topic ${name} (p=${partitions}, r=${replicas}, retention=${retention_ms}ms)"
  if "${RPK_CMD[@]}" topic create "$name" \
        --partitions "$partitions" \
        --replicas "$replicas" \
        --config "retention.ms=${retention_ms}" >/tmp/.rpk-out 2>&1; then
    return 0
  fi
  # Tolère "TOPIC_ALREADY_EXISTS"
  if grep -qiE 'already.*exists|TOPIC_ALREADY_EXISTS' /tmp/.rpk-out; then
    log "    (déjà présent — skip)"
    return 0
  fi
  err "Échec création topic $name :"
  cat /tmp/.rpk-out >&2
  return 1
}

# ---- Topics fonctionnels --------------------------------------------------
log "Création des topics admin-UI ..."

# OTP issuance — 7 jours
create_topic "auth.otp.issue"             3 1 604800000
# OTP verifications — 30 jours (audit court)
create_topic "auth.otp.verified"          3 1 2592000000
# Role lifecycle — 90 jours (audit légal)
create_topic "auth.role.granted"          1 1 7776000000
create_topic "auth.role.revoked"          1 1 7776000000
# Trust device — 30 jours
create_topic "auth.device.trusted"        3 1 2592000000
# Sessions — 7 jours
create_topic "auth.session.revoked"       3 1 604800000
# Break-glass — 1 an (audit critique)
create_topic "admin.break_glass.activated" 1 1 31536000000
# Settings change — ~7 ans (compliance)
create_topic "admin.settings.changed"      1 1 220924800000
# Suspensions — 7 jours (op rapide)
create_topic "admin.user.suspended"        3 1 604800000

# ---- Dead-Letter Queues (DLQ) — 30 jours ----------------------------------
log "Création des DLQ ..."
for t in auth.otp.issue auth.role.granted admin.break_glass.activated admin.settings.changed; do
  create_topic "${t}.dlq" 1 1 2592000000
done

log "OK — topics admin créés / vérifiés."
log ""
log "Inspect :"
log "  ${RPK_CMD[*]} topic list"
