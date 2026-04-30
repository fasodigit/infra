#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# =============================================================================
# sync-hansen-gfc.sh — TERROIR P1.C
#
# Mirror Hansen Global Forest Change v1.11 (University of Maryland UMD/GLAD)
# vers le bucket MinIO souverain `geo-mirror`. Permet à `terroir-eudr` de
# valider parcelles offline-first contre un dataset interne, sans dépendance
# runtime sur USGS/UMD.
#
# Source upstream : https://storage.googleapis.com/earthenginepartners-hansen/
# License (Hansen et al. 2013, Science 342:850-853) : CC BY 4.0
#   → usage commercial OK, attribution obligatoire dans toute évidence DDS
#     générée (cf. INFRA/terroir/docs/LICENSES-GEO.md).
#
# Cible MinIO :
#   - bucket : geo-mirror
#   - prefix : hansen-gfc/v1.11/
#   - layers : lossyear, treecover2000, datamask
#   - tiles  : 4 tuiles 10°×10° couvrant le Burkina Faso (lat 9-15N, lon -6—2E)
#                10N_010W, 10N_000E, 20N_010W, 20N_000E
#
# Versioning : figé v1.11. Toute bascule vers v1.12+ exige validation manuelle
# (cf. RUNBOOK-GEO-MIRRORS.md §refresh-policy).
#
# Usage :
#   bash INFRA/scripts/sync-hansen-gfc.sh
#
# Variables d'env (overrides) :
#   HANSEN_VERSION    (défaut : v1.11)
#   MINIO_ENDPOINT    (défaut : http://localhost:9201)
#   MINIO_ALIAS       (défaut : faso)
#   MINIO_BUCKET      (défaut : geo-mirror)
#   MINIO_ACCESS_KEY  (défaut : faso-dev-access-key)
#   MINIO_SECRET_KEY  (défaut : faso-dev-secret-key-change-me-32c)
#   DRY_RUN=1         (skip downloads, log seulement)
#   SMOKE_TEST=1      (1 seul tile×layer pour validation flow end-to-end)
#
# Pré-requis : `mc` (MinIO Client) ou `podman exec faso-minio mc ...`,
# `curl`, `jq`. Voir INFRA/terroir/docs/RUNBOOK-GEO-MIRRORS.md.
# =============================================================================

set -euo pipefail

VERSION="${HANSEN_VERSION:-v1.11}"
# La dernière publication GFC v1.11 contient les données jusqu'à fin 2023
# (release Apr 2024). Source : https://storage.googleapis.com/earthenginepartners-hansen/GFC-2023-v1.11/
GFC_YEAR="${HANSEN_GFC_YEAR:-2023}"
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

log() { echo "[hansen-gfc-sync] $*"; }
err() { echo "[hansen-gfc-sync] ERROR: $*" >&2; }

# ---- Sanity checks --------------------------------------------------------
for bin in curl jq; do
  if ! command -v "$bin" >/dev/null 2>&1; then
    err "$bin manquant dans PATH"
    exit 1
  fi
done

# `mc` peut être local OU exécuté dans le conteneur faso-minio.
# Si fallback exec, l'endpoint vu DEPUIS le conteneur n'est pas le port loopback
# host — c'est le service interne `http://localhost:9000` (port natif MinIO).
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
  err "  install : https://min.io/docs/minio/linux/reference/minio-mc.html"
  err "  ou démarre le stack observability (cf. RUNBOOK-GEO-MIRRORS.md)"
  exit 1
fi

# Quand mc tourne dans le conteneur faso-minio, on cible le port natif interne.
if [[ "$MC_EXEC_INSIDE" == "1" && "$MINIO_ENDPOINT" == "http://localhost:9201" ]]; then
  MINIO_ENDPOINT_EFFECTIVE="http://localhost:9000"
  log "  (endpoint ajusté pour exec interne : $MINIO_ENDPOINT_EFFECTIVE)"
else
  MINIO_ENDPOINT_EFFECTIVE="$MINIO_ENDPOINT"
fi

# ---- Setup mc alias -------------------------------------------------------
log "Setup mc alias '$MINIO_ALIAS' → $MINIO_ENDPOINT_EFFECTIVE"
"${MC[@]}" alias set "$MINIO_ALIAS" "$MINIO_ENDPOINT_EFFECTIVE" "$MINIO_ACCESS" "$MINIO_SECRET" >/dev/null 2>&1 || {
  err "mc alias set a échoué — MinIO joignable sur $MINIO_ENDPOINT_EFFECTIVE ?"
  exit 1
}
"${MC[@]}" mb --ignore-existing "$MINIO_ALIAS/$BUCKET" >/dev/null

