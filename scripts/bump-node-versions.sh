#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0
# FASO DIGITALISATION — Bump Node/npm package versions
# Called by semantic-release @semantic-release/exec prepareCmd
# Usage: NEXT_VERSION=x.y.z bash scripts/bump-node-versions.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INFRA_ROOT="$(dirname "$SCRIPT_DIR")"

if [[ -z "${NEXT_VERSION:-}" ]]; then
  echo "ERROR: NEXT_VERSION environment variable is not set" >&2
  exit 1
fi

echo "[bump-node-versions] Bumping Node package versions to ${NEXT_VERSION}"

# npm version bump helper (no git tag — semantic-release handles tagging)
bump_npm() {
  local package_json="$1"
  local label="$2"

  if [[ -f "$package_json" ]]; then
    # Use node to do a safe JSON in-place edit
    node -e "
      const fs = require('fs');
      const pkg = JSON.parse(fs.readFileSync('${package_json}', 'utf8'));
      pkg.version = '${NEXT_VERSION}';
      fs.writeFileSync('${package_json}', JSON.stringify(pkg, null, 2) + '\n');
    "
    echo "  [ok] ${label} → ${NEXT_VERSION}"
  else
    echo "  [warn] ${package_json} not found, skipping" >&2
  fi
}

# ── poulets BFF (Next.js) ─────────────────────────────────────────────────────
bump_npm "${INFRA_ROOT}/poulets-platform/bff/package.json" "poulets-platform/bff"

# ── poulets frontend (Angular) ────────────────────────────────────────────────
bump_npm "${INFRA_ROOT}/poulets-platform/frontend/package.json" "poulets-platform/frontend"

echo "[bump-node-versions] Done."
