#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# kaya-backup.sh — KAYA in-memory database backup for FASO DIGITALISATION
#
# Features:
#   - Trigger KAYA snapshot via RESP3 BGSAVE command
#   - Wait for snapshot completion
#   - Compress with zstd + encrypt with age
#   - Upload to S3-compatible storage
#   - Retention: last 24 hourly + 7 daily
#   - Prometheus pushgateway metric
#   - Structured JSON logging for Loki
#
# Environment variables (required):
#   KAYA_HOST            — KAYA host (default: kaya)
#   KAYA_PORT            — KAYA port (default: 6379)
#   KAYA_DATA_DIR        — KAYA data directory (default: /var/lib/kaya)
#   BACKUP_ENCRYPTION_KEY_FILE — path to age public key file
#   S3_BUCKET            — S3 bucket name
#   S3_ENDPOINT          — S3-compatible endpoint URL
#
# Usage:
#   ./kaya-backup.sh

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
KAYA_HOST="${KAYA_HOST:-kaya}"
KAYA_PORT="${KAYA_PORT:-6379}"
KAYA_DATA_DIR="${KAYA_DATA_DIR:-/var/lib/kaya}"
KAYA_SNAPSHOT_FILE="${KAYA_SNAPSHOT_FILE:-dump.rdb}"
BACKUP_DIR="${BACKUP_DIR:-/tmp/faso-kaya-backups}"
BACKUP_ENCRYPTION_KEY_FILE="${BACKUP_ENCRYPTION_KEY_FILE:?BACKUP_ENCRYPTION_KEY_FILE is required}"
S3_BUCKET="${S3_BUCKET:?S3_BUCKET is required}"
S3_ENDPOINT="${S3_ENDPOINT:?S3_ENDPOINT is required}"
S3_PREFIX="${S3_PREFIX:-faso/kaya}"
PUSHGATEWAY_URL="${PUSHGATEWAY_URL:-}"
ZSTD_LEVEL="${ZSTD_LEVEL:-3}"

TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
HOUR_TAG="$(date -u +%Y%m%d-%H)"
DATE_TAG="$(date -u +%Y%m%d)"

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
  printf '{"ts":"%s","level":"%s","component":"kaya-backup","msg":"%s"%s}\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$level" "$msg" "$extra"
}

log_info()  { log_json "info"  "$@"; }
log_error() { log_json "error" "$@"; }
log_warn()  { log_json "warn"  "$@"; }

# ---------------------------------------------------------------------------
# Metrics push
# ---------------------------------------------------------------------------
push_metric() {
  local metric_name="$1" value="$2" job="${3:-faso-kaya-backup}"
  if [[ -n "$PUSHGATEWAY_URL" ]]; then
    cat <<METRIC_EOF | curl -fsS --max-time 10 --data-binary @- \
      "${PUSHGATEWAY_URL}/metrics/job/${job}" 2>/dev/null || true
# HELP ${metric_name} FASO KAYA backup metric
# TYPE ${metric_name} gauge
${metric_name} ${value}
METRIC_EOF
  fi
}

# ---------------------------------------------------------------------------
# Cleanup handler
# ---------------------------------------------------------------------------
cleanup() {
  local exit_code=$?
  if [[ $exit_code -ne 0 ]]; then
    log_error "KAYA backup FAILED" "exit_code" "$exit_code"
    push_metric "faso_kaya_backup_last_failure_timestamp" "$(date +%s)"
    push_metric "faso_kaya_backup_last_status" "0"
  fi
  rm -f "${BACKUP_DIR}/"*.tmp 2>/dev/null || true
  exit "$exit_code"
}
trap cleanup EXIT

# ---------------------------------------------------------------------------
# Validate prerequisites
# ---------------------------------------------------------------------------
for cmd in zstd age s3cmd; do
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
# Trigger KAYA BGSAVE
# ---------------------------------------------------------------------------
log_info "Triggering KAYA BGSAVE" "host" "$KAYA_HOST" "port" "$KAYA_PORT"

# Use kaya-cli if available, fall back to redis-cli (RESP3 compatible)
KAYA_CLI=""
if command -v kaya-cli &>/dev/null; then
  KAYA_CLI="kaya-cli"
elif command -v redis-cli &>/dev/null; then
  KAYA_CLI="redis-cli"
else
  log_error "Neither kaya-cli nor redis-cli found"
  exit 1
fi

# Record last save timestamp before BGSAVE
LAST_SAVE=$("$KAYA_CLI" -h "$KAYA_HOST" -p "$KAYA_PORT" LASTSAVE 2>/dev/null || echo "0")

