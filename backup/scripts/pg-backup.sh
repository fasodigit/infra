#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# pg-backup.sh — Automated PostgreSQL backup for FASO DIGITALISATION
#
# Features:
#   - pg_basebackup for full backups (weekly schedule)
#   - zstd compression (level 3)
#   - age encryption (key path from BACKUP_ENCRYPTION_KEY_FILE)
#   - Upload to S3-compatible storage (OVH Object Storage)
#   - Retention: 4 weekly + 12 monthly + 1 yearly
#   - Prometheus pushgateway metric on success/failure
#   - Structured JSON logging for Loki
#
# Environment variables (required):
#   PGHOST              — PostgreSQL host (default: postgres)
#   PGPORT              — PostgreSQL port (default: 5432)
#   PGUSER              — PostgreSQL user (default: faso)
#   PGPASSWORD_FILE     — path to password file
#   BACKUP_ENCRYPTION_KEY_FILE — path to age public key file
#   S3_BUCKET           — S3 bucket name
#   S3_ENDPOINT         — S3-compatible endpoint URL (OVH)
#   PUSHGATEWAY_URL     — Prometheus pushgateway URL (optional)
#
# Usage:
#   ./pg-backup.sh [full|wal-only]

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
PGHOST="${PGHOST:-postgres}"
PGPORT="${PGPORT:-5432}"
PGUSER="${PGUSER:-faso}"
PGDATABASE="${PGDATABASE:-faso_main}"
BACKUP_TYPE="${1:-full}"
BACKUP_DIR="${BACKUP_DIR:-/tmp/faso-backups}"
BACKUP_ENCRYPTION_KEY_FILE="${BACKUP_ENCRYPTION_KEY_FILE:?BACKUP_ENCRYPTION_KEY_FILE is required}"
S3_BUCKET="${S3_BUCKET:?S3_BUCKET is required}"
S3_ENDPOINT="${S3_ENDPOINT:?S3_ENDPOINT is required}"
S3_PREFIX="${S3_PREFIX:-faso/postgres}"
PUSHGATEWAY_URL="${PUSHGATEWAY_URL:-}"
ZSTD_LEVEL="${ZSTD_LEVEL:-3}"

TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
DATE_TAG="$(date -u +%Y%m%d)"
WEEK_TAG="$(date -u +%Yw%V)"
MONTH_TAG="$(date -u +%Y%m)"
YEAR_TAG="$(date -u +%Y)"

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
  printf '{"ts":"%s","level":"%s","component":"pg-backup","msg":"%s"%s}\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$level" "$msg" "$extra"
}

log_info()  { log_json "info"  "$@"; }
log_error() { log_json "error" "$@"; }
log_warn()  { log_json "warn"  "$@"; }

# ---------------------------------------------------------------------------
# Metrics push (Prometheus Pushgateway)
# ---------------------------------------------------------------------------
push_metric() {
  local metric_name="$1" value="$2" job="${3:-faso-pg-backup}"
  if [[ -n "$PUSHGATEWAY_URL" ]]; then
    cat <<METRIC_EOF | curl -fsS --max-time 10 --data-binary @- \
      "${PUSHGATEWAY_URL}/metrics/job/${job}" 2>/dev/null || true
# HELP ${metric_name} FASO backup metric
# TYPE ${metric_name} gauge
${metric_name} ${value}
METRIC_EOF
  fi
}

# ---------------------------------------------------------------------------
# Cleanup handler
# ---------------------------------------------------------------------------
BACKUP_FILE=""
cleanup() {
  local exit_code=$?
  if [[ $exit_code -ne 0 ]]; then
    log_error "Backup FAILED" "exit_code" "$exit_code" "type" "$BACKUP_TYPE"
    push_metric "faso_backup_last_failure_timestamp" "$(date +%s)"
    push_metric "faso_backup_last_status" "0"
  fi
  # Remove temp files
  rm -f "${BACKUP_DIR}/"*.tmp 2>/dev/null || true
  exit "$exit_code"
}
trap cleanup EXIT

# ---------------------------------------------------------------------------
# Load password
# ---------------------------------------------------------------------------
if [[ -n "${PGPASSWORD_FILE:-}" && -f "$PGPASSWORD_FILE" ]]; then
  PGPASSWORD="$(tr -d '\n\r' < "$PGPASSWORD_FILE")"
  export PGPASSWORD
fi

# ---------------------------------------------------------------------------
# Validate prerequisites
# ---------------------------------------------------------------------------
for cmd in pg_basebackup zstd age s3cmd; do
  if ! command -v "$cmd" &>/dev/null; then
    log_error "Missing required command" "command" "$cmd"
    exit 1
  fi
done

if [[ ! -f "$BACKUP_ENCRYPTION_KEY_FILE" ]]; then
  log_error "Encryption key file not found" "path" "$BACKUP_ENCRYPTION_KEY_FILE"
  exit 1
fi

mkdir -p "$BACKUP_DIR"

