#!/usr/bin/env bash
# =============================================================================
# Build all Docker images for FASO DIGITALISATION
# =============================================================================
# Usage: ./build-all.sh [--no-cache]
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
REGISTRY="fasodifit"
VERSION="v0.1.0"

# Parse arguments
NO_CACHE=""
if [[ "${1:-}" == "--no-cache" ]]; then
    NO_CACHE="--no-cache"
    echo ">>> Building with --no-cache"
fi

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

function log_info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
function log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
function log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

function build_image() {
    local name=$1
    local dockerfile=$2
    local context=${3:-$PROJECT_ROOT}

    log_info "Building ${REGISTRY}/${name}:${VERSION} ..."

    if docker build \
        ${NO_CACHE} \
        -t "${REGISTRY}/${name}:latest" \
        -t "${REGISTRY}/${name}:${VERSION}" \
        -f "${dockerfile}" \
        "${context}"; then
        log_info "Successfully built ${REGISTRY}/${name}"
    else
        log_error "Failed to build ${REGISTRY}/${name}"
        return 1
    fi
}

echo "============================================================="
echo "  FASO DIGITALISATION - Docker Image Builder"
echo "  Registry: ${REGISTRY}"
echo "  Version:  ${VERSION}"
echo "  Context:  ${PROJECT_ROOT}"
echo "============================================================="
echo ""

FAILED=()

# ---------------------------------------------------------------------------
# Infrastructure
# ---------------------------------------------------------------------------
log_info "=== Building Infrastructure Images ==="

build_image "kaya" \
    "${PROJECT_ROOT}/docker/images/Dockerfile.kaya" \
    "${PROJECT_ROOT}" || FAILED+=("kaya")

build_image "armageddon" \
    "${PROJECT_ROOT}/docker/images/Dockerfile.armageddon" \
    "${PROJECT_ROOT}" || FAILED+=("armageddon")

build_image "xds-controller" \
    "${PROJECT_ROOT}/docker/images/Dockerfile.xds-controller" \
    "${PROJECT_ROOT}" || FAILED+=("xds-controller")

# ---------------------------------------------------------------------------
# Backend Services
# ---------------------------------------------------------------------------
log_info "=== Building Backend Service Images ==="

build_image "auth-ms" \
    "${PROJECT_ROOT}/docker/images/Dockerfile.auth-ms" \
    "${PROJECT_ROOT}" || FAILED+=("auth-ms")

build_image "poulets-api" \
    "${PROJECT_ROOT}/docker/images/Dockerfile.poulets-api" \
    "${PROJECT_ROOT}" || FAILED+=("poulets-api")

# ---------------------------------------------------------------------------
# Frontend
# ---------------------------------------------------------------------------
log_info "=== Building Frontend Images ==="

build_image "poulets-frontend" \
    "${PROJECT_ROOT}/docker/images/Dockerfile.poulets-frontend" \
    "${PROJECT_ROOT}" || FAILED+=("poulets-frontend")

build_image "poulets-bff" \
    "${PROJECT_ROOT}/poulets-platform/bff/Dockerfile" \
    "${PROJECT_ROOT}" || FAILED+=("poulets-bff")

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "============================================================="
if [ ${#FAILED[@]} -eq 0 ]; then
    log_info "All images built successfully!"
else
    log_error "Failed to build: ${FAILED[*]}"
    echo ""
    echo "Built images:"
    docker images --filter "reference=${REGISTRY}/*" --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}\t{{.CreatedAt}}"
    exit 1
fi
echo "============================================================="
echo ""

docker images --filter "reference=${REGISTRY}/*" --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}\t{{.CreatedAt}}"
