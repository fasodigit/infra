#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# validate-ports.sh — cross-check live listening sockets against port-policy.yaml
#
# Exit codes:
#   0  all live ports are allocated in policy (or in a reserved range that permits them)
#   1  violation detected (unknown port listening)
#   2  policy file missing / malformed
#
# Usage:
#   bash INFRA/scripts/validate-ports.sh          # report + exit 0/1
#   bash INFRA/scripts/validate-ports.sh --json   # machine-readable
#   bash INFRA/scripts/validate-ports.sh --fix    # print suggestions
set -euo pipefail

POLICY="${FASO_PORT_POLICY:-$(dirname "$0")/../port-policy.yaml}"
[[ -f "$POLICY" ]] || { echo "ERROR: policy $POLICY not found" >&2; exit 2; }

MODE="${1:-report}"

# Extract all declared ports from policy (YAML -> jq via python3 PyYAML-free inline parser)
DECLARED=$(python3 - "$POLICY" <<'PY'
import sys, re
path = sys.argv[1]
txt = open(path).read()
ports = set()
# Match 'port: <digits>' anywhere
for m in re.finditer(r'^\s*-?\s*\{[^}]*\bport:\s*(\d+)', txt, re.MULTILINE):
    ports.add(m.group(1))
for m in re.finditer(r'^\s*port:\s*(\d+)', txt, re.MULTILINE):
    ports.add(m.group(1))
print('\n'.join(sorted(ports, key=int)))
PY
)

# List actually-listening ports (TCP IPv4/IPv6)
LIVE=$(ss -tlnH 2>/dev/null | awk '{print $4}' | sed -E 's/.*:([0-9]+)$/\1/' | sort -un)

unknown=()
for p in $LIVE; do
  # Ignore ephemeral (>32768) and obvious OS ports
  [[ "$p" -ge 32768 || "$p" -le 22 ]] && continue
  if ! grep -qE "^${p}$" <<< "$DECLARED"; then
    unknown+=("$p")
  fi
done

if [[ "$MODE" == "--json" ]]; then
  printf '{"declared":[%s],"live":[%s],"unknown":[%s]}\n' \
    "$(paste -sd, <<<"$DECLARED")" \
    "$(paste -sd, <<<"$LIVE")" \
    "$(IFS=,; echo "${unknown[*]:-}")"
  [[ ${#unknown[@]} -eq 0 ]]
  exit $?
fi

echo "=== FASO port policy validation ==="
echo "Policy:   $POLICY"
echo "Declared: $(wc -l <<<"$DECLARED") ports"
echo "Live:     $(wc -l <<<"$LIVE") listening TCP ports"
echo

if [[ ${#unknown[@]} -gt 0 ]]; then
  echo "❌ VIOLATIONS — ports listening but NOT in policy:"
  for p in "${unknown[@]}"; do
    owner=$(ss -tlnp 2>/dev/null | awk -v pp="$p" '$4 ~ ":"pp"$" {print $6}' | head -1 | tr -d '()\"' || echo unknown)
    printf "  :%-6s  owner: %s\n" "$p" "$owner"
  done
  echo
  if [[ "$MODE" == "--fix" ]]; then
    echo "Suggested actions:"
    echo "  1. If the port is intentional: add to INFRA/port-policy.yaml in the appropriate range."
    echo "  2. If the port is a random JMX/ephemeral: pin via -Dcom.sun.management.jmxremote.port=<range>"
    echo "  3. If the service is obsolete: kill it."
  fi
  exit 1
fi

echo "✅ All live ports are declared in policy."
exit 0
