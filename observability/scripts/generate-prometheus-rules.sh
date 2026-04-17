#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Generate Prometheus recording + alert rules from Sloth SLO definitions.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SLO_DIR="$SCRIPT_DIR/../slo"
OUT_DIR="$SCRIPT_DIR/../prometheus/rules"

mkdir -p "$OUT_DIR"

if ! command -v sloth >/dev/null 2>&1; then
  echo "ERROR: sloth CLI not found. Install: https://sloth.dev/introduction/install/" >&2
  exit 1
fi

for slo_file in "$SLO_DIR"/*.slo.yaml; do
  service=$(basename "$slo_file" .slo.yaml)
  out_file="$OUT_DIR/${service}-slo-rules.yaml"
  echo "→ sloth generate $slo_file → $out_file"
  sloth generate -i "$slo_file" -o "$out_file"
done

echo "✓ Generated $(ls "$OUT_DIR" | wc -l) rule files in $OUT_DIR"
