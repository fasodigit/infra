#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# pg-restore.sh — PostgreSQL restore procedure for FASO DIGITALISATION
#
# Features:
#   - Download from S3-compatible storage
#   - Decrypt with age
#   - Decompress with zstd
#   - PITR support (restore to specific timestamp)
#   - Verification step (pg_isready + basic query)
#   - Safety: requires explicit --confirm flag
#
# Environment variables (required):
#   PGHOST              — PostgreSQL host (default: postgres)
#   PGPORT              — PostgreSQL port (default: 5432)
#   PGUSER              — PostgreSQL superuser (default: faso)
#   PGPASSWORD_FILE     — path to password file
#   BACKUP_DECRYPTION_KEY_FILE — path to age private key file
#   S3_BUCKET           — S3 bucket name
#   S3_ENDPOINT         — S3-compatible endpoint URL
#
# Usage:
#   # Restore latest weekly backup:
#   ./pg-restore.sh --confirm
#
#   # Restore specific backup from S3:
#   ./pg-restore.sh --s3-key faso/postgres/weekly/2026w16/pg-full-20260420T030000Z.tar.zst.age --confirm
#
#   # PITR — restore to specific timestamp:
#   ./pg-restore.sh --pitr "2026-04-20 14:30:00 UTC" --confirm

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
PGHOST="${PGHOST:-postgres}"
PGPORT="${PGPORT:-5432}"
PGUSER="${PGUSER:-faso}"
PGDATA="${PGDATA:-/var/lib/postgresql/data}"
RESTORE_DIR="${RESTORE_DIR:-/tmp/faso-restore}"
BACKUP_DECRYPTION_KEY_FILE="${BACKUP_DECRYPTION_KEY_FILE:?BACKUP_DECRYPTION_KEY_FILE is required}"
S3_BUCKET="${S3_BUCKET:?S3_BUCKET is required}"
S3_ENDPOINT="${S3_ENDPOINT:?S3_ENDPOINT is required}"
S3_PREFIX="${S3_PREFIX:-faso/postgres}"

CONFIRMED=false
S3_KEY=""
PITR_TARGET=""

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
while [[ $# -gt 0 ]]; do
  case "$1" in
    --confirm)       CONFIRMED=true; shift ;;
    --s3-key)        S3_KEY="$2"; shift 2 ;;
    --pitr)          PITR_TARGET="$2"; shift 2 ;;
    --pgdata)        PGDATA="$2"; shift 2 ;;
    --help|-h)
      echo "Usage: $0 [--s3-key <key>] [--pitr <timestamp>] [--pgdata <dir>] --confirm"
      echo ""
      echo "Options:"
      echo "  --confirm       Required safety flag to proceed with restore"
      echo "  --s3-key KEY    Specific S3 key to restore (default: latest weekly)"
      echo "  --pitr TS       Point-In-Time Recovery target (e.g. '2026-04-20 14:30:00 UTC')"
      echo "  --pgdata DIR    PostgreSQL data directory (default: /var/lib/postgresql/data)"
      exit 0
      ;;
    *)
      echo "ERROR: Unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

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
  printf '{"ts":"%s","level":"%s","component":"pg-restore","msg":"%s"%s}\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$level" "$msg" "$extra"
}

log_info()  { log_json "info"  "$@"; }
log_error() { log_json "error" "$@"; }
log_warn()  { log_json "warn"  "$@"; }

# ---------------------------------------------------------------------------
# Safety check
# ---------------------------------------------------------------------------
if [[ "$CONFIRMED" != "true" ]]; then
  log_error "Restore aborted — missing --confirm flag"
  echo ""
  echo "=== DANGER: THIS WILL OVERWRITE THE CURRENT DATABASE ==="
  echo ""
  echo "To proceed, re-run with --confirm:"
  echo "  $0 --confirm"
  echo ""
  echo "For PITR:"
  echo "  $0 --pitr '2026-04-20 14:30:00 UTC' --confirm"
  echo ""
  exit 1
fi

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
for cmd in zstd age s3cmd pg_isready psql; do
  if ! command -v "$cmd" &>/dev/null; then
    log_error "Missing required command" "command" "$cmd"
    exit 1
  fi
done

if [[ ! -f "$BACKUP_DECRYPTION_KEY_FILE" ]]; then
  log_error "Decryption key file not found" "path" "$BACKUP_DECRYPTION_KEY_FILE"
  exit 1
fi

mkdir -p "$RESTORE_DIR"

# ---------------------------------------------------------------------------
# Resolve S3 key (find latest if not specified)
# ---------------------------------------------------------------------------
if [[ -z "$S3_KEY" ]]; then
  log_info "No --s3-key specified, finding latest weekly backup"
  S3_KEY=$(s3cmd ls "s3://${S3_BUCKET}/${S3_PREFIX}/weekly/" \
    --host="${S3_ENDPOINT}" \
    --host-bucket="${S3_BUCKET}.${S3_ENDPOINT}" \
    --recursive 2>/dev/null \
    | grep '\.tar\.zst\.age$' \
    | sort -k1,2 \
    | tail -1 \
    | awk '{print $NF}' \
    | sed "s|s3://${S3_BUCKET}/||")

  if [[ -z "$S3_KEY" ]]; then
    log_error "No backups found in S3"
    exit 1
  fi
  log_info "Latest backup found" "s3_key" "$S3_KEY"
fi

