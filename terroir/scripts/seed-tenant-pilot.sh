#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# seed-tenant-pilot.sh — Creates the t_pilot tenant via terroir-admin :9904.
# Idempotent: silently succeeds if the tenant already exists (HTTP 409).
#
# Usage:
#   bash INFRA/terroir/scripts/seed-tenant-pilot.sh
#
# Override admin URL:
#   TERROIR_ADMIN_URL=http://localhost:9904 bash seed-tenant-pilot.sh

set -euo pipefail

ADMIN_URL="${TERROIR_ADMIN_URL:-http://localhost:9904}"
SLUG="t_pilot"

echo "[seed-tenant-pilot] target: $ADMIN_URL"
echo "[seed-tenant-pilot] provisioning tenant slug=$SLUG"

HTTP_STATUS=$(curl -o /tmp/terroir_pilot_resp.json -w "%{http_code}" \
  -fsS -X POST "$ADMIN_URL/admin/tenants" \
  -H 'Content-Type: application/json' \
  -d '{
    "slug": "t_pilot",
    "legal_name": "Coopérative Pilote BF",
    "country_iso2": "BF",
    "region": "Boucle du Mouhoun",
    "primary_crop": "coton"
  }' 2>&1) || HTTP_STATUS="000"

BODY=$(cat /tmp/terroir_pilot_resp.json 2>/dev/null || echo '{}')

if [ "$HTTP_STATUS" = "201" ]; then
  echo "[seed-tenant-pilot] SUCCESS (201 Created)"
  echo "$BODY" | jq .
elif [ "$HTTP_STATUS" = "409" ]; then
  echo "[seed-tenant-pilot] ALREADY EXISTS (409 Conflict) — idempotent, OK"
  echo "$BODY" | jq .
elif [ "$HTTP_STATUS" = "000" ]; then
  echo "[seed-tenant-pilot] ERROR: terroir-admin unreachable at $ADMIN_URL" >&2
  echo "  Is terroir-admin running? Try: cargo run -p terroir-admin" >&2
  exit 1
else
  echo "[seed-tenant-pilot] UNEXPECTED HTTP $HTTP_STATUS" >&2
  echo "$BODY" | jq . >&2
  exit 1
fi

# Verify the tenant is ACTIVE
echo ""
echo "[seed-tenant-pilot] verifying tenant status..."
VERIFY=$(curl -fsS "$ADMIN_URL/admin/tenants/$SLUG" 2>/dev/null || echo '{}')
STATUS=$(echo "$VERIFY" | jq -r '.status // "unknown"')

if [ "$STATUS" = "ACTIVE" ]; then
  echo "[seed-tenant-pilot] tenant $SLUG is ACTIVE"
  echo "$VERIFY" | jq '{id, slug, status, schema_name, audit_schema_name}'
else
  echo "[seed-tenant-pilot] WARNING: tenant status is '$STATUS' (expected ACTIVE)" >&2
  echo "$VERIFY" | jq . >&2
  exit 1
fi
