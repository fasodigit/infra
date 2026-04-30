#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# =============================================================================
# download-geolite2.sh — Phase 4.b.6 risk-based scoring (auth-ms)
#
# Downloads the MaxMind GeoLite2-City.mmdb database used by GeoIpResolver to
# resolve a request IP into {country, city, lat, lon}. License: CC BY-SA 4.0
# (compatible with AGPL-3.0-or-later).
#
# Pre-requisite : a MaxMind license key.
#   * For dev / CI : place it in $MAXMIND_LICENSE_KEY before invoking the
#     script, or fetch it from Vault :
#         export MAXMIND_LICENSE_KEY=$(vault kv get -field=value \
#             faso/auth-ms/maxmind-license-key)
#   * For prod    : provisioned by the platform's secret store.
#
# Outputs the .mmdb at the destination path (default: ./GeoLite2-City.mmdb).
# In production we mount it at /var/lib/auth-ms/GeoLite2-City.mmdb (matches
# the default of `${admin.geoip.database-path}` in application.yml).
#
# Usage :
#   bash INFRA/scripts/download-geolite2.sh                        # → ./GeoLite2-City.mmdb
#   bash INFRA/scripts/download-geolite2.sh /var/lib/auth-ms/GeoLite2-City.mmdb
# =============================================================================

set -euo pipefail

KEY="${MAXMIND_LICENSE_KEY:?Need MAXMIND_LICENSE_KEY (vault kv get faso/auth-ms/maxmind-license-key)}"
DEST="${1:-./GeoLite2-City.mmdb}"
URL="https://download.maxmind.com/app/geoip_download?edition_id=GeoLite2-City&license_key=${KEY}&suffix=tar.gz"

log() { echo "[geolite2] $*"; }

if ! command -v curl >/dev/null 2>&1; then
  echo "[geolite2] ERROR: curl not in PATH" >&2
  exit 1
fi
if ! command -v tar >/dev/null 2>&1; then
  echo "[geolite2] ERROR: tar not in PATH" >&2
  exit 1
fi

TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT

log "Downloading GeoLite2-City to ${TMP}/geo.tgz ..."
curl -fsSL --retry 3 --retry-delay 5 -o "${TMP}/geo.tgz" "${URL}"

log "Extracting archive ..."
tar -xzf "${TMP}/geo.tgz" -C "${TMP}"

MMDB="$(find "${TMP}" -name "GeoLite2-City.mmdb" -print -quit)"
if [[ -z "${MMDB}" ]]; then
  echo "[geolite2] ERROR: GeoLite2-City.mmdb not found in archive" >&2
  exit 1
fi

# Make sure the destination directory exists.
mkdir -p "$(dirname "${DEST}")"
mv "${MMDB}" "${DEST}"

log "OK — GeoLite2-City.mmdb available at ${DEST}"
log "    size : $(stat -c %s "${DEST}" 2>/dev/null || stat -f %z "${DEST}") bytes"
log "    sha256 (informative) : $(sha256sum "${DEST}" 2>/dev/null | awk '{print $1}' || echo n/a)"