# ---------------------------------------------------------------------------
# Download
# ---------------------------------------------------------------------------
DOWNLOAD_FILE="${RESTORE_DIR}/$(basename "$S3_KEY")"
log_info "Downloading backup from S3" "s3_key" "$S3_KEY"

s3cmd get "s3://${S3_BUCKET}/${S3_KEY}" "$DOWNLOAD_FILE" \
  --host="${S3_ENDPOINT}" \
  --host-bucket="${S3_BUCKET}.${S3_ENDPOINT}" \
  --force \
  --quiet

DOWNLOAD_SIZE="$(stat -c%s "$DOWNLOAD_FILE" 2>/dev/null || stat -f%z "$DOWNLOAD_FILE")"
log_info "Download complete" "size_bytes" "$DOWNLOAD_SIZE"

# ---------------------------------------------------------------------------
# Decrypt + Decompress
# ---------------------------------------------------------------------------
DECRYPTED_FILE="${RESTORE_DIR}/pg-restore.tar"
log_info "Decrypting and decompressing"

age --decrypt \
  --identity "$BACKUP_DECRYPTION_KEY_FILE" \
  "$DOWNLOAD_FILE" \
| zstd -d --threads=0 \
  -o "$DECRYPTED_FILE"

log_info "Decryption and decompression complete"

# ---------------------------------------------------------------------------
# Stop PostgreSQL (if running in container, the orchestrator handles this)
# ---------------------------------------------------------------------------
log_warn "Stopping PostgreSQL before restore"
if command -v pg_ctl &>/dev/null && [[ -d "$PGDATA" ]]; then
  pg_ctl -D "$PGDATA" stop -m fast 2>/dev/null || true
fi

# ---------------------------------------------------------------------------
# Restore base backup
# ---------------------------------------------------------------------------
log_info "Restoring base backup to PGDATA" "pgdata" "$PGDATA"

# Clear existing data (this is the destructive step)
if [[ -d "$PGDATA" ]]; then
  rm -rf "${PGDATA:?}"/*
fi

# Extract base backup
tar -xf "$DECRYPTED_FILE" -C "$PGDATA"

# ---------------------------------------------------------------------------
# PITR configuration (if requested)
# ---------------------------------------------------------------------------
if [[ -n "$PITR_TARGET" ]]; then
  log_info "Configuring PITR" "target" "$PITR_TARGET"

  # Create recovery.signal for PostgreSQL 12+
  touch "${PGDATA}/recovery.signal"

  # Write recovery parameters to postgresql.auto.conf
  {
    echo ""
    echo "# PITR recovery — added by pg-restore.sh on $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "recovery_target_time = '${PITR_TARGET}'"
    echo "recovery_target_action = 'promote'"
    echo "restore_command = '/scripts/pg-wal-restore.sh %f %p'"
  } >> "${PGDATA}/postgresql.auto.conf"

  log_info "PITR configured — PostgreSQL will replay WAL up to target timestamp"
fi

# ---------------------------------------------------------------------------
# Fix permissions
# ---------------------------------------------------------------------------
chown -R postgres:postgres "$PGDATA" 2>/dev/null || true
chmod 0700 "$PGDATA"

# ---------------------------------------------------------------------------
# Start PostgreSQL
# ---------------------------------------------------------------------------
log_info "Starting PostgreSQL"
if command -v pg_ctl &>/dev/null; then
  pg_ctl -D "$PGDATA" start -w -t 120
fi

# ---------------------------------------------------------------------------
# Verification
# ---------------------------------------------------------------------------
log_info "Verifying restore"

# Wait for PostgreSQL to be ready
RETRY=0
MAX_RETRIES=30
while [[ $RETRY -lt $MAX_RETRIES ]]; do
  if pg_isready -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -q 2>/dev/null; then
    break
  fi
  RETRY=$((RETRY + 1))
  sleep 2
done

if [[ $RETRY -ge $MAX_RETRIES ]]; then
  log_error "PostgreSQL did not become ready after restore"
  exit 1
fi

log_info "PostgreSQL is ready"

# Basic verification query
VERIFY_RESULT=$(psql -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -d "$PGDATABASE" \
  -t -A -c "SELECT current_timestamp, pg_is_in_recovery();" 2>/dev/null || echo "FAILED")

if [[ "$VERIFY_RESULT" == "FAILED" ]]; then
  log_error "Verification query failed"
  exit 1
fi

log_info "Verification passed" "result" "$VERIFY_RESULT"

# Check all expected databases exist
for db in faso_main kratos keto auth poulets; do
  if psql -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -lqt 2>/dev/null | grep -qw "$db"; then
    log_info "Database exists" "database" "$db"
  else
    log_warn "Database missing after restore" "database" "$db"
  fi
done

# ---------------------------------------------------------------------------
# Cleanup temp files
# ---------------------------------------------------------------------------
rm -f "$DOWNLOAD_FILE" "$DECRYPTED_FILE"

log_info "Restore completed successfully" \
  "pitr" "${PITR_TARGET:-none}" "s3_key" "$S3_KEY"

echo ""
echo "=== RESTORE COMPLETE ==="
echo "  Timestamp: $(date -u)"
echo "  Source:    s3://${S3_BUCKET}/${S3_KEY}"
echo "  PITR:      ${PITR_TARGET:-N/A}"
echo "  Status:    PostgreSQL is ready and verified"
echo ""
echo "Next steps:"
echo "  1. Verify application connectivity"
echo "  2. Check replication status (if applicable)"
echo "  3. Restart dependent services (Kratos, Keto, auth-ms, poulets-api)"
echo "========================"