# ---------------------------------------------------------------------------
# Full backup via pg_basebackup
# ---------------------------------------------------------------------------
if [[ "$BACKUP_TYPE" == "full" ]]; then
  BACKUP_FILE="${BACKUP_DIR}/pg-full-${TIMESTAMP}.tar.zst.age"
  BACKUP_TEMP="${BACKUP_DIR}/pg-full-${TIMESTAMP}.tmp"

  log_info "Starting full backup" "host" "$PGHOST" "timestamp" "$TIMESTAMP"

  # pg_basebackup -> zstd -> age encrypt -> file
  pg_basebackup \
    --host="$PGHOST" \
    --port="$PGPORT" \
    --username="$PGUSER" \
    --pgdata=- \
    --format=tar \
    --checkpoint=fast \
    --wal-method=none \
    --no-password \
    --label="faso-full-${TIMESTAMP}" \
  | zstd -"${ZSTD_LEVEL}" --threads=0 \
  | age --encrypt --recipients-file "$BACKUP_ENCRYPTION_KEY_FILE" \
    -o "$BACKUP_TEMP"

  mv "$BACKUP_TEMP" "$BACKUP_FILE"

  BACKUP_SIZE="$(stat -c%s "$BACKUP_FILE" 2>/dev/null || stat -f%z "$BACKUP_FILE")"
  log_info "Backup compressed and encrypted" \
    "file" "$BACKUP_FILE" "size_bytes" "$BACKUP_SIZE"

  # Upload to S3 — weekly slot
  S3_KEY="${S3_PREFIX}/weekly/${WEEK_TAG}/pg-full-${TIMESTAMP}.tar.zst.age"
  log_info "Uploading to S3" "bucket" "$S3_BUCKET" "key" "$S3_KEY"

  s3cmd put "$BACKUP_FILE" \
    "s3://${S3_BUCKET}/${S3_KEY}" \
    --host="${S3_ENDPOINT}" \
    --host-bucket="${S3_BUCKET}.${S3_ENDPOINT}" \
    --no-mime-magic \
    --quiet

  # Also copy to monthly and yearly slots if applicable
  # Monthly: keep one per month (overwrite the monthly slot)
  S3_MONTHLY="${S3_PREFIX}/monthly/${MONTH_TAG}/pg-full-${TIMESTAMP}.tar.zst.age"
  s3cmd cp "s3://${S3_BUCKET}/${S3_KEY}" "s3://${S3_BUCKET}/${S3_MONTHLY}" \
    --host="${S3_ENDPOINT}" \
    --host-bucket="${S3_BUCKET}.${S3_ENDPOINT}" \
    --quiet 2>/dev/null || true

  # Yearly: first backup of the year becomes the yearly
  YEARLY_EXISTS=$(s3cmd ls "s3://${S3_BUCKET}/${S3_PREFIX}/yearly/${YEAR_TAG}/" \
    --host="${S3_ENDPOINT}" \
    --host-bucket="${S3_BUCKET}.${S3_ENDPOINT}" 2>/dev/null | head -1 || true)
  if [[ -z "$YEARLY_EXISTS" ]]; then
    S3_YEARLY="${S3_PREFIX}/yearly/${YEAR_TAG}/pg-full-${TIMESTAMP}.tar.zst.age"
    s3cmd cp "s3://${S3_BUCKET}/${S3_KEY}" "s3://${S3_BUCKET}/${S3_YEARLY}" \
      --host="${S3_ENDPOINT}" \
      --host-bucket="${S3_BUCKET}.${S3_ENDPOINT}" \
      --quiet 2>/dev/null || true
  fi

  # Remove local backup (S3 is the source of truth)
  rm -f "$BACKUP_FILE"

  log_info "Upload complete" "s3_key" "$S3_KEY" "size_bytes" "$BACKUP_SIZE"

else
  log_error "Unknown backup type" "type" "$BACKUP_TYPE"
  exit 1
fi

# ---------------------------------------------------------------------------
# Retention cleanup
# ---------------------------------------------------------------------------
log_info "Running retention cleanup"

# Weekly: keep last 4
cleanup_old_backups() {
  local prefix="$1" keep="$2"
  local items
  items=$(s3cmd ls "s3://${S3_BUCKET}/${prefix}/" \
    --host="${S3_ENDPOINT}" \
    --host-bucket="${S3_BUCKET}.${S3_ENDPOINT}" 2>/dev/null \
    | awk '{print $NF}' \
    | sort -r)
  local count=0
  while IFS= read -r item; do
    [[ -z "$item" ]] && continue
    count=$((count + 1))
    if [[ $count -gt $keep ]]; then
      log_info "Deleting old backup" "path" "$item"
      s3cmd del "$item" \
        --host="${S3_ENDPOINT}" \
        --host-bucket="${S3_BUCKET}.${S3_ENDPOINT}" \
        --quiet --recursive 2>/dev/null || true
    fi
  done <<< "$items"
}

cleanup_old_backups "${S3_PREFIX}/weekly"  4
cleanup_old_backups "${S3_PREFIX}/monthly" 12
cleanup_old_backups "${S3_PREFIX}/yearly"  1

# ---------------------------------------------------------------------------
# Push success metrics
# ---------------------------------------------------------------------------
push_metric "faso_backup_last_success_timestamp" "$(date +%s)"
push_metric "faso_backup_last_status" "1"
push_metric "faso_backup_size_bytes" "${BACKUP_SIZE:-0}"

log_info "Backup completed successfully" \
  "type" "$BACKUP_TYPE" "timestamp" "$TIMESTAMP"
