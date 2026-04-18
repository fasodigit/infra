#!/usr/bin/env bash
#
# github-issues-import.sh
#
# Parse BACKLOG-EPICS.md and generate `gh issue create` commands for every
# EPIC-XX section found. By default, the script runs in DRY-RUN mode:
# it prints the commands to stdout without executing them.
#
# To actually create the GitHub issues, pass `--execute` AND answer `yes`
# to the confirmation prompt. The script exits with status 1 if either
# safeguard fails.
#
# Usage:
#   ./github-issues-import.sh                          # dry-run (default)
#   ./github-issues-import.sh --repo owner/name        # dry-run on specific repo
#   ./github-issues-import.sh --execute                # confirmed real run
#   ./github-issues-import.sh --backlog path/to.md     # alternate backlog file
#
# Environment variables:
#   GH_REPO                 # same as --repo
#   GITHUB_ISSUES_DRY_RUN=1 # force dry-run even if --execute is passed
#
# Exit codes:
#   0  success
#   1  safeguard failure or missing confirmation
#   2  bad arguments / missing files
#
# FASO DIGITALISATION — Plateforme Poulets

set -euo pipefail

#--- defaults ------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BACKLOG_DEFAULT="${SCRIPT_DIR}/../docs/BACKLOG-EPICS.md"

BACKLOG_FILE="${BACKLOG_DEFAULT}"
DRY_RUN=1
REPO="${GH_REPO:-}"
TMP_DIR=""

#--- argument parsing ----------------------------------------------------------

usage() {
  sed -n '2,25p' "$0" | sed -e 's/^# \{0,1\}//'
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --execute)     DRY_RUN=0; shift ;;
    --repo)        REPO="$2"; shift 2 ;;
    --backlog)     BACKLOG_FILE="$2"; shift 2 ;;
    -h|--help)     usage ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
done

# Force dry-run when env var is set, regardless of --execute.
if [[ "${GITHUB_ISSUES_DRY_RUN:-0}" == "1" ]]; then
  DRY_RUN=1
fi

#--- preflight checks ----------------------------------------------------------

if [[ ! -f "${BACKLOG_FILE}" ]]; then
  echo "ERROR: backlog file not found: ${BACKLOG_FILE}" >&2
  exit 2
fi

if [[ "${DRY_RUN}" -eq 0 ]]; then
  if ! command -v gh >/dev/null 2>&1; then
    echo "ERROR: gh CLI not installed. Install via: brew install gh / apt install gh" >&2
    exit 2
  fi

  if [[ -z "${REPO}" ]]; then
    echo "ERROR: --execute requires --repo owner/name or GH_REPO env var" >&2
    exit 2
  fi

  # Explicit confirmation to avoid accidental bulk issue creation.
  echo ""
  echo "=============================================================="
  echo " ABOUT TO CREATE GITHUB ISSUES IN REPO: ${REPO}"
  echo " Source: ${BACKLOG_FILE}"
  echo "=============================================================="
  echo ""
  read -r -p "Type 'yes' to proceed with creation: " CONFIRM
  if [[ "${CONFIRM}" != "yes" ]]; then
    echo "Aborted: confirmation not given (expected 'yes')." >&2
    exit 1
  fi
fi

#--- temp workspace ------------------------------------------------------------

TMP_DIR="$(mktemp -d -t faso-epics-XXXXXX)"
trap 'rm -rf "${TMP_DIR}"' EXIT

#--- parse backlog -------------------------------------------------------------

# We detect epic boundaries using the regex ^## EPIC-XX at column 0.
# Implementation relies on POSIX awk + grep/sed. For each epic we produce:
#   - <TMP_DIR>/<EPIC-XX>.body.md   : full epic markdown section
#   - <TMP_DIR>/<EPIC-XX>.meta      : key=value pairs (epic_id, title, labels, priority, effort)

awk -v tmp_dir="${TMP_DIR}" '
BEGIN { in_epic = 0 }

# New epic header
/^## EPIC-[0-9]+[[:space:]]*:/ {
  if (in_epic == 1) {
    close(body_path); close(meta_path)
  }
  # Strip "## " prefix
  header = $0
  sub(/^## /, "", header)
  # header now = "EPIC-XX : Title..."
  # epic_id = everything before first " :" or ": "
  eid = header
  sub(/[[:space:]]*:.*$/, "", eid)
  # title = everything after first ": "
  ttl = header
  sub(/^[^:]*:[[:space:]]*/, "", ttl)
  body_path = tmp_dir "/" eid ".body.md"
  meta_path = tmp_dir "/" eid ".meta"
  print "# " eid " : " ttl        > body_path
  print ""                         >> body_path
  print "epic_id=" eid             > meta_path
  print "title=" ttl               >> meta_path
  in_epic = 1
  next
}