# ---- Upload helper --------------------------------------------------------
# Si mc tourne local : `mc cp src dst`. Si exec interne : pipe via stdin
# (le tmpdir host n'est pas mounté dans le conteneur faso-minio).
mc_upload() {
  local src="$1" dst="$2"
  if [[ "$MC_EXEC_INSIDE" == "1" ]]; then
    # shellcheck disable=SC2002
    cat "$src" | "${MC[@]}" pipe "$dst" >/dev/null
  else
    "${MC[@]}" cp "$src" "$dst" >/dev/null
  fi
}

# ---- Tiles & layers --------------------------------------------------------
# 4 tuiles 10°×10° couvrant Burkina Faso et zones limitrophes
TILES_BF=("10N_010W" "10N_000E" "20N_010W" "20N_000E")
LAYERS=("lossyear" "treecover2000" "datamask")

if [[ "$SMOKE_TEST" == "1" ]]; then
  log "SMOKE_TEST=1 — restreindre à 1 tile × 1 layer"
  TILES_BF=("10N_010W")
  LAYERS=("datamask")  # datamask est le plus petit (~50MB), idéal smoke
fi

BASE_URL="https://storage.googleapis.com/earthenginepartners-hansen/GFC-${GFC_YEAR}-${VERSION}"

# ---- Download + upload tiles ----------------------------------------------
COUNT_DOWNLOADED=0
COUNT_SKIPPED=0
COUNT_FAILED=0

for tile in "${TILES_BF[@]}"; do
  for layer in "${LAYERS[@]}"; do
    file="Hansen_GFC-${GFC_YEAR}-${VERSION}_${layer}_${tile}.tif"
    s3path="$MINIO_ALIAS/$BUCKET/hansen-gfc/${VERSION}/${file}"

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
    if curl -fsSL --retry 3 --retry-delay 5 -o "$WORK/$file" "${BASE_URL}/${file}"; then
      log "  ⬆ uploading to $s3path"
      mc_upload "$WORK/$file" "$s3path"
      rm -f "$WORK/$file"
      COUNT_DOWNLOADED=$((COUNT_DOWNLOADED + 1))
    else
      err "  ✗ failed to fetch $file (HTTP error)"
      COUNT_FAILED=$((COUNT_FAILED + 1))
    fi
  done
done

# ---- MANIFEST.json (audit trail) ------------------------------------------
log "Writing MANIFEST.json"
TILES_JSON="$(printf '%s\n' "${TILES_BF[@]}" | jq -R . | jq -s .)"
LAYERS_JSON="$(printf '%s\n' "${LAYERS[@]}" | jq -R . | jq -s .)"

cat > "$WORK/MANIFEST.json" <<EOF
{
  "dataset": "hansen-gfc",
  "version": "${VERSION}",
  "gfc_year": "${GFC_YEAR}",
  "license": "CC BY 4.0",
  "license_url": "https://creativecommons.org/licenses/by/4.0/",
  "source_url": "${BASE_URL}/",
  "citation": "Hansen, M.C., et al. 2013. High-Resolution Global Maps of 21st-Century Forest Cover Change. Science 342: 850-853.",
  "attribution_url": "https://glad.umd.edu/dataset/global-forest-change",
  "tiles": ${TILES_JSON},
  "layers": ${LAYERS_JSON},
  "synced_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "bucket": "${BUCKET}",
  "prefix": "hansen-gfc/${VERSION}/",
  "downloaded": ${COUNT_DOWNLOADED},
  "skipped": ${COUNT_SKIPPED},
  "failed": ${COUNT_FAILED},
  "smoke_test": ${SMOKE_TEST}
}
EOF

if [[ "$DRY_RUN" != "1" ]]; then
  mc_upload "$WORK/MANIFEST.json" "$MINIO_ALIAS/$BUCKET/hansen-gfc/${VERSION}/MANIFEST.json"
fi

# ---- Summary --------------------------------------------------------------
log "Hansen GFC ${VERSION} mirror summary :"
log "  downloaded : $COUNT_DOWNLOADED"
log "  skipped    : $COUNT_SKIPPED (already mirrored)"
log "  failed     : $COUNT_FAILED"

if [[ "$DRY_RUN" != "1" ]]; then
  log "  bucket recap :"
  "${MC[@]}" ls --recursive --summarize "$MINIO_ALIAS/$BUCKET/hansen-gfc/${VERSION}/" 2>/dev/null | tail -3 || true
fi

if [[ "$COUNT_FAILED" -gt 0 ]]; then
  err "Sync incomplet — $COUNT_FAILED tile(s) en échec"
  exit 2
fi

log "OK — Hansen GFC ${VERSION} mirror à jour"
