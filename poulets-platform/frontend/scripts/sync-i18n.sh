#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# © 2026 FASO DIGITALISATION — Burkina Faso
#
# sync-i18n.sh — Export fr.json to TSV for translators, reimport their work.
#
# Usage:
#   Export:  ./scripts/sync-i18n.sh export <lang>   # e.g. mos, dyu, ful
#   Import:  ./scripts/sync-i18n.sh import <lang> <tsv_file>
#   Audit:   ./scripts/sync-i18n.sh audit <lang>
#
# TSV format (for translators):
#   KEY\tFR_VALUE\t<LANG>_VALUE
#   Lines starting with # are comments and are ignored.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
I18N_DIR="$SCRIPT_DIR/../src/assets/i18n"
FR_JSON="$I18N_DIR/fr.json"

SUPPORTED_LANGS=(mos dyu ful en)

# ────────────────────────────────────────────────────────────────────────────
# Helpers
# ────────────────────────────────────────────────────────────────────────────

check_dep() {
  if ! command -v "$1" &>/dev/null; then
    echo "ERROR: '$1' is required but not installed." >&2
    exit 1
  fi
}

check_dep node
check_dep jq

# Flatten nested JSON to dot-notation keys
# e.g. { "COMMON": { "LOGIN": "Connexion" } }  →  COMMON.LOGIN\tConnexion
flatten_json() {
  local file="$1"
  node -e "
const fs = require('fs');
const obj = JSON.parse(fs.readFileSync('$file', 'utf8'));
function flatten(o, prefix) {
  const out = {};
  for (const [k, v] of Object.entries(o)) {
    const key = prefix ? prefix + '.' + k : k;
    if (v && typeof v === 'object' && !Array.isArray(v)) {
      Object.assign(out, flatten(v, key));
    } else {
      out[key] = String(v);
    }
  }
  return out;
}
const flat = flatten(obj, '');
for (const [k, v] of Object.entries(flat)) {
  if (k.startsWith('_meta.')) continue;
  process.stdout.write(k + '\t' + v + '\n');
}
"
}

# Unflatten TSV back to nested JSON (only updates existing lang file values)
unflatten_tsv_to_json() {
  local lang="$1"
  local tsv="$2"
  local target="$I18N_DIR/${lang}.json"
  node -e "
const fs = require('fs');
const tsv = fs.readFileSync('$tsv', 'utf8');
let target;
try { target = JSON.parse(fs.readFileSync('$target', 'utf8')); } catch { target = {}; }

function set_nested(obj, keyPath, val) {
  const parts = keyPath.split('.');
  let cur = obj;
  for (let i = 0; i < parts.length - 1; i++) {
    if (!cur[parts[i]] || typeof cur[parts[i]] !== 'object') cur[parts[i]] = {};
    cur = cur[parts[i]];
  }
  cur[parts[parts.length - 1]] = val;
}

for (const line of tsv.split('\n')) {
  if (!line.trim() || line.startsWith('#')) continue;
  const cols = line.split('\t');
  if (cols.length < 3) continue;
  const key = cols[0].trim();
  const translated = cols[2].trim();
  if (translated && !translated.startsWith('[${lang^^}]') && !translated.startsWith('[MOS]') && !translated.startsWith('[DYU]') && !translated.startsWith('[FUL]')) {
    set_nested(target, key, translated);
  }
}

fs.writeFileSync('$target', JSON.stringify(target, null, 2) + '\n', 'utf8');
console.log('Imported translations into $target');
"
}

# ────────────────────────────────────────────────────────────────────────────
# Commands
# ────────────────────────────────────────────────────────────────────────────