# End of epic section when "Résumé priorités" or alike reached
in_epic == 1 && /^## Résumé/ {
  close(body_path); close(meta_path)
  in_epic = 0
  next
}

in_epic == 1 {
  print $0 >> body_path

  # Extract labels line: **Labels**: `a`, `b`, `c`
  if ($0 ~ /^\*\*Labels\*\*:/) {
    line = $0
    sub(/^\*\*Labels\*\*:[[:space:]]*/, "", line)
    gsub(/`/, "", line)
    print "labels=" line >> meta_path
  }

  # Extract priority P0/P1/P2
  if ($0 ~ /^\*\*Priority\*\*:/) {
    line = $0
    # find occurrence of P0, P1 or P2
    if (match(line, /P[012]/)) {
      p = substr(line, RSTART, RLENGTH)
      print "priority=" p >> meta_path
    }
  }

  # Extract effort token like "3w" / "2d" / "1m"
  if ($0 ~ /^\*\*Effort\*\*:/) {
    line = $0
    if (match(line, /[0-9]+[a-zA-Z]+/)) {
      e = substr(line, RSTART, RLENGTH)
      print "effort=" e >> meta_path
    }
  }
}

END {
  if (in_epic == 1) {
    close(body_path); close(meta_path)
  }
}
' "${BACKLOG_FILE}"

#--- generate / execute commands -----------------------------------------------

shopt -s nullglob
META_FILES=( "${TMP_DIR}"/EPIC-*.meta )

if [[ ${#META_FILES[@]} -eq 0 ]]; then
  echo "ERROR: no epics parsed from ${BACKLOG_FILE} (regex '^## EPIC-XX' matched 0 lines)" >&2
  exit 2
fi

echo ""
echo "Parsed ${#META_FILES[@]} epics from ${BACKLOG_FILE}"
echo "Mode: $([[ "${DRY_RUN}" -eq 1 ]] && echo "DRY-RUN (no gh calls)" || echo "EXECUTE → ${REPO}")"
echo ""

COUNT=0
for META in "${META_FILES[@]}"; do
  BODY_FILE="${META%.meta}.body.md"

  # Load meta into shell variables
  # shellcheck disable=SC1090
  epic_id=""; title=""; labels=""; priority=""; effort=""
  while IFS='=' read -r k v; do
    case "$k" in
      epic_id)  epic_id="$v" ;;
      title)    title="$v" ;;
      labels)   labels="$v" ;;
      priority) priority="$v" ;;
      effort)   effort="$v" ;;
    esac
  done < "${META}"

  # Sanitize labels: comma + space -> comma only, strip whitespace
  labels_clean="$(echo "${labels}" | tr -d ' ')"
  # Fallback: if no labels parsed, synthesize from priority
  if [[ -z "${labels_clean}" && -n "${priority}" ]]; then
    labels_clean="epic,priority-${priority}"
  fi

  issue_title="${epic_id} : ${title}"
  # Prefix effort in label set (e.g. effort-3w) if present
  if [[ -n "${effort}" ]]; then
    labels_clean="${labels_clean},effort-${effort}"
  fi

  COUNT=$((COUNT + 1))

  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "# ---- [${COUNT}] ${epic_id} ----"
    echo "gh issue create \\"
    [[ -n "${REPO}" ]] && echo "  --repo \"${REPO}\" \\"
    echo "  --title \"${issue_title}\" \\"
    echo "  --body-file \"${BODY_FILE}\" \\"
    echo "  --label \"${labels_clean}\""
    echo ""
  else
    echo "[${COUNT}/${#META_FILES[@]}] Creating issue: ${issue_title}"
    gh issue create \
      --repo "${REPO}" \
      --title "${issue_title}" \
      --body-file "${BODY_FILE}" \
      --label "${labels_clean}"
    echo ""
  fi
done

echo ""
echo "Done. ${COUNT} epics processed (mode: $([[ "${DRY_RUN}" -eq 1 ]] && echo "DRY-RUN" || echo "EXECUTE"))."

if [[ "${DRY_RUN}" -eq 1 ]]; then
  echo "To actually create the issues, re-run with: --execute --repo owner/name"
fi

exit 0
