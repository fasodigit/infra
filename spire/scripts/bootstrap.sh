#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# FASO DIGITALISATION — SPIRE bootstrap script (dev environment)
#
# What this script does:
#   1. Generates a self-signed CA (dev only) if certs/ is absent.
#   2. Starts faso-spire-server via podman-compose.
#   3. Waits for server to be healthy.
#   4. Generates a join token and writes it to SPIRE_JOIN_TOKEN env var.
#   5. Starts faso-spire-agent.
#   6. Registers 8 workload SPIFFE entries (TTL 86400s, parent = node).
#
# Workload UIDs: defaults to current user UID; override via env:
#   ARMAGEDDON_UID, KAYA_UID, AUTH_UID, POULETS_UID,
#   NOTIFIER_UID, ETAT_CIVIL_UID, SOGESY_UID, BFF_UID
#
# Usage (idempotent — safe to re-run):
#   cd INFRA/spire
#   bash scripts/bootstrap.sh
#
# Requires: podman-compose, openssl, jq, curl

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SPIRE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
COMPOSE_FILE="${SPIRE_DIR}/podman-compose.spire.yml"
TRUST_DOMAIN="faso.gov.bf"
SERVER_CONTAINER="faso-spire-server"
AGENT_CONTAINER="faso-spire-agent"
SPIRE_SERVER_BIN="/opt/spire/bin/spire-server"
CERTS_DIR="${SPIRE_DIR}/certs"

# ── UIDs for unix workload attestation ────────────────────────────────────
CURRENT_UID="$(id -u)"
ARMAGEDDON_UID="${ARMAGEDDON_UID:-${CURRENT_UID}}"
KAYA_UID="${KAYA_UID:-${CURRENT_UID}}"
AUTH_UID="${AUTH_UID:-${CURRENT_UID}}"
POULETS_UID="${POULETS_UID:-${CURRENT_UID}}"
NOTIFIER_UID="${NOTIFIER_UID:-${CURRENT_UID}}"
ETAT_CIVIL_UID="${ETAT_CIVIL_UID:-${CURRENT_UID}}"
SOGESY_UID="${SOGESY_UID:-${CURRENT_UID}}"
BFF_UID="${BFF_UID:-${CURRENT_UID}}"

log()  { echo "[bootstrap] $*"; }
die()  { echo "[bootstrap] ERROR: $*" >&2; exit 1; }
wait_healthy() {
  local svc="$1" max_attempts=30 attempt=0
  log "Waiting for ${svc} to be healthy..."
  until podman inspect --format '{{.State.Health.Status}}' "${svc}" 2>/dev/null | grep -q "healthy"; do
    attempt=$(( attempt + 1 ))
    [[ ${attempt} -ge ${max_attempts} ]] && die "${svc} did not become healthy after ${max_attempts} attempts"
    sleep 3
  done
  log "${svc} is healthy."
}

# ── Step 1: Generate self-signed dev CA if missing ────────────────────────
if [[ ! -f "${CERTS_DIR}/ca.crt" || ! -f "${CERTS_DIR}/ca.key" ]]; then
  log "Generating self-signed CA for trust domain ${TRUST_DOMAIN} (dev only)..."
  mkdir -p "${CERTS_DIR}"
  openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 \
    -keyout "${CERTS_DIR}/ca.key" \
    -out    "${CERTS_DIR}/ca.crt" \
    -days   90 \
    -nodes \
    -subj "/C=BF/O=FASO DIGITALISATION/CN=FASO SPIFFE CA" \
    -addext "subjectAltName=URI:spiffe://${TRUST_DOMAIN}" \
    2>/dev/null
  chmod 600 "${CERTS_DIR}/ca.key"
  log "CA written to ${CERTS_DIR}/"
else
  log "CA already present at ${CERTS_DIR}/ — skipping generation."
fi

# ── Step 2: Create faso-net if absent ─────────────────────────────────────
if ! podman network exists faso-net 2>/dev/null; then
  log "Creating podman network faso-net..."
  podman network create faso-net
fi

# ── Step 3: Start SPIRE Server ────────────────────────────────────────────
log "Starting faso-spire-server..."
podman-compose -f "${COMPOSE_FILE}" up -d faso-spire-server
wait_healthy "${SERVER_CONTAINER}"

