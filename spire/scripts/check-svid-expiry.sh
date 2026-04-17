#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Verify every workload's SVID expires in > 72h. Pushes metrics to Pushgateway.
# Exits 1 if any SVID < 72h (triggers alert).
set -euo pipefail

WORKLOADS=("kaya" "armageddon" "auth-ms" "poulets-api" "notifier-ms")
WARN_THRESHOLD_SECS=259200   # 72h
CRITICAL_THRESHOLD_SECS=86400 # 24h
PUSHGATEWAY_URL="${PROM_PUSHGATEWAY_URL:-}"

exit_code=0
metrics=""

for workload in "${WORKLOADS[@]}"; do
  spiffe_id="spiffe://faso.gov.bf/ns/default/sa/${workload}"

  # Fetch via socket from any spire-agent pod (kubectl exec). In CI, use spire-agent in the same pod.
  if ! svid_info=$(kubectl exec -n spire -ti spire-agent-0 -- \
       /opt/spire/bin/spire-agent api fetch x509 -socketPath /run/spire/sockets/agent.sock \
       2>/dev/null | grep -A1 "$spiffe_id" | tail -1); then
    echo "ERROR: cannot fetch SVID for $workload" >&2
    exit_code=2
    continue
  fi

  not_after=$(echo "$svid_info" | openssl x509 -noout -enddate 2>/dev/null | cut -d= -f2 || echo "")
  if [[ -z "$not_after" ]]; then
    echo "ERROR: failed to parse SVID for $workload" >&2
    exit_code=2
    continue
  fi

  expiry_secs=$(( $(date -d "$not_after" +%s) - $(date +%s) ))
  metrics+="spire_svid_expiry_seconds{workload=\"${workload}\"} ${expiry_secs}"$'\n'

  if (( expiry_secs < CRITICAL_THRESHOLD_SECS )); then
    echo "CRITICAL: $workload SVID expires in ${expiry_secs}s (< 24h)" >&2
    exit_code=1
  elif (( expiry_secs < WARN_THRESHOLD_SECS )); then
    echo "WARN: $workload SVID expires in ${expiry_secs}s (< 72h)" >&2
    [[ $exit_code -eq 0 ]] && exit_code=1
  else
    echo "OK: $workload SVID expires in ${expiry_secs}s"
  fi
done

if [[ -n "$PUSHGATEWAY_URL" ]]; then
  printf '%s' "$metrics" | curl --data-binary @- "${PUSHGATEWAY_URL}/metrics/job/spire-svid-monitor"
fi

exit $exit_code
