#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# =============================================================================
# sync-jrc-tmf.sh — TERROIR P1.C
#
# Mirror EU JRC Tropical Moist Forest dataset vers le bucket MinIO souverain
# `geo-mirror`. Permet à `terroir-eudr` de croiser Hansen GFC avec la source
# autoritaire UE pour conformité EUDR.
#
# Source upstream : https://forobs.jrc.ec.europa.eu/TMF/
# License : Open Data PSI 2019 (équivalent CC BY 4.0).
#   Vancutsem, C., et al. 2021. Long-term (1990-2019) monitoring of forest
#   cover changes in the humid tropics. Science Advances 7, eabe1603.
#   → compatible AGPL ; attribution obligatoire dans toute évidence DDS.
#
# Cible MinIO :
#   - bucket : geo-mirror
#   - prefix : jrc-tmf/v1_2024/
#   - tiles  : couverture Afrique de l'Ouest (BFA, CIV, GHA) — pertinent pour
#              les coopératives cajou/cacao en limite zone tropicale humide.
#              BF lui-même = Sahel/Soudano-Sahel, peu de TMF natif ; CIV+GHA
#              fournissent les références déforestation post-2020 utiles.
#
# Note : le format de download JRC TMF utilise des noms de fichier par bloc
# continental (AnnualChange_*) accessibles via l'IFORCE download portal. Les
# URLs exactes peuvent changer ; en cas d'échec HTTP, fallback `manual fetch`
# via le portail web (cf. RUNBOOK-GEO-MIRRORS.md §jrc-manual).
#
# Versioning : figé v1_2024 (dernière publication JRC). Bascule vers v1_2025
# exige validation manuelle.
#
# Usage :
#   bash INFRA/scripts/sync-jrc-tmf.sh
#
# Variables d'env (overrides) :
#   JRC_TMF_VERSION   (défaut : v1_2024)
#   MINIO_ENDPOINT    (défaut : http://localhost:9201)
#   MINIO_ALIAS       (défaut : faso)
#   MINIO_BUCKET      (défaut : geo-mirror)
#   MINIO_ACCESS_KEY  (défaut : faso-dev-access-key)
#   MINIO_SECRET_KEY  (défaut : faso-dev-secret-key-change-me-32c)
#   DRY_RUN=1
#   SMOKE_TEST=1
# =============================================================================

set -euo pipefail

VERSION="${JRC_TMF_VERSION:-v1_2024}"
BUCKET="${MINIO_BUCKET:-geo-mirror}"
MINIO_ALIAS="${MINIO_ALIAS:-faso}"
MINIO_ENDPOINT="${MINIO_ENDPOINT:-http://localhost:9201}"
MINIO_ACCESS="${MINIO_ACCESS_KEY:-faso-dev-access-key}"
MINIO_SECRET="${MINIO_SECRET_KEY:-faso-dev-secret-key-change-me-32c}"
DRY_RUN="${DRY_RUN:-0}"
SMOKE_TEST="${SMOKE_TEST:-0}"

WORK="$(mktemp -d)"
# shellcheck disable=SC2064
trap "rm -rf '$WORK'" EXIT

log() { echo "[jrc-tmf-sync] $*"; }
warn() { echo "[jrc-tmf-sync] WARN: $*" >&2; }
err() { echo "[jrc-tmf-sync] ERROR: $*" >&2; }

# ---- Sanity checks --------------------------------------------------------
for bin in curl jq; do
  if ! command -v "$bin" >/dev/null 2>&1; then
    err "$bin manquant dans PATH"
    exit 1
  fi
done

MC_EXEC_INSIDE=0
if command -v mc >/dev/null 2>&1; then
  MC=(mc)
elif command -v podman >/dev/null 2>&1 && podman ps --format '{{.Names}}' | grep -q '^faso-minio$'; then
  log "mc local non trouvé — fallback sur 'podman exec faso-minio mc'"
  MC=(podman exec -i faso-minio mc)
  MC_EXEC_INSIDE=1
elif command -v docker >/dev/null 2>&1 && docker ps --format '{{.Names}}' 2>/dev/null | grep -q '^faso-minio$'; then
  log "mc local non trouvé — fallback sur 'docker exec faso-minio mc'"
  MC=(docker exec -i faso-minio mc)
  MC_EXEC_INSIDE=1
else
  err "ni 'mc' (MinIO Client) ni conteneur faso-minio disponibles"
  exit 1
fi

if [[ "$MC_EXEC_INSIDE" == "1" && "$MINIO_ENDPOINT" == "http://localhost:9201" ]]; then
  MINIO_ENDPOINT_EFFECTIVE="http://localhost:9000"
  log "  (endpoint ajusté pour exec interne : $MINIO_ENDPOINT_EFFECTIVE)"
else
  MINIO_ENDPOINT_EFFECTIVE="$MINIO_ENDPOINT"
fi