# ── Step 4: Generate join token ───────────────────────────────────────────
log "Generating join token..."
JOIN_TOKEN="$(podman exec "${SERVER_CONTAINER}" \
  "${SPIRE_SERVER_BIN}" token generate \
  -spiffeID "spiffe://${TRUST_DOMAIN}/ns/spire/sa/spire-agent" \
  -ttl 3600 \
  | grep "Token:" | awk '{print $2}')"
[[ -z "${JOIN_TOKEN}" ]] && die "Failed to generate join token"
log "Join token generated: ${JOIN_TOKEN:0:8}…"
export SPIRE_JOIN_TOKEN="${JOIN_TOKEN}"

# Patch agent.conf to inject join token (dev convenience; prod uses k8s_psat)
# We use an env var passed to podman-compose instead of patching the file.

# ── Step 5: Start SPIRE Agent ─────────────────────────────────────────────
log "Starting faso-spire-agent..."
podman-compose -f "${COMPOSE_FILE}" up -d faso-spire-agent
wait_healthy "${AGENT_CONTAINER}"

# ── Step 6: Resolve node SPIFFE ID ────────────────────────────────────────
# The agent registers itself; we use its SPIFFE ID as parentID for workloads.
NODE_SPIFFE_ID="spiffe://${TRUST_DOMAIN}/ns/spire/sa/spire-agent"
log "Using parentID: ${NODE_SPIFFE_ID}"

# Helper: create entry if it does not already exist (idempotent)
create_entry() {
  local spiffe_id="$1" selector="$2" ttl="${3:-86400}"
  local parent_id="${NODE_SPIFFE_ID}"

  # Check if entry already exists
  if podman exec "${SERVER_CONTAINER}" \
       "${SPIRE_SERVER_BIN}" entry show \
       -spiffeID "${spiffe_id}" 2>/dev/null | grep -q "Entry ID"; then
    log "Entry already exists: ${spiffe_id} — skipping."
    return 0
  fi

  podman exec "${SERVER_CONTAINER}" \
    "${SPIRE_SERVER_BIN}" entry create \
      -spiffeID  "${spiffe_id}" \
      -parentID  "${parent_id}" \
      -selector  "${selector}" \
      -ttl       "${ttl}"
  log "Registered: ${spiffe_id} (selector=${selector}, ttl=${ttl}s)"
}

# ── Step 7: Register 8 workload entries ───────────────────────────────────
# TTL 86400s (24h) per spec. Selectors use unix:uid for dev environment.
log "Registering workload SPIFFE entries..."

create_entry \
  "spiffe://${TRUST_DOMAIN}/ns/default/sa/armageddon" \
  "unix:uid:${ARMAGEDDON_UID}" \
  86400

create_entry \
  "spiffe://${TRUST_DOMAIN}/ns/default/sa/kaya" \
  "unix:uid:${KAYA_UID}" \
  86400

create_entry \
  "spiffe://${TRUST_DOMAIN}/ns/default/sa/auth-ms" \
  "unix:uid:${AUTH_UID}" \
  86400

create_entry \
  "spiffe://${TRUST_DOMAIN}/ns/default/sa/poulets-api" \
  "unix:uid:${POULETS_UID}" \
  86400

create_entry \
  "spiffe://${TRUST_DOMAIN}/ns/default/sa/notifier-ms" \
  "unix:uid:${NOTIFIER_UID}" \
  86400

create_entry \
  "spiffe://${TRUST_DOMAIN}/ns/default/sa/etat-civil-ms" \
  "unix:uid:${ETAT_CIVIL_UID}" \
  86400

create_entry \
  "spiffe://${TRUST_DOMAIN}/ns/default/sa/sogesy-ms" \
  "unix:uid:${SOGESY_UID}" \
  86400

create_entry \
  "spiffe://${TRUST_DOMAIN}/ns/default/sa/frontend-bff" \
  "unix:uid:${BFF_UID}" \
  86400

# ── Done ──────────────────────────────────────────────────────────────────
log ""
log "Bootstrap complete."
log "  Trust domain : spiffe://${TRUST_DOMAIN}"
log "  Server API   : 127.0.0.1:8081"
log "  Agent socket : /run/spire/sockets/agent.sock  (via spire-sockets volume)"
log "  Metrics      : :9988 (server), :9989 (agent)"
log ""
log "Verify entries:"
log "  podman exec ${SERVER_CONTAINER} ${SPIRE_SERVER_BIN} entry show"
log ""
log "Check SVID expiry:"
log "  bash scripts/check-expiration.sh"
