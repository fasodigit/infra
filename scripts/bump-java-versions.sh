#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0
# FASO DIGITALISATION — Bump Maven project versions
# Called by semantic-release @semantic-release/exec prepareCmd
# Usage: NEXT_VERSION=x.y.z bash scripts/bump-java-versions.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INFRA_ROOT="$(dirname "$SCRIPT_DIR")"

if [[ -z "${NEXT_VERSION:-}" ]]; then
  echo "ERROR: NEXT_VERSION environment variable is not set" >&2
  exit 1
fi

echo "[bump-java-versions] Bumping Maven versions to ${NEXT_VERSION}"

# Maven version bump helper
bump_maven() {
  local module_path="$1"
  local module_name="$2"

  if [[ -f "${INFRA_ROOT}/${module_path}/pom.xml" ]]; then
    (
      cd "${INFRA_ROOT}/${module_path}"
      mvn --batch-mode --no-transfer-progress \
        versions:set \
        -DnewVersion="${NEXT_VERSION}" \
        -DgenerateBackupPoms=false
    )
    echo "  [ok] ${module_name} → ${NEXT_VERSION}"
  else
    echo "  [warn] ${module_path}/pom.xml not found, skipping" >&2
  fi
}

# ── auth-ms ───────────────────────────────────────────────────────────────────
bump_maven "auth-ms" "auth-ms"

# ── poulets backend ───────────────────────────────────────────────────────────
bump_maven "poulets-platform/backend" "poulets-platform/backend"

# ── notifier-ms (created when provisioned) ───────────────────────────────────
if [[ -d "${INFRA_ROOT}/notifier-ms" ]]; then
  bump_maven "notifier-ms" "notifier-ms"
fi

echo "[bump-java-versions] Done."
