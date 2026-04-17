#!/usr/bin/env bash
# sync-dashboards.sh — Push FASO dashboards to Grafana API
# Usage: GRAFANA_TOKEN=xxx GRAFANA_URL=https://grafana.example.com ./sync-dashboards.sh
# Optional: GRAFANA_ORG_ID (default: 1)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DASHBOARDS_DIR="${SCRIPT_DIR}/../dashboards"
GRAFANA_URL="${GRAFANA_URL:-http://localhost:3000}"
GRAFANA_ORG_ID="${GRAFANA_ORG_ID:-1}"
FOLDER_UID="faso-digitalisation-folder"
FOLDER_TITLE="FASO DIGITALISATION"

# Validate required env vars
if [[ -z "${GRAFANA_TOKEN:-}" ]]; then
  echo "[ERROR] GRAFANA_TOKEN environment variable is required" >&2
  exit 1
fi

AUTH_HEADER="Authorization: Bearer ${GRAFANA_TOKEN}"
CONTENT_TYPE="Content-Type: application/json"
BASE_URL="${GRAFANA_URL}/api"

# ────────────────────────────────────────────────────────────
# Helper: API call with error reporting
# ────────────────────────────────────────────────────────────
api_call() {
  local method="$1"
  local endpoint="$2"
  local data="${3:-}"
  local response http_code

  if [[ -n "$data" ]]; then
    response=$(curl -s -w "\n%{http_code}" -X "$method" \
      -H "$AUTH_HEADER" -H "$CONTENT_TYPE" \
      -H "X-Grafana-Org-Id: ${GRAFANA_ORG_ID}" \
      -d "$data" \
      "${BASE_URL}${endpoint}")
  else
    response=$(curl -s -w "\n%{http_code}" -X "$method" \
      -H "$AUTH_HEADER" \
      -H "X-Grafana-Org-Id: ${GRAFANA_ORG_ID}" \
      "${BASE_URL}${endpoint}")
  fi

  http_code=$(echo "$response" | tail -1)
  body=$(echo "$response" | head -n -1)

  if [[ "$http_code" -ge 400 ]]; then
    echo "[ERROR] HTTP ${http_code} for ${method} ${endpoint}: ${body}" >&2
    return 1
  fi
  echo "$body"
}

# ────────────────────────────────────────────────────────────
# Ensure folder exists
# ────────────────────────────────────────────────────────────
ensure_folder() {
  echo "[INFO] Ensuring folder '${FOLDER_TITLE}' exists..."
  local folder_payload
  folder_payload=$(jq -nc \
    --arg uid "$FOLDER_UID" \
    --arg title "$FOLDER_TITLE" \
    '{"uid": $uid, "title": $title}')

  # Try to get folder first
  if api_call GET "/folders/${FOLDER_UID}" > /dev/null 2>&1; then
    echo "[INFO] Folder '${FOLDER_TITLE}' already exists."
  else
    api_call POST "/folders" "$folder_payload" > /dev/null
    echo "[INFO] Folder '${FOLDER_TITLE}' created."
  fi
}

# ────────────────────────────────────────────────────────────
# Push a single dashboard
# ────────────────────────────────────────────────────────────
push_dashboard() {
  local dashboard_file="$1"
  local dashboard_name
  dashboard_name=$(basename "$dashboard_file" .json)

  echo "[INFO] Pushing dashboard: ${dashboard_name}..."

  # Read dashboard JSON
  local dashboard_json
  dashboard_json=$(jq '.' "$dashboard_file")

  # Wrap in Grafana import/update payload
  local payload
  payload=$(jq -n \
    --argjson dashboard "$dashboard_json" \
    --arg folderUid "$FOLDER_UID" \
    '{
      "dashboard": ($dashboard | del(.__inputs, .__requires) | .id = null),
      "folderUid": $folderUid,
      "overwrite": true,
      "message": "Automated sync from git"
    }')

  local result
  result=$(api_call POST "/dashboards/db" "$payload")

  local status uid
  status=$(echo "$result" | jq -r '.status // "unknown"')
  uid=$(echo "$result" | jq -r '.uid // "unknown"')

  echo "[OK] ${dashboard_name} — status: ${status}, uid: ${uid}"
}

# ────────────────────────────────────────────────────────────
# Main
# ────────────────────────────────────────────────────────────
main() {
  echo "======================================================"
  echo " FASO DIGITALISATION — Grafana Dashboard Sync"
  echo " Target: ${GRAFANA_URL}"
  echo " Org ID: ${GRAFANA_ORG_ID}"
  echo "======================================================"

  # Test connectivity
  echo "[INFO] Testing Grafana connectivity..."
  api_call GET "/health" > /dev/null
  echo "[INFO] Grafana is reachable."

  ensure_folder

  # Push all dashboards
  local success=0
  local failed=0

  for dashboard_file in "${DASHBOARDS_DIR}"/*.json; do
    if [[ -f "$dashboard_file" ]]; then
      if push_dashboard "$dashboard_file"; then
        ((success++)) || true
      else
        ((failed++)) || true
        echo "[WARN] Failed to push: $(basename "$dashboard_file")" >&2
      fi
    fi
  done

  echo "======================================================"
  echo " Sync complete: ${success} succeeded, ${failed} failed"
  echo "======================================================"

  if [[ $failed -gt 0 ]]; then
    exit 1
  fi
}

main "$@"
