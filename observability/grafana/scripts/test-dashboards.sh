#!/usr/bin/env bash
# test-dashboards.sh — Validate Grafana dashboard JSON structure via jq
# Usage: ./test-dashboards.sh [dashboards_dir]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DASHBOARDS_DIR="${1:-${SCRIPT_DIR}/../dashboards}"

# ────────────────────────────────────────────────────────────
# Required fields per dashboard
# ────────────────────────────────────────────────────────────
REQUIRED_TOP_LEVEL_FIELDS=("title" "uid" "schemaVersion" "tags" "panels" "time" "refresh")
REQUIRED_TAGS_PREFIX="sovereignty=faso-digitalisation"
MIN_PANELS=6

# ────────────────────────────────────────────────────────────
# Counters
# ────────────────────────────────────────────────────────────
passed=0
failed=0
errors=()

# ────────────────────────────────────────────────────────────
# Helpers
# ────────────────────────────────────────────────────────────
log_pass() { echo "  [PASS] $1"; }
log_fail() { echo "  [FAIL] $1"; errors+=("$2: $1"); ((failed++)) || true; }

validate_dashboard() {
  local file="$1"
  local name
  name=$(basename "$file" .json)
  local file_errors=0

  echo ""
  echo "Validating: ${name}"
  echo "  File: ${file}"

  # 1. Valid JSON
  if ! jq '.' "$file" > /dev/null 2>&1; then
    log_fail "Invalid JSON" "$name"
    ((failed++)) || true
    return
  fi
  log_pass "Valid JSON"

  # 2. Required top-level fields
  for field in "${REQUIRED_TOP_LEVEL_FIELDS[@]}"; do
    local value
    value=$(jq -r ".$field // empty" "$file")
    if [[ -z "$value" || "$value" == "null" ]]; then
      log_fail "Missing required field: .$field" "$name"
      ((file_errors++)) || true
    else
      log_pass "Field .$field present"
    fi
  done

  # 3. UID is stable and non-empty
  local uid
  uid=$(jq -r '.uid' "$file")
  if [[ -z "$uid" || "$uid" == "null" ]]; then
    log_fail "Missing or null .uid" "$name"
    ((file_errors++)) || true
  elif [[ "$uid" == *"null"* ]]; then
    log_fail ".uid contains 'null': ${uid}" "$name"
    ((file_errors++)) || true
  else
    log_pass "UID stable: ${uid}"
  fi

  # 4. Sovereignty tag present
  local has_sovereignty_tag
  has_sovereignty_tag=$(jq -r '[.tags[]? | select(. == "sovereignty=faso-digitalisation")] | length' "$file")
  if [[ "$has_sovereignty_tag" -eq 0 ]]; then
    log_fail "Missing tag 'sovereignty=faso-digitalisation' in .tags" "$name"
    ((file_errors++)) || true
  else
    log_pass "Sovereignty tag present"
  fi

  # 5. Minimum panel count
  local panel_count
  panel_count=$(jq '[.panels[]? | select(.type != "row")] | length' "$file")
  if [[ "$panel_count" -lt "$MIN_PANELS" ]]; then
    log_fail "Insufficient panels: ${panel_count} < ${MIN_PANELS} required" "$name"
    ((file_errors++)) || true
  else
    log_pass "Panel count: ${panel_count} >= ${MIN_PANELS}"
  fi

  # 6. All non-row panels have a datasource configured
  local panels_without_ds
  panels_without_ds=$(jq '[.panels[]? | select(.type != "row" and (.datasource == null or .datasource == ""))] | length' "$file")
  if [[ "$panels_without_ds" -gt 0 ]]; then
    log_fail "${panels_without_ds} panel(s) missing datasource configuration" "$name"
    ((file_errors++)) || true
  else
    log_pass "All panels have datasource configured"
  fi

  # 7. All non-row panels have at least one target
  local panels_without_targets
  panels_without_targets=$(jq '[.panels[]? | select(.type != "row" and (.targets == null or (.targets | length) == 0))] | length' "$file")
  if [[ "$panels_without_targets" -gt 0 ]]; then
    log_fail "${panels_without_targets} panel(s) have no targets/queries" "$name"
    ((file_errors++)) || true
  else
    log_pass "All panels have targets defined"
  fi

  # 8. schemaVersion >= 36
  local schema_version
  schema_version=$(jq '.schemaVersion' "$file")
  if [[ "$schema_version" -lt 36 ]]; then
    log_fail "schemaVersion ${schema_version} is too old (need >= 36)" "$name"
    ((file_errors++)) || true
  else
    log_pass "schemaVersion: ${schema_version}"
  fi

  # 9. No duplicate panel IDs
  local duplicate_ids
  duplicate_ids=$(jq '[.panels[]?.id] | group_by(.) | map(select(length > 1)) | length' "$file")
  if [[ "$duplicate_ids" -gt 0 ]]; then
    log_fail "Duplicate panel IDs found" "$name"
    ((file_errors++)) || true
  else
    log_pass "No duplicate panel IDs"
  fi

  if [[ "$file_errors" -eq 0 ]]; then
    ((passed++)) || true
    echo "  Result: PASSED"
  else
    echo "  Result: FAILED (${file_errors} error(s))"
  fi
}

# ────────────────────────────────────────────────────────────
# Check jq availability
# ────────────────────────────────────────────────────────────
if ! command -v jq &> /dev/null; then
  echo "[ERROR] jq is required but not installed. Install with: apt-get install jq / brew install jq" >&2
  exit 1
fi

echo "======================================================"
echo " FASO DIGITALISATION — Dashboard JSON Validation"
echo " Directory: ${DASHBOARDS_DIR}"
echo "======================================================"

if [[ ! -d "$DASHBOARDS_DIR" ]]; then
  echo "[ERROR] Dashboards directory not found: ${DASHBOARDS_DIR}" >&2
  exit 1
fi

dashboard_count=0
for dashboard_file in "${DASHBOARDS_DIR}"/*.json; do
  if [[ -f "$dashboard_file" ]]; then
    validate_dashboard "$dashboard_file"
    ((dashboard_count++)) || true
  fi
done

echo ""
echo "======================================================"
echo " Validation Summary"
echo " Dashboards checked: ${dashboard_count}"
echo " Passed:  ${passed}"
echo " Failed:  ${failed}"

if [[ ${#errors[@]} -gt 0 ]]; then
  echo ""
  echo " Errors:"
  for err in "${errors[@]}"; do
    echo "   - ${err}"
  done
fi
echo "======================================================"

if [[ $failed -gt 0 ]]; then
  exit 1
fi

echo " All validations passed!"
exit 0
