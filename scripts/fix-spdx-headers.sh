#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# fix-spdx-headers.sh — Inject AGPL-3.0-or-later SPDX headers into ALL FASO
# source files (.java, .rs, .ts, .tsx, .yml, .yaml, .sql).
#
# Extends the original spdx-headers.sh with broader coverage including YAML,
# SQL, and additional service directories (notifier-ms, shared, BFF).
#
# Usage:
#   bash INFRA/scripts/fix-spdx-headers.sh           # apply fixes
#   bash INFRA/scripts/fix-spdx-headers.sh --dry-run  # report only

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
    "/.git/"
    "/.claude/"
  )
  for pat in "${skip_patterns[@]}"; do
    [[ "$path" == *"$pat"* ]] && return 0
  done
  # Skip *.d.ts
  [[ "$path" == *.d.ts ]] && return 0
  return 1
}

# ---------------------------------------------------------------------------
# Prepend helper — inserts $header before existing content.
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
# Check if file already has SPDX in first 5 lines
# ---------------------------------------------------------------------------
has_spdx() {
  head -n 5 "$1" | grep -q "SPDX-License-Identifier:" 2>/dev/null
}

# ===========================================================================
# 1. Rust: *.rs
# ===========================================================================
RS_HEADER="// SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
// SPDX-License-Identifier: AGPL-3.0-or-later"

RUST_DIRS=(
  "$INFRA_ROOT/kaya"
  "$INFRA_ROOT/armageddon"
  "$INFRA_ROOT/xds-controller"
)

for dir in "${RUST_DIRS[@]}"; do
  [[ -d "$dir" ]] || continue
  while IFS= read -r -d '' file; do
    should_skip "$file" && { SKIPPED=$((SKIPPED + 1)); continue; }
    has_spdx "$file" && { SKIPPED=$((SKIPPED + 1)); continue; }
    prepend_header "$file" "$RS_HEADER"
  done < <(find "$dir" -type f -name "*.rs" -print0 2>/dev/null)
done

# ===========================================================================
# 2. Java: *.java
# ===========================================================================
JAVA_HEADER="// SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
// SPDX-License-Identifier: AGPL-3.0-or-later"

JAVA_DIRS=(
  "$INFRA_ROOT/auth-ms/src"
  "$INFRA_ROOT/poulets-platform/backend/src"
  "$INFRA_ROOT/poulets-platform/backend/services"
  "$INFRA_ROOT/poulets-platform/bff/src"
  "$INFRA_ROOT/notifier-ms"
  "$INFRA_ROOT/shared"
)

for dir in "${JAVA_DIRS[@]}"; do
  [[ -d "$dir" ]] || continue
  while IFS= read -r -d '' file; do
    should_skip "$file" && { SKIPPED=$((SKIPPED + 1)); continue; }
    has_spdx "$file" && { SKIPPED=$((SKIPPED + 1)); continue; }
    prepend_header "$file" "$JAVA_HEADER"
  done < <(find "$dir" -type f -name "*.java" -print0 2>/dev/null)
done

# ===========================================================================
# 3. TypeScript / TSX
# ===========================================================================
TS_HEADER="// SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
// SPDX-License-Identifier: AGPL-3.0-or-later"

TS_DIRS=(
  "$INFRA_ROOT/poulets-platform/frontend/src"
  "$INFRA_ROOT/poulets-platform/bff/src"
)

for dir in "${TS_DIRS[@]}"; do
  [[ -d "$dir" ]] || continue
  while IFS= read -r -d '' file; do
    should_skip "$file" && { SKIPPED=$((SKIPPED + 1)); continue; }
    has_spdx "$file" && { SKIPPED=$((SKIPPED + 1)); continue; }
    prepend_header "$file" "$TS_HEADER"
  done < <(find "$dir" -type f \( -name "*.ts" -o -name "*.tsx" \) -print0 2>/dev/null)
done

# ===========================================================================
# 4. YAML / YML
# ===========================================================================
YAML_HEADER="# SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
# SPDX-License-Identifier: AGPL-3.0-or-later"

YAML_DIRS=(
  "$INFRA_ROOT/.github"
  "$INFRA_ROOT/observability"
  "$INFRA_ROOT/docker"
  "$INFRA_ROOT/ory"
  "$INFRA_ROOT/vault"
  "$INFRA_ROOT/growthbook"
  "$INFRA_ROOT/spire"
)

for dir in "${YAML_DIRS[@]}"; do
  [[ -d "$dir" ]] || continue
  while IFS= read -r -d '' file; do
    should_skip "$file" && { SKIPPED=$((SKIPPED + 1)); continue; }
    has_spdx "$file" && { SKIPPED=$((SKIPPED + 1)); continue; }
    prepend_header "$file" "$YAML_HEADER"
  done < <(find "$dir" -type f \( -name "*.yml" -o -name "*.yaml" \) -print0 2>/dev/null)
done

# ===========================================================================
# 5. SQL
# ===========================================================================
SQL_HEADER="-- SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
-- SPDX-License-Identifier: AGPL-3.0-or-later"

SQL_DIRS=(
  "$INFRA_ROOT/auth-ms"
  "$INFRA_ROOT/poulets-platform"
  "$INFRA_ROOT/notifier-ms"
  "$INFRA_ROOT/shared"
  "$INFRA_ROOT/docker"
  "$INFRA_ROOT/ory"
)

for dir in "${SQL_DIRS[@]}"; do
  [[ -d "$dir" ]] || continue
  while IFS= read -r -d '' file; do
    should_skip "$file" && { SKIPPED=$((SKIPPED + 1)); continue; }
    has_spdx "$file" && { SKIPPED=$((SKIPPED + 1)); continue; }
    prepend_header "$file" "$SQL_HEADER"
  done < <(find "$dir" -type f -name "*.sql" -print0 2>/dev/null)
done

# ===========================================================================
# Summary
# ===========================================================================
echo ""
echo "Done. Injected: $INJECTED | Already present / skipped: $SKIPPED"
if $DRY_RUN && [[ $INJECTED -gt 0 ]]; then
  echo "Re-run without --dry-run to apply."
fi