# Trigger background save
BGSAVE_RESULT=$("$KAYA_CLI" -h "$KAYA_HOST" -p "$KAYA_PORT" BGSAVE 2>/dev/null)
if [[ "$BGSAVE_RESULT" != *"Background saving started"* && "$BGSAVE_RESULT" != *"OK"* ]]; then
  log_error "BGSAVE command failed" "result" "$BGSAVE_RESULT"
  exit 1
fi

log_info "BGSAVE triggered, waiting for completion"

# Wait for snapshot to complete (max 5 minutes)
TIMEOUT=300
ELAPSED=0
while [[ $ELAPSED -lt $TIMEOUT ]]; do
  CURRENT_SAVE=$("$KAYA_CLI" -h "$KAYA_HOST" -p "$KAYA_PORT" LASTSAVE 2>/dev/null || echo "0")
  if [[ "$CURRENT_SAVE" != "$LAST_SAVE" && "$CURRENT_SAVE" != "0" ]]; then
    log_info "BGSAVE completed" "elapsed_seconds" "$ELAPSED"
    break
  fi
  sleep 2
  ELAPSED=$((ELAPSED + 2))
done

if [[ $ELAPSED -ge $TIMEOUT ]]; then
  log_error "BGSAVE timed out after ${TIMEOUT}s"
  exit 1
fi

# ---------------------------------------------------------------------------
# Copy snapshot, compress, encrypt, upload
# ---------------------------------------------------------------------------
SNAPSHOT_PATH="${KAYA_DATA_DIR}/${KAYA_SNAPSHOT_FILE}"
if [[ ! -f "$SNAPSHOT_PATH" ]]; then
  log_error "Snapshot file not found" "path" "$SNAPSHOT_PATH"
  exit 1
fi

BACKUP_FILE="${BACKUP_DIR}/kaya-${TIMESTAMP}.rdb.zst.age"

log_info "Compressing and encrypting snapshot"

zstd -"${ZSTD_LEVEL}" --threads=0 -c "$SNAPSHOT_PATH" \
  | age --encrypt --recipients-file "$BACKUP_ENCRYPTION_KEY_FILE" \
    -o "${BACKUP_FILE}.tmp"

mv "${BACKUP_FILE}.tmp" "$BACKUP_FILE"

BACKUP_SIZE="$(stat -c%s "$BACKUP_FILE" 2>/dev/null || stat -f%z "$BACKUP_FILE")"
log_info "Snapshot compressed and encrypted" "size_bytes" "$BACKUP_SIZE"

# Upload — hourly slot
S3_HOURLY="${S3_PREFIX}/hourly/${HOUR_TAG}/kaya-${TIMESTAMP}.rdb.zst.age"
log_info "Uploading to S3" "s3_key" "$S3_HOURLY"

s3cmd put "$BACKUP_FILE" \
  "s3://${S3_BUCKET}/${S3_HOURLY}" \
  --host="${S3_ENDPOINT}" \
  --host-bucket="${S3_BUCKET}.${S3_ENDPOINT}" \
  --no-mime-magic \
  --quiet

# Also copy to daily slot
S3_DAILY="${S3_PREFIX}/daily/${DATE_TAG}/kaya-${TIMESTAMP}.rdb.zst.age"
s3cmd cp "s3://${S3_BUCKET}/${S3_HOURLY}" "s3://${S3_BUCKET}/${S3_DAILY}" \
  --host="${S3_ENDPOINT}" \
  --host-bucket="${S3_BUCKET}.${S3_ENDPOINT}" \
  --quiet 2>/dev/null || true

# Remove local backup
rm -f "$BACKUP_FILE"

log_info "Upload complete" "s3_key" "$S3_HOURLY"

# ---------------------------------------------------------------------------
# Retention cleanup
# ---------------------------------------------------------------------------
log_info "Running retention cleanup"

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
      log_info "Deleting old KAYA backup" "path" "$item"
      s3cmd del "$item" \
        --host="${S3_ENDPOINT}" \
        --host-bucket="${S3_BUCKET}.${S3_ENDPOINT}" \
        --quiet --recursive 2>/dev/null || true
    fi
  done <<< "$items"
}

cleanup_old_backups "${S3_PREFIX}/hourly" 24
cleanup_old_backups "${S3_PREFIX}/daily"  7

# ---------------------------------------------------------------------------
# Push success metrics
# ---------------------------------------------------------------------------
push_metric "faso_kaya_backup_last_success_timestamp" "$(date +%s)"
push_metric "faso_kaya_backup_last_status" "1"
push_metric "faso_kaya_backup_size_bytes" "$BACKUP_SIZE"

log_info "KAYA backup completed successfully" "timestamp" "$TIMESTAMP"