cmd_export() {
  local lang="${1:-}"
  if [[ -z "$lang" ]]; then
    echo "Usage: $0 export <lang>" >&2; exit 1
  fi
  local target="$I18N_DIR/${lang}.json"
  local out="$SCRIPT_DIR/../i18n-export-${lang}.tsv"

  echo "# Poulets BF — i18n translation sheet"          > "$out"
  echo "# Language: ${lang}"                            >> "$out"
  echo "# Date: $(date -u +%Y-%m-%dT%H:%MZ)"           >> "$out"
  echo "# Instructions:"                                >> "$out"
  echo "#   - Fill column 3 with the translation"       >> "$out"
  echo "#   - Do NOT modify columns 1 or 2"             >> "$out"
  echo "#   - Remove [${lang^^}] prefix when translated" >> "$out"
  echo "#   - Return file to: fasodigitalisation@gmail.com" >> "$out"
  echo "#"                                              >> "$out"
  echo -e "KEY\tFR\t${lang^^}"                          >> "$out"

  local fr_flat
  fr_flat="$(flatten_json "$FR_JSON")"

  local lang_flat=""
  if [[ -f "$target" ]]; then
    lang_flat="$(flatten_json "$target")"
  fi

  while IFS=$'\t' read -r key fr_val; do
    # Skip meta keys
    [[ "$key" == _meta.* ]] && continue
    local lang_val=""
    lang_val="$(echo "$lang_flat" | awk -F'\t' -v k="$key" '$1 == k { print $2 }' | head -1)"
    echo -e "${key}\t${fr_val}\t${lang_val}" >> "$out"
  done <<< "$fr_flat"

  echo "Exported to: $out"
  echo "Send this file to your translators for language: $lang"
}

cmd_import() {
  local lang="${1:-}"
  local tsv="${2:-}"
  if [[ -z "$lang" || -z "$tsv" ]]; then
    echo "Usage: $0 import <lang> <tsv_file>" >&2; exit 1
  fi
  if [[ ! -f "$tsv" ]]; then
    echo "ERROR: TSV file not found: $tsv" >&2; exit 1
  fi
  unflatten_tsv_to_json "$lang" "$tsv"
}

cmd_audit() {
  local lang="${1:-}"
  if [[ -z "$lang" ]]; then
    # Audit all
    for l in "${SUPPORTED_LANGS[@]}"; do
      cmd_audit "$l"
    done
    return
  fi

  local target="$I18N_DIR/${lang}.json"
  if [[ ! -f "$target" ]]; then
    echo "[$lang] MISSING file: $target"
    return
  fi

  local fr_keys lang_keys total covered pct
  fr_keys=$(flatten_json "$FR_JSON" | grep -v '^_meta\.' | awk -F'\t' '{print $1}' | sort)
  lang_keys=$(flatten_json "$target" | grep -v '^_meta\.' | awk -F'\t' '$2 !~ /^\[/ {print $1}' | sort)

  total=$(echo "$fr_keys" | wc -l)
  covered=$(comm -12 <(echo "$fr_keys") <(echo "$lang_keys") | wc -l)
  pct=$(( covered * 100 / total ))

  local status="OK"
  [[ $pct -lt 80 ]] && status="WARN (< 80%)"

  printf "[%s] %d/%d keys translated (%d%%) — %s\n" "$lang" "$covered" "$total" "$pct" "$status"

  if [[ "${VERBOSE:-0}" == "1" ]]; then
    echo "  Missing keys:"
    comm -23 <(echo "$fr_keys") <(echo "$lang_keys") | sed 's/^/    /'
  fi
}

# ────────────────────────────────────────────────────────────────────────────
# Main
# ────────────────────────────────────────────────────────────────────────────

COMMAND="${1:-help}"
shift || true

case "$COMMAND" in
  export) cmd_export "$@" ;;
  import) cmd_import "$@" ;;
  audit)  cmd_audit  "$@" ;;
  help|--help|-h)
    echo "Usage: $0 <command> [args]"
    echo ""
    echo "Commands:"
    echo "  export <lang>            Export fr.json + lang.json to TSV for translators"
    echo "  import <lang> <tsv>      Reimport translated TSV into lang.json"
    echo "  audit [lang]             Check translation coverage (all langs if omitted)"
    echo ""
    echo "Supported langs: ${SUPPORTED_LANGS[*]}"
    echo ""
    echo "Example workflow:"
    echo "  ./scripts/sync-i18n.sh export mos"
    echo "  # → share i18n-export-mos.tsv with Mooré translators"
    echo "  ./scripts/sync-i18n.sh import mos i18n-export-mos-translated.tsv"
    echo "  ./scripts/sync-i18n.sh audit mos"
    ;;
  *)
    echo "Unknown command: $COMMAND. Run '$0 help' for usage." >&2
    exit 1
    ;;
esac
