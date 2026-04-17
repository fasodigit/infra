#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION
#
# spdx-check.sh — CI mode: exit 1 if any FASO source file is missing its SPDX header.
# Usage: bash INFRA/scripts/spdx-check.sh

set -euo pipefail

INFRA_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MISSING=()

# ---------------------------------------------------------------------------
# Skip-path predicate
# ---------------------------------------------------------------------------
should_skip() {
  local path="$1"
  local skip_patterns=(
    "node_modules"
    "/target/"
    "/dist/"
    "/build/"
    "/generated/"
    "/gen/"
    "/vendor/"
    "/third_party/"
  )
  for pat in "${skip_patterns[@]}"; do
    [[ "$path" == *"$pat"* ]] && return 0
  done
  [[ "$path" == *.d.ts ]] && return 0
  return 1
}

# ---------------------------------------------------------------------------
# Check a single file for SPDX presence in first 5 lines
# ---------------------------------------------------------------------------
check_file() {
  local file="$1"
  should_skip "$file" && return
  if ! head -n 5 "$file" | grep -q "SPDX-License-Identifier:"; then
    MISSING+=("$file")
  fi
}

# ---------------------------------------------------------------------------
# Rust
# ---------------------------------------------------------------------------
for dir in "$INFRA_ROOT/kaya/src" "$INFRA_ROOT/armageddon"; do
  while IFS= read -r -d '' file; do
    check_file "$file"
  done < <(find "$dir" -type f -name "*.rs" -print0 2>/dev/null)
done

# ---------------------------------------------------------------------------
# Java
# ---------------------------------------------------------------------------
for dir in "$INFRA_ROOT/auth-ms/src" "$INFRA_ROOT/poulets-platform/backend/src"; do
  while IFS= read -r -d '' file; do
    check_file "$file"
  done < <(find "$dir" -type f -name "*.java" -print0 2>/dev/null)
done

# ---------------------------------------------------------------------------
# TypeScript / TSX
# ---------------------------------------------------------------------------
while IFS= read -r -d '' file; do
  check_file "$file"
done < <(find "$INFRA_ROOT/poulets-platform/frontend/src" -type f \
  \( -name "*.ts" -o -name "*.tsx" \) -print0 2>/dev/null)

# ---------------------------------------------------------------------------
# Protobuf (FASO-owned)
# ---------------------------------------------------------------------------
for dir in \
  "$INFRA_ROOT/poulets-platform/backend/src/main/proto" \
  "$INFRA_ROOT/kaya"; do
  while IFS= read -r -d '' file; do
    check_file "$file"
  done < <(find "$dir" -type f -name "*.proto" -print0 2>/dev/null)
done

# ---------------------------------------------------------------------------
# Report
# ---------------------------------------------------------------------------
if [[ ${#MISSING[@]} -eq 0 ]]; then
  echo "SPDX check passed: all source files have AGPL-3.0-or-later headers."
  exit 0
else
  echo "SPDX check FAILED: ${#MISSING[@]} file(s) missing SPDX-License-Identifier header:"
  for f in "${MISSING[@]}"; do
    echo "  - $f"
  done
  echo ""
  echo "Run: bash INFRA/scripts/spdx-headers.sh"
  exit 1
fi
