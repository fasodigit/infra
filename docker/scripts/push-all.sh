#!/usr/bin/env bash
# =============================================================================
# Push all container images to Docker Hub for FASO DIGITALISATION
# =============================================================================
# Usage: ./push-all.sh [--dry-run]
# Prerequisite: podman login docker.io or docker login (must already be authenticated)
# =============================================================================

set -euo pipefail

# Auto-detect container runtime: prefer podman, fallback to docker
if command -v podman &>/dev/null; then
    CTR=podman
elif command -v docker &>/dev/null; then
    CTR=docker
else
    echo "ERROR: Neither podman nor docker found" >&2; exit 1
fi

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
echo "  FASO DIGITALISATION - Container Image Pusher (${CTR})"
echo "  Registry: ${REGISTRY}"
echo "  Version:  ${VERSION}"
echo "  Images:   ${#IMAGES[@]}"
echo "============================================================="
echo ""

# Verify login
if [ "$CTR" = "podman" ]; then
    podman login --get-login docker.io &>/dev/null || log_warn "Run: podman login docker.io"
else
    docker info 2>/dev/null | grep -q "Username" || log_warn "Run: docker login"
fi

FAILED=()
PUSHED=()

for image in "${IMAGES[@]}"; do
    # Check that the image exists locally
    if ! $CTR image inspect "${REGISTRY}/${image}:latest" &>/dev/null; then
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
        if $CTR push "${REGISTRY}/${image}:latest" "docker.io/${REGISTRY}/${image}:latest" && \
           $CTR push "${REGISTRY}/${image}:${VERSION}" "docker.io/${REGISTRY}/${image}:${VERSION}"; then
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
