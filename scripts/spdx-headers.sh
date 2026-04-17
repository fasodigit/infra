#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION
#
# spdx-headers.sh — Inject AGPL-3.0-or-later SPDX headers into FASO source files.
# Idempotent: re-running does NOT duplicate headers.
# Usage: bash INFRA/scripts/spdx-headers.sh [--dry-run]

set -euo pipefail

INFRA_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DRY_RUN=false
INJECTED=0
SKIPPED=0

if [[ "${1:-}" == "--dry-run" ]]; then
  DRY_RUN=true
  echo "[DRY-RUN] No files will be modified."
fi

# ---------------------------------------------------------------------------
# Skip-path predicate: returns 0 (true) if path should be skipped
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
    if [[ "$path" == *"$pat"* ]]; then
      return 0
    fi
  done
  # Skip *.d.ts
  if [[ "$path" == *.d.ts ]]; then
    return 0
  fi
  return 1
}

# ---------------------------------------------------------------------------
# Prepend helper — inserts $header before existing content.
# Idempotency check: caller must verify header is absent before calling.
# ---------------------------------------------------------------------------
prepend_header() {
  local file="$1"
  local header="$2"
  if $DRY_RUN; then
    echo "[DRY-RUN] Would inject into: $file"
    INJECTED=$((INJECTED + 1))
    return
  fi
  local tmp
  tmp="$(mktemp)"
  printf '%s\n' "$header" > "$tmp"
  cat "$file" >> "$tmp"
  mv "$tmp" "$file"
  echo "[INJECTED] $file"
  INJECTED=$((INJECTED + 1))
}

# ---------------------------------------------------------------------------
# Rust: *.rs under INFRA/kaya/src/** and INFRA/armageddon/src/**
# Header uses // style (two lines)
# ---------------------------------------------------------------------------
RS_HEADER="// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION"

RUST_DIRS=(
  "$INFRA_ROOT/kaya/src"
  "$INFRA_ROOT/armageddon"
)

while IFS= read -r -d '' file; do
  should_skip "$file" && { SKIPPED=$((SKIPPED + 1)); continue; }
  first_line="$(head -n 1 "$file")"
  if [[ "$first_line" == "// SPDX-License-Identifier:"* ]] || \
     [[ "$first_line" == "//! SPDX-License-Identifier:"* ]]; then
    SKIPPED=$((SKIPPED + 1))
    continue
  fi
  prepend_header "$file" "$RS_HEADER"
done < <(find "${RUST_DIRS[@]}" -type f -name "*.rs" -print0 2>/dev/null)

# ---------------------------------------------------------------------------
# Java: *.java under auth-ms and poulets-platform/backend
# Header uses /* */ block style (before package declaration)
# ---------------------------------------------------------------------------
JAVA_HEADER="/*
 * SPDX-License-Identifier: AGPL-3.0-or-later
 * Copyright (C) 2026 FASO DIGITALISATION
 */"

JAVA_DIRS=(
  "$INFRA_ROOT/auth-ms/src"
  "$INFRA_ROOT/poulets-platform/backend/src"
)

while IFS= read -r -d '' file; do
  should_skip "$file" && { SKIPPED=$((SKIPPED + 1)); continue; }
  first_line="$(head -n 1 "$file")"
  if [[ "$first_line" == "/*"* ]] && grep -qm1 "SPDX-License-Identifier:" "$file"; then
    SKIPPED=$((SKIPPED + 1))
    continue
  fi
  # Also skip if the file already has SPDX anywhere in the first 5 lines
  if head -n 5 "$file" | grep -q "SPDX-License-Identifier:"; then
    SKIPPED=$((SKIPPED + 1))
    continue
  fi
  prepend_header "$file" "$JAVA_HEADER"
done < <(find "${JAVA_DIRS[@]}" -type f -name "*.java" -print0 2>/dev/null)

# ---------------------------------------------------------------------------
# TypeScript/TSX: Angular/BFF — skip node_modules, dist, generated
# ---------------------------------------------------------------------------
TS_HEADER="// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION"

TS_DIRS=(
  "$INFRA_ROOT/poulets-platform/frontend/src"
)

while IFS= read -r -d '' file; do
  should_skip "$file" && { SKIPPED=$((SKIPPED + 1)); continue; }
  first_line="$(head -n 1 "$file")"
  if [[ "$first_line" == "// SPDX-License-Identifier:"* ]]; then
    SKIPPED=$((SKIPPED + 1))
    continue
  fi
  prepend_header "$file" "$TS_HEADER"
done < <(find "${TS_DIRS[@]}" -type f \( -name "*.ts" -o -name "*.tsx" \) -print0 2>/dev/null)

# ---------------------------------------------------------------------------
# Protobuf: *.proto — FASO-owned protos only (not xds-controller/proto which
# contains third-party envoy/google protos)
# ---------------------------------------------------------------------------
PROTO_HEADER="// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION"

PROTO_DIRS=(
  "$INFRA_ROOT/poulets-platform/backend/src/main/proto"
  "$INFRA_ROOT/kaya"
)

while IFS= read -r -d '' file; do
  should_skip "$file" && { SKIPPED=$((SKIPPED + 1)); continue; }
  first_line="$(head -n 1 "$file")"
  if [[ "$first_line" == "// SPDX-License-Identifier:"* ]]; then
    SKIPPED=$((SKIPPED + 1))
    continue
  fi
  prepend_header "$file" "$PROTO_HEADER"
done < <(find "${PROTO_DIRS[@]}" -type f -name "*.proto" -print0 2>/dev/null)

# ---------------------------------------------------------------------------
echo ""
echo "Done. Injected: $INJECTED | Already present / skipped: $SKIPPED"
