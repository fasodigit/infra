#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION
#
# generate-rules.sh — Génère les recording + alert rules Prometheus depuis
# les définitions Sloth v1 (*.slo.yaml) présentes dans ce répertoire.
#
# Usage :
#   bash INFRA/observability/slo/generate-rules.sh
#
# Prérequis :
#   - sloth v0.12.0+ dans $PATH (https://github.com/slok/sloth/releases)
#
# Output :
#   INFRA/observability/prometheus/rules/slo/<service>.rules.yml
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
SLO_DIR="${SCRIPT_DIR}"
OUTPUT_DIR="${REPO_ROOT}/INFRA/observability/prometheus/rules/slo"

if ! command -v sloth >/dev/null 2>&1; then
  cat >&2 <<'EOF'
[ERROR] sloth n'est pas installé.
  Installation rapide :
    curl -sLo sloth.tar.gz https://github.com/slok/sloth/releases/download/v0.12.0/sloth-linux-amd64.tar.gz
    tar -xzf sloth.tar.gz && sudo mv sloth /usr/local/bin/sloth
EOF
  exit 1
fi

mkdir -p "${OUTPUT_DIR}"

echo "[sloth] Validation des définitions SLO dans ${SLO_DIR}"
sloth validate -i "${SLO_DIR}"

echo "[sloth] Génération des Prometheus rules → ${OUTPUT_DIR}"
shopt -s nullglob
for f in "${SLO_DIR}"/*.slo.yaml; do
  base="$(basename "$f" .slo.yaml)"
  out="${OUTPUT_DIR}/${base}.rules.yml"
  echo "  - ${base}.slo.yaml → slo/${base}.rules.yml"
  sloth generate -i "$f" -o "$out"
done

echo "[sloth] Terminé. Rules générées :"
ls -1 "${OUTPUT_DIR}"
