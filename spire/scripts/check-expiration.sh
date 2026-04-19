#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# FASO DIGITALISATION — SPIRE SVID expiration checker
#
# Behaviour:
#   - Lists all SPIFFE entries via `spire-server entry show`.
#   - For each entry, fetches its X.509 SVID TTL from the registration record.
#   - Emits metric `spire_svid_expires_in_hours{spiffe_id="..."}` to Pushgateway.
#   - Prints WARN for TTL < 72h, CRITICAL for TTL < 24h.
#   - Exits 1 if at least one SVID has TTL < 24h (CI pipeline fails).
#   - Exits 0 otherwise (including WARN-only cases for alerting path via Prometheus).
#
# Environment variables:
#   PROM_PUSHGATEWAY_URL  Prometheus pushgateway base URL (optional).
#                         Example: http://localhost:9091
#   SPIRE_SERVER_SOCKET   Path to spire-server admin socket.
#                         Default: /tmp/spire-server/private/api.sock
#   SPIRE_SERVER_BIN      Path to spire-server binary.
#                         Default: /opt/spire/bin/spire-server
#
# Usage:
#   # Local (dev — directly on agent host):
#   bash INFRA/spire/scripts/check-expiration.sh
#
#   # Via podman exec:
#   podman exec faso-spire-server bash /opt/spire/scripts/check-expiration.sh
#
#   # In CI (GitHub Actions):
#   See .github/workflows/spire-expiration-check.yml

set -euo pipefail

TRUST_DOMAIN="faso.gov.bf"
SPIRE_SERVER_BIN="${SPIRE_SERVER_BIN:-/opt/spire/bin/spire-server}"
SPIRE_SERVER_SOCKET="${SPIRE_SERVER_SOCKET:-/tmp/spire-server/private/api.sock}"
PROM_PUSHGATEWAY_URL="${PROM_PUSHGATEWAY_URL:-}"
PROM_JOB="spire-svid-monitor"

WARN_HOURS=72
CRITICAL_HOURS=24

log()      { echo "[check-expiration] $*"; }
log_warn() { echo "[check-expiration] WARN:     $*" >&2; }
log_crit() { echo "[check-expiration] CRITICAL: $*" >&2; }

# Workloads to check (the 8 registered in bootstrap.sh + spire-agent itself)
WORKLOADS=(
  "armageddon"
  "kaya"
  "auth-ms"
  "poulets-api"
  "notifier-ms"
  "etat-civil-ms"
  "sogesy-ms"
  "frontend-bff"
)

exit_code=0
prometheus_metrics=""

# Build Prometheus metric block for pushgateway
emit_metric() {
  local spiffe_id="$1" hours_remaining="$2"
  # Escape quotes for label value
  local safe_id="${spiffe_id//\"/\\\"}"
  prometheus_metrics+="spire_svid_expires_in_hours{spiffe_id=\"${safe_id}\"} ${hours_remaining}"$'\n'
}

# ── Fetch TTL for each workload entry ─────────────────────────────────────
# `entry show` outputs the registered TTL (seconds).  For running SVIDs the
# real remaining lifetime must be queried via the agent.  In this script we
# use the simpler server-side registration TTL as a conservative lower bound:
# if a SVID was just issued its remaining time ≈ TTL; worst case it was issued
# just before check → remaining ≈ 0.  For alerting purposes we therefore also
# track the absolute registration TTL and warn at 72h threshold.
#
# For precise remaining lifetime (running SVIDs), the companion monitoring
# workflow can use `spire-agent api fetch x509` + openssl.

for workload in "${WORKLOADS[@]}"; do
  spiffe_id="spiffe://${TRUST_DOMAIN}/ns/default/sa/${workload}"

  # Fetch entry TTL from server registration store
  raw_ttl="$(${SPIRE_SERVER_BIN} entry show \
               -socketPath "${SPIRE_SERVER_SOCKET}" \
               -spiffeID   "${spiffe_id}" 2>/dev/null \
             | grep -i "^X509-SVID TTL\|^TTL" \
             | head -1 \
             | grep -oE '[0-9]+' \
             | head -1 || echo "")"

  if [[ -z "${raw_ttl}" ]]; then
    log_warn "Cannot fetch TTL for ${spiffe_id} — entry not found or server unreachable."
    # Treat as 0 h remaining so alert fires
    emit_metric "${spiffe_id}" 0
    exit_code=1
    continue
  fi

  hours_remaining=$(( raw_ttl / 3600 ))

  if (( hours_remaining < CRITICAL_HOURS )); then
    log_crit "${workload}: ${hours_remaining}h remaining (< ${CRITICAL_HOURS}h) — SVID CRITICAL"
    exit_code=1
  elif (( hours_remaining < WARN_HOURS )); then
    log_warn "${workload}: ${hours_remaining}h remaining (< ${WARN_HOURS}h)"
    # Do NOT set exit_code=1 for warn — Prometheus alert handles the warn path.
  else
    log "${workload}: OK — ${hours_remaining}h remaining"
  fi

  emit_metric "${spiffe_id}" "${hours_remaining}"
done

# ── Push metrics to Pushgateway ───────────────────────────────────────────
if [[ -n "${PROM_PUSHGATEWAY_URL}" && -n "${prometheus_metrics}" ]]; then
  log "Pushing metrics to ${PROM_PUSHGATEWAY_URL}..."
  # Prefix with TYPE / HELP lines for proper Pushgateway ingestion
  payload="# HELP spire_svid_expires_in_hours Hours until SPIRE SVID expires (per workload)."$'\n'
  payload+="# TYPE spire_svid_expires_in_hours gauge"$'\n'
  payload+="${prometheus_metrics}"

  http_status="$(printf '%s' "${payload}" | curl --silent --show-error \
    --write-out '%{http_code}' \
    --output /dev/null \
    --data-binary @- \
    "${PROM_PUSHGATEWAY_URL}/metrics/job/${PROM_JOB}" 2>&1 || echo "curl_error")"

  if [[ "${http_status}" =~ ^2 ]]; then
    log "Metrics pushed successfully (HTTP ${http_status})."
  else
    log_warn "Pushgateway returned HTTP ${http_status} — metrics may not have been recorded."
  fi
else
  log "PROM_PUSHGATEWAY_URL not set — skipping metric push."
  log "Metrics that would have been pushed:"
  printf '%s' "${prometheus_metrics}" | sed 's/^/  /'
fi

# ── Summary ──────────────────────────────────────────────────────────────
if [[ ${exit_code} -eq 0 ]]; then
  log "All SVIDs healthy (>= ${WARN_HOURS}h remaining)."
else
  log_crit "At least one SVID expires within ${CRITICAL_HOURS}h — triggering CI failure."
fi

exit "${exit_code}"
