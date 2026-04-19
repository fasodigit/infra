#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION
#
# Synthetic monitoring driver.
# Loops every 5 minutes, runs each flow in parallel with a 2-minute timeout,
# retries once on transient network errors, then pushes metrics to
# Prometheus Pushgateway (the pushing is done inside each spec via
# helpers/prometheus-push.ts).
#
# Exit codes:
#   0 — normal shutdown (SIGTERM)
#   Any non-zero from individual flows is logged but does NOT terminate
#   the loop: a failure must not stop the monitor.

set -u -o pipefail

cd "$(dirname "$0")/.."

FLOWS=(auth poulets-order etat-civil)
FLOW_TIMEOUT="${FLOW_TIMEOUT:-120s}"
LOOP_INTERVAL="${LOOP_INTERVAL:-300}" # seconds

log() { printf '[%s] %s\n' "$(date -Iseconds)" "$*"; }

shutting_down=0
trap 'shutting_down=1; log "received SIGTERM/SIGINT, exiting loop"; exit 0' TERM INT

run_flow() {
  local flow="$1"
  local spec="playwright/flows/${flow}.spec.ts"
  local attempt=1
  local max_attempts=2

  while [[ "$attempt" -le "$max_attempts" ]]; do
    log "flow=${flow} attempt=${attempt} starting"
    if timeout --foreground --kill-after=10s "$FLOW_TIMEOUT" \
         npx playwright test "$spec" --reporter=list; then
      log "flow=${flow} attempt=${attempt} OK"
      return 0
    fi
    local rc=$?
    log "flow=${flow} attempt=${attempt} FAILED rc=${rc}"
    if [[ "$attempt" -ge "$max_attempts" ]]; then
      return "$rc"
    fi
    # Retry only on likely-transient codes (124 timeout, 1 generic).
    if [[ "$rc" -eq 124 || "$rc" -eq 1 ]]; then
      sleep 5
      attempt=$((attempt + 1))
      continue
    fi
    return "$rc"
  done
}

main() {
  log "synthetic monitor starting (flows=${FLOWS[*]} interval=${LOOP_INTERVAL}s)"

  while [[ "$shutting_down" -eq 0 ]]; do
    local iter_start
    iter_start=$(date +%s)

    local pids=()
    for flow in "${FLOWS[@]}"; do
      run_flow "$flow" &
      pids+=("$!")
    done

    for pid in "${pids[@]}"; do
      wait "$pid" || true
    done

    local elapsed=$(( $(date +%s) - iter_start ))
    local sleep_for=$(( LOOP_INTERVAL - elapsed ))
    if [[ "$sleep_for" -lt 10 ]]; then sleep_for=10; fi
    log "iteration complete in ${elapsed}s — sleeping ${sleep_for}s"
    sleep "$sleep_for" &
    wait $!
  done
}

main "$@"
