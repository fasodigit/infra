#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Build Coraza-proxy-wasm — WAF module for ARMAGEDDON wasm_adapter.
#
# Output: coraza-waf.wasm (~8-12MB)
# Requirements: tinygo >= 0.32, go >= 1.22
#
# Usage:
#   bash build.sh

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKDIR=$(mktemp -d)
trap "rm -rf $WORKDIR" EXIT

CORAZA_PROXY_WASM_TAG="${CORAZA_PROXY_WASM_TAG:-0.6.0}"

echo "[coraza] cloning corazawaf/coraza-proxy-wasm @ ${CORAZA_PROXY_WASM_TAG}"
git clone --depth 1 --branch "${CORAZA_PROXY_WASM_TAG}" \
  https://github.com/corazawaf/coraza-proxy-wasm.git "$WORKDIR/coraza-proxy-wasm"
cd "$WORKDIR/coraza-proxy-wasm"

echo "[coraza] tinygo build → coraza-waf.wasm"
tinygo build \
  -target=wasi \
  -gc=custom -tags='custommalloc no_fs_access proxywasm_timing' \
  -o "${SCRIPT_DIR}/coraza-waf.wasm" \
  ./...

echo "[coraza] verifying output"
ls -la "${SCRIPT_DIR}/coraza-waf.wasm"

echo "[coraza] downloading OWASP CRS v4.10.0 starter pack"
CRS_URL="https://github.com/coreruleset/coreruleset/archive/v4.10.0.tar.gz"
curl -L "$CRS_URL" -o "$WORKDIR/crs.tar.gz"
mkdir -p "${SCRIPT_DIR}/crs"
tar xz -C "${SCRIPT_DIR}/crs" --strip-components=1 -f "$WORKDIR/crs.tar.gz"
cp "${SCRIPT_DIR}/crs/crs-setup.conf.example" "${SCRIPT_DIR}/crs/crs-setup.conf" 2>/dev/null || true

echo "[coraza] OK — coraza-waf.wasm and CRS rules in ${SCRIPT_DIR}"