log "Setup mc alias '$MINIO_ALIAS' → $MINIO_ENDPOINT_EFFECTIVE"
"${MC[@]}" alias set "$MINIO_ALIAS" "$MINIO_ENDPOINT_EFFECTIVE" "$MINIO_ACCESS" "$MINIO_SECRET" >/dev/null 2>&1 || {
  err "mc alias set a échoué — MinIO joignable sur $MINIO_ENDPOINT_EFFECTIVE ?"
  exit 1
}
"${MC[@]}" mb --ignore-existing "$MINIO_ALIAS/$BUCKET" >/dev/null

mc_upload() {
  local src="$1" dst="$2"
  if [[ "$MC_EXEC_INSIDE" == "1" ]]; then
    # shellcheck disable=SC2002
    cat "$src" | "${MC[@]}" pipe "$dst" >/dev/null
  else
    "${MC[@]}" cp "$src" "$dst" >/dev/null
  fi
}

# ---- Tiles JRC TMF --------------------------------------------------------
# Couverture pertinente Afrique de l'Ouest pour terroir-eudr P1
TILES_AFRIQUE_OUEST=("AnnualChange_BFA" "AnnualChange_CIV" "AnnualChange_GHA")

if [[ "$SMOKE_TEST" == "1" ]]; then
  log "SMOKE_TEST=1 — restreindre à 1 tile"
  TILES_AFRIQUE_OUEST=("AnnualChange_CIV")
fi

# IFORCE download endpoint — peut nécessiter ajustement selon évolution portail
BASE_URL="https://ies-ows.jrc.ec.europa.eu/iforce/tmf_v1/download"

COUNT_DOWNLOADED=0
COUNT_SKIPPED=0
COUNT_FAILED=0

for tile in "${TILES_AFRIQUE_OUEST[@]}"; do
  file="${tile}.tif"
  s3path="$MINIO_ALIAS/$BUCKET/jrc-tmf/${VERSION}/${file}"

  if "${MC[@]}" stat "$s3path" >/dev/null 2>&1; then
    log "  ✓ $file (already mirrored)"
    COUNT_SKIPPED=$((COUNT_SKIPPED + 1))
    continue
  fi

  if [[ "$DRY_RUN" == "1" ]]; then
    log "  [DRY] would download $file"
    continue
  fi

  log "  ⬇ downloading $file..."
  if curl -fsSL --retry 3 --retry-delay 5 -o "$WORK/$file" "${BASE_URL}/${tile}"; then
    log "  ⬆ uploading to $s3path"
    mc_upload "$WORK/$file" "$s3path"
    rm -f "$WORK/$file"
    COUNT_DOWNLOADED=$((COUNT_DOWNLOADED + 1))
  else
    warn "  ⚠ $file not available — JRC TMF download URL may have changed"
    warn "    fallback : manual fetch via https://forobs.jrc.ec.europa.eu/TMF/"
    warn "    et upload : mc cp <file> $s3path"
    COUNT_FAILED=$((COUNT_FAILED + 1))
  fi
done

# ---- MANIFEST.json --------------------------------------------------------
log "Writing MANIFEST.json"
TILES_JSON="$(printf '%s\n' "${TILES_AFRIQUE_OUEST[@]}" | jq -R . | jq -s .)"

cat > "$WORK/MANIFEST.json" <<EOF
{
  "dataset": "jrc-tmf",
  "version": "${VERSION}",
  "license": "CC BY 4.0 (Open Data PSI 2019)",
  "license_url": "https://creativecommons.org/licenses/by/4.0/",
  "source_url": "https://forobs.jrc.ec.europa.eu/TMF/",
  "citation": "Vancutsem, C., et al. 2021. Long-term (1990-2019) monitoring of forest cover changes in the humid tropics. Science Advances 7, eabe1603.",
  "attribution_url": "https://forobs.jrc.ec.europa.eu/TMF",
  "tiles": ${TILES_JSON},
  "synced_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "bucket": "${BUCKET}",
  "prefix": "jrc-tmf/${VERSION}/",
  "downloaded": ${COUNT_DOWNLOADED},
  "skipped": ${COUNT_SKIPPED},
  "failed": ${COUNT_FAILED},
  "smoke_test": ${SMOKE_TEST}
}
EOF

if [[ "$DRY_RUN" != "1" ]]; then
  mc_upload "$WORK/MANIFEST.json" "$MINIO_ALIAS/$BUCKET/jrc-tmf/${VERSION}/MANIFEST.json"
fi

log "JRC TMF ${VERSION} mirror summary :"
log "  downloaded : $COUNT_DOWNLOADED"
log "  skipped    : $COUNT_SKIPPED (already mirrored)"
log "  failed     : $COUNT_FAILED"

if [[ "$DRY_RUN" != "1" ]]; then
  log "  bucket recap :"
  "${MC[@]}" ls --recursive --summarize "$MINIO_ALIAS/$BUCKET/jrc-tmf/${VERSION}/" 2>/dev/null | tail -3 || true
fi

if [[ "$COUNT_FAILED" -gt 0 ]]; then
  warn "Sync incomplet — $COUNT_FAILED tile(s) en échec ; manual fetch requis"
  # Exit 2 = warning (compat avec scheduler — distinguer error vrai)
  exit 2
fi

log "OK — JRC TMF ${VERSION} mirror à jour"
