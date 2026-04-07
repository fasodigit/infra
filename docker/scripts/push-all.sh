#!/usr/bin/env bash
# =============================================================================
# Push all container images to Docker Hub for FASO DIGITALISATION
# =============================================================================
# Usage: ./push-all.sh [--dry-run]
# Prerequisite: podman login docker.io (must already be authenticated)
# =============================================================================

set -euo pipefail

REGISTRY="fasodifit"
VERSION="v0.1.0"

# Parse arguments
DRY_RUN=false
if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=true
    echo ">>> DRY RUN mode - no images will be pushed"
fi

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

function log_info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
function log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
function log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

# All images to push
IMAGES=(
    "kaya"
    "armageddon"
    "xds-controller"
    "auth-ms"
    "poulets-api"
    "poulets-frontend"
    "poulets-bff"
)

echo "============================================================="
echo "  FASO DIGITALISATION - Podman Image Pusher"
echo "  Registry: ${REGISTRY}"
echo "  Version:  ${VERSION}"
echo "  Images:   ${#IMAGES[@]}"
echo "============================================================="
echo ""

# Verify podman login
if ! podman login --get-login docker.io &>/dev/null; then
    log_warn "Podman login status could not be verified."
    log_warn "Make sure you are logged in: podman login docker.io"
fi

FAILED=()
PUSHED=()

for image in "${IMAGES[@]}"; do
    # Check that the image exists locally
    if ! podman image inspect "${REGISTRY}/${image}:latest" &>/dev/null; then
        log_warn "Image ${REGISTRY}/${image}:latest not found locally, skipping."
        FAILED+=("${image}")
        continue
    fi

    log_info "Pushing ${REGISTRY}/${image}:latest and ${REGISTRY}/${image}:${VERSION} ..."

    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY RUN] Would push docker.io/${REGISTRY}/${image}:latest"
        log_info "[DRY RUN] Would push docker.io/${REGISTRY}/${image}:${VERSION}"
        PUSHED+=("${image}")
    else
        if podman push "${REGISTRY}/${image}:latest" "docker.io/${REGISTRY}/${image}:latest" && \
           podman push "${REGISTRY}/${image}:${VERSION}" "docker.io/${REGISTRY}/${image}:${VERSION}"; then
            log_info "Successfully pushed ${REGISTRY}/${image}"
            PUSHED+=("${image}")
        else
            log_error "Failed to push ${REGISTRY}/${image}"
            FAILED+=("${image}")
        fi
    fi
done

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "============================================================="
echo "  Push Summary"
echo "============================================================="

if [ ${#PUSHED[@]} -gt 0 ]; then
    log_info "Pushed (${#PUSHED[@]}): ${PUSHED[*]}"
fi

if [ ${#FAILED[@]} -gt 0 ]; then
    log_error "Failed/Skipped (${#FAILED[@]}): ${FAILED[*]}"
    exit 1
fi

log_info "All images pushed successfully!"
echo ""
echo "Images available at:"
for image in "${IMAGES[@]}"; do
    echo "  https://hub.docker.com/r/${REGISTRY}/${image}"
done
