#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0
# FASO DIGITALISATION — Bump Cargo workspace versions
# Called by semantic-release @semantic-release/exec prepareCmd
# Usage: NEXT_VERSION=x.y.z bash scripts/bump-versions.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INFRA_ROOT="$(dirname "$SCRIPT_DIR")"

if [[ -z "${NEXT_VERSION:-}" ]]; then
  echo "ERROR: NEXT_VERSION environment variable is not set" >&2
  exit 1
fi

echo "[bump-versions] Bumping Rust workspace versions to ${NEXT_VERSION}"

# ── kaya workspace ────────────────────────────────────────────────────────────
KAYA_TOML="${INFRA_ROOT}/kaya/Cargo.toml"
if [[ -f "$KAYA_TOML" ]]; then
  sed -i "s/^\(version\s*=\s*\"\)[^\"]*\"/\1${NEXT_VERSION}\"/" "$KAYA_TOML"
  echo "  [ok] kaya/Cargo.toml → ${NEXT_VERSION}"
else
  echo "  [warn] kaya/Cargo.toml not found, skipping" >&2
fi

# ── armageddon workspace ──────────────────────────────────────────────────────
ARMAGEDDON_TOML="${INFRA_ROOT}/armageddon/Cargo.toml"
if [[ -f "$ARMAGEDDON_TOML" ]]; then
  sed -i "s/^\(version\s*=\s*\"\)[^\"]*\"/\1${NEXT_VERSION}\"/" "$ARMAGEDDON_TOML"
  echo "  [ok] armageddon/Cargo.toml → ${NEXT_VERSION}"
else
  echo "  [warn] armageddon/Cargo.toml not found, skipping" >&2
fi

echo "[bump-versions] Done."
