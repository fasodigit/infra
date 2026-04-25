#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# consul-backup.sh — Consul snapshot backup for FASO DIGITALISATION
#
# Features:
#   - consul snapshot save
#   - Encrypt with age + upload to S3
#   - Retention: 7 daily + 4 weekly
#   - Prometheus pushgateway metric
#   - Structured JSON logging for Loki
#
# Environment variables (required):
#   CONSUL_HTTP_ADDR     — Consul address (default: http://127.0.0.1:8500)
#   CONSUL_HTTP_TOKEN    — Consul ACL token (optional, for ACL-enabled clusters)
#   BACKUP_ENCRYPTION_KEY_FILE — path to age public key file
#   S3_BUCKET            — S3 bucket name
#   S3_ENDPOINT          — S3-compatible endpoint URL
#
# Usage:
#   ./consul-backup.sh

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
CONSUL_HTTP_ADDR="${CONSUL_HTTP_ADDR:-http://127.0.0.1:8500}"
BACKUP_DIR="${BACKUP_DIR:-/tmp/faso-consul-backups}"
BACKUP_ENCRYPTION_KEY_FILE="${BACKUP_ENCRYPTION_KEY_FILE:?BACKUP_ENCRYPTION_KEY_FILE is required}"
S3_BUCKET="${S3_BUCKET:?S3_BUCKET is required}"
S3_ENDPOINT="${S3_ENDPOINT:?S3_ENDPOINT is required}"
S3_PREFIX="${S3_PREFIX:-faso/consul}"
PUSHGATEWAY_URL="${PUSHGATEWAY_URL:-}"

export CONSUL_HTTP_ADDR

TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
DATE_TAG="$(date -u +%Y%m%d)"
WEEK_TAG="$(date -u +%Yw%V)"

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
  printf '{"ts":"%s","level":"%s","component":"consul-backup","msg":"%s"%s}\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$level" "$msg" "$extra"
}

log_info()  { log_json "info"  "$@"; }
log_error() { log_json "error" "$@"; }

# ---------------------------------------------------------------------------
# Metrics push
# ---------------------------------------------------------------------------
push_metric() {
  local metric_name="$1" value="$2" job="${3:-faso-consul-backup}"
  if [[ -n "$PUSHGATEWAY_URL" ]]; then
    cat <<METRIC_EOF | curl -fsS --max-time 10 --data-binary @- \
      "${PUSHGATEWAY_URL}/metrics/job/${job}" 2>/dev/null || true
# HELP ${metric_name} FASO Consul backup metric
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
    log_error "Consul backup FAILED" "exit_code" "$exit_code"
    push_metric "faso_consul_backup_last_failure_timestamp" "$(date +%s)"
    push_metric "faso_consul_backup_last_status" "0"
  fi
  rm -f "${BACKUP_DIR}/"*.tmp 2>/dev/null || true
  exit "$exit_code"
}
trap cleanup EXIT

# ---------------------------------------------------------------------------
# Validate prerequisites
# ---------------------------------------------------------------------------
for cmd in consul age s3cmd; do
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
# Check Consul health
# ---------------------------------------------------------------------------
CONSUL_LEADER=$(curl -fsS "${CONSUL_HTTP_ADDR}/v1/status/leader" 2>/dev/null || echo '""')
if [[ "$CONSUL_LEADER" == '""' || -z "$CONSUL_LEADER" ]]; then
  log_error "Consul has no leader or is unreachable"
  exit 1
fi

log_info "Consul leader detected" "leader" "$CONSUL_LEADER"

# ---------------------------------------------------------------------------
# Take Consul snapshot
# ---------------------------------------------------------------------------
SNAPSHOT_FILE="${BACKUP_DIR}/consul-snapshot-${TIMESTAMP}.snap"
log_info "Taking Consul snapshot" "consul_addr" "$CONSUL_HTTP_ADDR"

consul snapshot save "$SNAPSHOT_FILE"

if [[ ! -f "$SNAPSHOT_FILE" ]]; then
  log_error "Snapshot file not created"
  exit 1
fi

# Verify snapshot integrity
consul snapshot inspect "$SNAPSHOT_FILE" >/dev/null 2>&1
if [[ $? -ne 0 ]]; then
  log_error "Snapshot verification failed"
  exit 1
fi

SNAPSHOT_SIZE="$(stat -c%s "$SNAPSHOT_FILE" 2>/dev/null || stat -f%z "$SNAPSHOT_FILE")"
log_info "Snapshot created and verified" "size_bytes" "$SNAPSHOT_SIZE"

# ---------------------------------------------------------------------------
# Encrypt + Upload
# ---------------------------------------------------------------------------
ENCRYPTED_FILE="${BACKUP_DIR}/consul-snapshot-${TIMESTAMP}.snap.age"

log_info "Encrypting snapshot"
age --encrypt --recipients-file "$BACKUP_ENCRYPTION_KEY_FILE" \
  -o "${ENCRYPTED_FILE}.tmp" \
  "$SNAPSHOT_FILE"

mv "${ENCRYPTED_FILE}.tmp" "$ENCRYPTED_FILE"
rm -f "$SNAPSHOT_FILE"

ENCRYPTED_SIZE="$(stat -c%s "$ENCRYPTED_FILE" 2>/dev/null || stat -f%z "$ENCRYPTED_FILE")"

# Upload — daily slot
S3_DAILY="${S3_PREFIX}/daily/${DATE_TAG}/consul-snapshot-${TIMESTAMP}.snap.age"
log_info "Uploading to S3" "s3_key" "$S3_DAILY"

s3cmd put "$ENCRYPTED_FILE" \
  "s3://${S3_BUCKET}/${S3_DAILY}" \
  --host="${S3_ENDPOINT}" \
  --host-bucket="${S3_BUCKET}.${S3_ENDPOINT}" \
  --no-mime-magic \
  --quiet

# Also copy to weekly slot
S3_WEEKLY="${S3_PREFIX}/weekly/${WEEK_TAG}/consul-snapshot-${TIMESTAMP}.snap.age"
s3cmd cp "s3://${S3_BUCKET}/${S3_DAILY}" "s3://${S3_BUCKET}/${S3_WEEKLY}" \
  --host="${S3_ENDPOINT}" \
  --host-bucket="${S3_BUCKET}.${S3_ENDPOINT}" \
  --quiet 2>/dev/null || true

rm -f "$ENCRYPTED_FILE"

log_info "Upload complete" "s3_key" "$S3_DAILY"

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
      log_info "Deleting old Consul backup" "path" "$item"
      s3cmd del "$item" \
        --host="${S3_ENDPOINT}" \
        --host-bucket="${S3_BUCKET}.${S3_ENDPOINT}" \
        --quiet --recursive 2>/dev/null || true
    fi
  done <<< "$items"
}

cleanup_old_backups "${S3_PREFIX}/daily"  7
cleanup_old_backups "${S3_PREFIX}/weekly" 4

# ---------------------------------------------------------------------------
# Push success metrics
# ---------------------------------------------------------------------------
push_metric "faso_consul_backup_last_success_timestamp" "$(date +%s)"
push_metric "faso_consul_backup_last_status" "1"
push_metric "faso_consul_backup_size_bytes" "$ENCRYPTED_SIZE"

log_info "Consul backup completed successfully" "timestamp" "$TIMESTAMP"
