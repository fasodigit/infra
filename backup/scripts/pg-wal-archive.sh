#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# pg-wal-archive.sh — WAL archiving helper for PostgreSQL
#
# Called by PostgreSQL archive_command:
#   archive_command = '/scripts/pg-wal-archive.sh %p %f'
#
# Features:
#   - Compress each WAL segment with zstd
#   - Encrypt with age
#   - Upload to S3-compatible storage
#   - Idempotent (safe to re-run — checks if segment already exists in S3)
#   - Structured JSON logging for Loki
#
# Environment variables (required):
#   BACKUP_ENCRYPTION_KEY_FILE — path to age public key file
#   S3_BUCKET           — S3 bucket name
#   S3_ENDPOINT         — S3-compatible endpoint URL
#
# Arguments:
#   $1 — WAL segment path (%p from archive_command)
#   $2 — WAL segment filename (%f from archive_command)

set -euo pipefail

# ---------------------------------------------------------------------------
# Arguments
# ---------------------------------------------------------------------------
WAL_PATH="${1:?Usage: $0 <wal_path> <wal_filename>}"
WAL_NAME="${2:?Usage: $0 <wal_path> <wal_filename>}"

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
BACKUP_ENCRYPTION_KEY_FILE="${BACKUP_ENCRYPTION_KEY_FILE:?BACKUP_ENCRYPTION_KEY_FILE is required}"
S3_BUCKET="${S3_BUCKET:?S3_BUCKET is required}"
S3_ENDPOINT="${S3_ENDPOINT:?S3_ENDPOINT is required}"
S3_PREFIX="${S3_PREFIX:-faso/postgres/wal}"
STAGING_DIR="${WAL_STAGING_DIR:-/tmp/faso-wal-staging}"
PUSHGATEWAY_URL="${PUSHGATEWAY_URL:-}"

# ---------------------------------------------------------------------------
# Structured JSON logging
# ---------------------------------------------------------------------------
log_json() {
  local level="$1" msg="$2"
  shift 2
  local extra=""
  while [[ $# -ge 2 ]]; do
    extra="${extra},\"$1\":\"$2\""
    shift 2
  done
  printf '{"ts":"%s","level":"%s","component":"pg-wal-archive","msg":"%s"%s}\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$level" "$msg" "$extra"
}

log_info()  { log_json "info"  "$@"; }
log_error() { log_json "error" "$@"; }

# ---------------------------------------------------------------------------
# Idempotency check — skip if segment already archived
# ---------------------------------------------------------------------------
S3_KEY="${S3_PREFIX}/${WAL_NAME}.zst.age"

if s3cmd info "s3://${S3_BUCKET}/${S3_KEY}" \
    --host="${S3_ENDPOINT}" \
    --host-bucket="${S3_BUCKET}.${S3_ENDPOINT}" \
    &>/dev/null; then
  log_info "WAL segment already archived — skipping" "segment" "$WAL_NAME"
  exit 0
fi

# ---------------------------------------------------------------------------
# Validate prerequisites
# ---------------------------------------------------------------------------
if [[ ! -f "$WAL_PATH" ]]; then
  log_error "WAL segment file not found" "path" "$WAL_PATH" "segment" "$WAL_NAME"
  exit 1
fi

if [[ ! -f "$BACKUP_ENCRYPTION_KEY_FILE" ]]; then
  log_error "Encryption key file not found" "path" "$BACKUP_ENCRYPTION_KEY_FILE"
  exit 1
fi

for cmd in zstd age s3cmd; do
  if ! command -v "$cmd" &>/dev/null; then
    log_error "Missing required command" "command" "$cmd"
    exit 1
  fi
done

mkdir -p "$STAGING_DIR"

# ---------------------------------------------------------------------------
# Compress + Encrypt + Upload
# ---------------------------------------------------------------------------
STAGED_FILE="${STAGING_DIR}/${WAL_NAME}.zst.age"

log_info "Archiving WAL segment" "segment" "$WAL_NAME" "source" "$WAL_PATH"

# Pipeline: compress -> encrypt -> staging file
zstd -3 --threads=0 -c "$WAL_PATH" \
  | age --encrypt --recipients-file "$BACKUP_ENCRYPTION_KEY_FILE" \
    -o "${STAGED_FILE}.tmp"

mv "${STAGED_FILE}.tmp" "$STAGED_FILE"

# Upload to S3
s3cmd put "$STAGED_FILE" \
  "s3://${S3_BUCKET}/${S3_KEY}" \
  --host="${S3_ENDPOINT}" \
  --host-bucket="${S3_BUCKET}.${S3_ENDPOINT}" \
  --no-mime-magic \
  --quiet

# Remove staged file after successful upload
rm -f "$STAGED_FILE"

WAL_SIZE="$(stat -c%s "$WAL_PATH" 2>/dev/null || stat -f%z "$WAL_PATH")"
log_info "WAL segment archived" \
  "segment" "$WAL_NAME" "size_bytes" "$WAL_SIZE" "s3_key" "$S3_KEY"

# ---------------------------------------------------------------------------
# Push metric (optional)
# ---------------------------------------------------------------------------
if [[ -n "$PUSHGATEWAY_URL" ]]; then
  cat <<METRIC_EOF | curl -fsS --max-time 10 --data-binary @- \
    "${PUSHGATEWAY_URL}/metrics/job/faso-pg-wal-archive" 2>/dev/null || true
# HELP faso_wal_archive_last_success_timestamp Last successful WAL archive timestamp
# TYPE faso_wal_archive_last_success_timestamp gauge
faso_wal_archive_last_success_timestamp $(date +%s)
# HELP faso_wal_archive_segment_bytes Size of last archived WAL segment
# TYPE faso_wal_archive_segment_bytes gauge
faso_wal_archive_segment_bytes ${WAL_SIZE}
METRIC_EOF
fi

exit 0
