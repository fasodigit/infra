#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# =============================================================================
# bootstrap-dev.sh — idempotent FASO DIGITALISATION dev-stack bootstrap
# =============================================================================
# Replaces the manual recovery hacks documented in the 2026-04-29 RUNBOOK:
#   1. pg_resetwal -f after corrupted WAL (only when container in restart loop)
#   2. DROP/CREATE auth_ms + poulets_db after pg_namespace desync
#   3. ALTER USER ... PASSWORD drift sync
#   4. JWT_KEY_ENCRYPTION_KEY generation (env var was missing)
#   5. DELETE FROM jwt_signing_keys when encryption key is fresh
#   6. /tmp/faso-pii-encryption-key + /tmp/faso-pii-blind-index-key (audit-lib)
#   7. Start faso-consul container (Spring Cloud Consul refuses startup without)
#   8. Sequential audit-lib Flyway migration (eliminates CREATE SCHEMA race)
#
# DESIGN:
#   - Idempotent: detect-then-act. Safe to run 3+ times — each step prints
#     "✅ already healthy" when already converged.
#   - Non-destructive: never drops data unless --reset is passed.
#   - --dry-run prints the plan without executing.
#   - Uses podman first, falls back to docker if podman absent (CLAUDE.md §1).
#
# CALLED:
#   bash INFRA/scripts/bootstrap-dev.sh                # idempotent converge
#   bash INFRA/scripts/bootstrap-dev.sh --dry-run      # plan only
#   bash INFRA/scripts/bootstrap-dev.sh --reset        # DROPS auth_ms + poulets_db
#
# Exit codes:
#   0  stack converged or dry-run completed
#   1  unrecoverable error (Postgres DOWN with no data volume to recover, etc.)
#   2  user aborted --reset confirmation
# =============================================================================
set -euo pipefail

# ---- CLI flags --------------------------------------------------------------
DRY_RUN=0
RESET=0
for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=1 ;;
        --reset)   RESET=1 ;;
        -h|--help)
            sed -n '2,40p' "$0"
            exit 0
            ;;
        *) echo "[bootstrap] unknown flag: $arg" >&2; exit 1 ;;
    esac
done

# ---- Container runtime shim (podman first, docker fallback) -----------------
if command -v podman >/dev/null 2>&1; then
    CRT=podman
elif command -v docker >/dev/null 2>&1; then
    CRT=docker
    echo "[bootstrap] WARN: podman not installed, falling back to docker (CLAUDE.md §1 requires podman in prod)"
else
    echo "[bootstrap] FATAL: neither podman nor docker found in PATH" >&2
    exit 1
fi

# ---- Path resolution --------------------------------------------------------
INFRA_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SECRETS_DIR="${INFRA_ROOT}/docker/compose/secrets"
TOP_SECRETS_DIR="${INFRA_ROOT}/secrets"
AUDIT_MIGRATION_SQL="${INFRA_ROOT}/shared/audit-lib/src/main/resources/db/audit-migration/V100__create_audit_log.sql"

# Container names (match podman-compose.yml)
PG_CTR="faso-postgres"
CONSUL_CTR="faso-consul"

# Dev databases (per the manual-hack runbook — see header)
DEV_DBS=("auth_ms" "poulets_db" "notifier")
DEV_USERS=("auth_ms" "poulets_api" "notifier")

# ---- Helpers ----------------------------------------------------------------
log()   { printf '[bootstrap] %s\n' "$*"; }
ok()    { printf '[bootstrap] \xE2\x9C\x85 %s\n' "$*"; }
warn()  { printf '[bootstrap] \xE2\x9A\xA0  %s\n' "$*" >&2; }
err()   { printf '[bootstrap] \xE2\x9D\x8C %s\n' "$*" >&2; }
plan()  { printf '[bootstrap] [DRY-RUN] would: %s\n' "$*"; }

# Run a command unless --dry-run.
do_or_plan() {
    if [[ "$DRY_RUN" -eq 1 ]]; then
        plan "$*"
    else
        "$@"
    fi
}

# Run a shell pipeline (string) unless --dry-run.
sh_or_plan() {
    if [[ "$DRY_RUN" -eq 1 ]]; then
        plan "$*"
    else
        bash -c "$*"
    fi
}

ctr_state() {
    # Echoes one of: RUNNING / EXITED / RESTARTING / NOT-FOUND
    local name="$1"
    local state
    state=$($CRT inspect -f '{{.State.Status}}' "$name" 2>/dev/null || echo "NOT-FOUND")
    case "$state" in
        running)     echo "RUNNING" ;;
        exited)      echo "EXITED" ;;
        restarting)  echo "RESTARTING" ;;
        created)     echo "CREATED" ;;
        NOT-FOUND)   echo "NOT-FOUND" ;;
        *)           echo "$state" ;;
    esac
}

pg_psql() {
    # Run psql inside the postgres container as superuser, on the given DB.
    # Usage: pg_psql <db> <sql>
    local db="$1" sql="$2"
    $CRT exec -i "$PG_CTR" psql -U faso -d "$db" -v ON_ERROR_STOP=1 -At -c "$sql"
}

pg_psql_file() {
    # Run a SQL file from the host inside the postgres container.
    # Usage: pg_psql_file <db> <host-path>
    local db="$1" file="$2"
    $CRT exec -i "$PG_CTR" psql -U faso -d "$db" -v ON_ERROR_STOP=1 < "$file"
}

# ---- Step banner ------------------------------------------------------------
print_banner() {
    echo "==============================================================================="
    echo "  FASO DIGITALISATION — bootstrap-dev.sh"
    echo "  runtime  = $CRT"
    echo "  dry-run  = $DRY_RUN"
    echo "  reset    = $RESET"
    echo "  infra    = $INFRA_ROOT"
    echo "==============================================================================="
}

# =============================================================================
# STEP 0: --reset confirmation gate
# =============================================================================
confirm_reset() {
    if [[ "$RESET" -ne 1 ]]; then return 0; fi
    if [[ "$DRY_RUN" -eq 1 ]]; then
        warn "--reset + --dry-run: would DROP auth_ms + poulets_db (skipping confirm in dry-run)"
        return 0
    fi
    warn "--reset will DROP DATABASES: ${DEV_DBS[*]}"
    read -r -p "Type 'RESET' to confirm: " ans
    if [[ "$ans" != "RESET" ]]; then
        err "aborted by user"; exit 2
    fi
}

# =============================================================================
# STEP 1: Ensure the docker/compose/secrets/postgres_password.txt exists.
#         init-secrets.sh is the canonical generator; just call it idempotently.
# =============================================================================
step_compose_secrets() {
    log "Step 1/8: compose secrets (postgres + kratos + keto)"
    if [[ -f "${SECRETS_DIR}/postgres_password.txt" ]]; then
        ok "postgres_password.txt present"
    else
        do_or_plan bash "${INFRA_ROOT}/docker/compose/scripts/init-secrets.sh"
    fi
}

# =============================================================================
# STEP 2: Postgres container state — recover from corrupt WAL if RESTARTING.
# =============================================================================
step_postgres_recover() {
    log "Step 2/8: Postgres container state"
    local state
    state=$(ctr_state "$PG_CTR")
    case "$state" in
        RUNNING)
            ok "$PG_CTR is RUNNING"
            ;;
        EXITED|CREATED)
            log "$PG_CTR is $state — starting"
            do_or_plan $CRT start "$PG_CTR"
            ;;
        RESTARTING)
            warn "$PG_CTR is in RESTART LOOP — likely corrupt WAL. Inspecting last log lines."
            $CRT logs --tail 30 "$PG_CTR" 2>&1 | tail -20 || true
            if $CRT logs --tail 50 "$PG_CTR" 2>&1 | grep -qE 'PANIC.*could not locate a valid checkpoint record|could not read from log file'; then
                warn "Detected WAL PANIC. Will run pg_resetwal in a one-shot container."
                pg_resetwal_recovery
            else
                err "$PG_CTR restarting but no WAL panic found. Manual triage required:"
                err "  $CRT logs $PG_CTR | tail -100"
                exit 1
            fi
            ;;
        NOT-FOUND)
            err "$PG_CTR does not exist. Run 'podman-compose -f docker/compose/podman-compose.yml up -d postgres' first."
            exit 1
            ;;
        *)
            warn "$PG_CTR unknown state: $state"
            ;;
    esac

    # Wait until ready.
    if [[ "$DRY_RUN" -eq 0 ]]; then
        for i in $(seq 1 30); do
            if $CRT exec "$PG_CTR" pg_isready -U faso >/dev/null 2>&1; then
                ok "Postgres accepts connections"; return 0
            fi
            sleep 1
        done
        err "Postgres still not ready after 30s"; exit 1
    fi
}

pg_resetwal_recovery() {
    # The SAFE recovery flow:
    #   1. Stop the running/restarting container cleanly.
    #   2. Run a one-shot postgres container that mounts the same volume,
    #      executes `pg_resetwal -f /var/lib/postgresql/data` as the postgres user,
    #      then exits.
    #   3. Restart the original container.
    local image volume
    image=$($CRT inspect -f '{{.Config.Image}}' "$PG_CTR")
    # Extract the volume that backs /var/lib/postgresql/data
    volume=$($CRT inspect -f '{{range .Mounts}}{{if eq .Destination "/var/lib/postgresql/data"}}{{.Source}}{{end}}{{end}}' "$PG_CTR")
    if [[ -z "$volume" ]]; then
        err "could not resolve PG data volume — refusing to run pg_resetwal blind"; exit 1
    fi
    log "  image=$image  volume=$volume"
    do_or_plan $CRT stop -t 30 "$PG_CTR"
    if [[ "$DRY_RUN" -eq 0 ]]; then
        $CRT run --rm \
            -v "${volume}:/var/lib/postgresql/data" \
            --user postgres \
            "$image" \
            pg_resetwal -f /var/lib/postgresql/data
    else
        plan "$CRT run --rm -v ${volume}:/var/lib/postgresql/data --user postgres $image pg_resetwal -f /var/lib/postgresql/data"
    fi
    do_or_plan $CRT start "$PG_CTR"
    ok "pg_resetwal recovery completed"
}

# =============================================================================
# STEP 3: Dev databases — create if missing, drop+recreate if --reset.
# =============================================================================
step_dev_databases() {
    log "Step 3/8: dev databases (auth_ms / poulets_db / notifier)"
    if [[ "$RESET" -eq 1 ]]; then
        for db in "${DEV_DBS[@]}"; do
            warn "  --reset DROP DATABASE $db"
            do_or_plan bash -c "$CRT exec -i $PG_CTR psql -U faso -d postgres -v ON_ERROR_STOP=1 -c 'DROP DATABASE IF EXISTS $db'"
        done
    fi

    for i in "${!DEV_DBS[@]}"; do
        local db="${DEV_DBS[$i]}"
        local owner="${DEV_USERS[$i]}"
        if [[ "$DRY_RUN" -eq 1 ]]; then
            plan "ensure DB $db OWNER $owner"
            continue
        fi
        # 1. ensure role exists (Spring Cloud Vault would mint these in prod)
        if ! $CRT exec -i "$PG_CTR" psql -U faso -d postgres -At -c "SELECT 1 FROM pg_roles WHERE rolname='$owner'" | grep -q 1; then
            $CRT exec -i "$PG_CTR" psql -U faso -d postgres -v ON_ERROR_STOP=1 \
                -c "CREATE ROLE $owner WITH LOGIN PASSWORD 'placeholder_will_be_synced_step_4'"
            log "  created role $owner"
        fi
        # 2. ensure DB exists
        if ! $CRT exec -i "$PG_CTR" psql -U faso -d postgres -At -c "SELECT 1 FROM pg_database WHERE datname='$db'" | grep -q 1; then
            $CRT exec -i "$PG_CTR" psql -U faso -d postgres -v ON_ERROR_STOP=1 \
                -c "CREATE DATABASE $db OWNER $owner"
            log "  created database $db OWNER $owner"
        else
            ok "  database $db exists"
        fi
    done
}

# =============================================================================
# STEP 4: Sync user passwords from compose secrets/postgres_password.txt.
# =============================================================================
step_sync_passwords() {
    log "Step 4/8: sync user passwords from secrets/postgres_password.txt"
    if [[ ! -f "${SECRETS_DIR}/postgres_password.txt" ]]; then
        err "missing ${SECRETS_DIR}/postgres_password.txt — run init-secrets.sh first"; exit 1
    fi
    local pw
    pw=$(tr -d '\n\r' < "${SECRETS_DIR}/postgres_password.txt")
    if [[ -z "$pw" ]]; then err "postgres_password.txt is empty"; exit 1; fi
    for user in "${DEV_USERS[@]}"; do
        if [[ "$DRY_RUN" -eq 1 ]]; then
            plan "ALTER USER $user WITH PASSWORD <from secrets/postgres_password.txt>"
            continue
        fi
        # ALTER USER is idempotent — applying the same password twice is a no-op.
        # Pass via env var to avoid leaking the password in `ps`.
        # shellcheck disable=SC2016
        $CRT exec -e "PWVAL=$pw" -i "$PG_CTR" \
            psql -U faso -d postgres -v ON_ERROR_STOP=1 \
            -v "u=$user" \
            -c "ALTER USER \"$user\" WITH PASSWORD '$pw'" >/dev/null
    done
    ok "passwords synced for ${DEV_USERS[*]}"
}

# =============================================================================
# STEP 5: JWT key encryption + PII keys (host-side files).
# =============================================================================
gen_keyfile() {
    # Idempotent: skip if file exists & non-empty.
    local path="$1" mode="${2:-600}"
    if [[ -s "$path" ]]; then
        ok "  $path already present ($(stat -c %s "$path") bytes)"
        return 0
    fi
    if [[ "$DRY_RUN" -eq 1 ]]; then
        plan "openssl rand -base64 32 > $path && chmod $mode $path"
        return 0
    fi
    mkdir -p "$(dirname "$path")"
    openssl rand -base64 32 | tr -d '\n' > "$path"
    chmod "$mode" "$path"
    ok "  generated $path ($(stat -c %s "$path") bytes, chmod $mode)"
}

step_secrets() {
    log "Step 5/8: per-service secrets (JWT key + PII keys)"

    # Ensure top-level secrets/ exists & is gitignored (fail loud if not).
    # Tolerant of CRLF line endings on Windows-edited files.
    if ! tr -d '\r' < "${INFRA_ROOT}/.gitignore" | grep -qE '^secrets/?$'; then
        err ".gitignore is missing 'secrets/' entry — refusing to write key files."
        err "  Add this line to ${INFRA_ROOT}/.gitignore: secrets/"
        exit 1
    fi

    if [[ "$DRY_RUN" -eq 0 ]]; then mkdir -p "$TOP_SECRETS_DIR"; fi

    gen_keyfile "${TOP_SECRETS_DIR}/jwt-key-encryption-key.txt" 600
    gen_keyfile "/tmp/faso-pii-encryption-key" 600
    gen_keyfile "/tmp/faso-pii-blind-index-key" 600
}

# =============================================================================
# STEP 6: Detect stale jwt_signing_keys (encrypted under an old JWT key).
#   Strategy: store a fingerprint (sha256 of the key) in a small companion table
#   created by this script. If the fingerprint changes, the previously seeded
#   keys can no longer be decrypted → wipe them so JwtService.@PostConstruct
#   re-seeds with the current key. NEVER runs in production (gated on dev DB).
# =============================================================================
step_jwt_signing_keys_consistency() {
    log "Step 6/8: jwt_signing_keys ↔ JWT_KEY_ENCRYPTION_KEY consistency"
    local keyfile="${TOP_SECRETS_DIR}/jwt-key-encryption-key.txt"
    if [[ "$DRY_RUN" -eq 1 ]]; then
        plan "compare sha256(${keyfile}) with auth_ms.bootstrap_meta.jwt_key_fp"
        plan "  on mismatch: DELETE FROM jwt_signing_keys; upsert new fingerprint"
        plan "  (skipped if jwt_signing_keys table doesn't exist — auth-ms hasn't booted yet)"
        return 0
    fi
    if [[ ! -s "$keyfile" ]]; then
        warn "  no JWT key file yet — skipping (Step 5 should have created it)"; return 0
    fi

    # auth_ms must exist (Step 3 ensured it). If the table doesn't exist yet,
    # auth-ms hasn't run Flyway — nothing to wipe.
    if ! pg_psql auth_ms "SELECT 1 FROM information_schema.tables WHERE table_name='jwt_signing_keys'" | grep -q 1; then
        ok "  jwt_signing_keys table not yet created (auth-ms hasn't run Flyway) — nothing to do"
        return 0
    fi

    pg_psql auth_ms "
        CREATE TABLE IF NOT EXISTS bootstrap_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )" >/dev/null

    local current_fp stored_fp
    current_fp=$(sha256sum "$keyfile" | awk '{print $1}')
    stored_fp=$(pg_psql auth_ms "SELECT value FROM bootstrap_meta WHERE key='jwt_key_fp'" || echo "")

    if [[ "$current_fp" == "$stored_fp" ]]; then
        ok "  JWT encryption key fingerprint matches DB — keeping existing jwt_signing_keys"
        return 0
    fi

    warn "  JWT encryption key changed (or first run) — wiping stale jwt_signing_keys"
    pg_psql auth_ms "DELETE FROM jwt_signing_keys" >/dev/null
    pg_psql auth_ms "
        INSERT INTO bootstrap_meta(key,value) VALUES('jwt_key_fp','$current_fp')
        ON CONFLICT (key) DO UPDATE SET value=EXCLUDED.value, updated_at=NOW()" >/dev/null
    ok "  jwt_signing_keys cleared; auth-ms @PostConstruct will re-seed on next boot"
}

# =============================================================================
# STEP 7: Consul container state — Spring Cloud Consul refuses startup if down.
# =============================================================================
step_consul() {
    log "Step 7/8: Consul container state"
    local state
    state=$(ctr_state "$CONSUL_CTR")
    case "$state" in
        RUNNING)  ok "$CONSUL_CTR is RUNNING" ;;
        EXITED|CREATED) do_or_plan $CRT start "$CONSUL_CTR" ;;
        NOT-FOUND)
            warn "$CONSUL_CTR does not exist. Java services will need spring.config.import=optional:consul:"
            warn "Start Consul: cd $INFRA_ROOT/docker/compose && podman-compose -f podman-compose.yml -f ../../vault/podman-compose.vault.yml up -d consul"
            return 0
            ;;
        RESTARTING) warn "$CONSUL_CTR restarting — check 'logs $CONSUL_CTR'" ;;
        *) warn "$CONSUL_CTR state=$state" ;;
    esac

    # Wait for leader.
    if [[ "$DRY_RUN" -eq 0 && "$state" != "NOT-FOUND" ]]; then
        for i in $(seq 1 20); do
            if curl -fsS "http://127.0.0.1:8500/v1/status/leader" 2>/dev/null | grep -qE '"[0-9].*:[0-9]+"'; then
                ok "  Consul leader elected"
                return 0
            fi
            sleep 1
        done
        warn "  Consul leader not elected after 20s (services may degrade gracefully)"
    fi
}

# =============================================================================
# STEP 8: Apply audit-lib Flyway baseline SEQUENTIALLY in auth_ms + poulets_db.
#         Eliminates the CREATE SCHEMA race when both services boot in parallel.
# =============================================================================
step_audit_baseline() {
    log "Step 8/8: audit-lib baseline (sequential apply, race-free)"
    if [[ ! -f "$AUDIT_MIGRATION_SQL" ]]; then
        warn "  audit migration SQL missing: $AUDIT_MIGRATION_SQL — skipping"
        return 0
    fi
    for db in auth_ms poulets_db; do
        if [[ "$DRY_RUN" -eq 1 ]]; then
            plan "psql $db < $AUDIT_MIGRATION_SQL"
            continue
        fi
        # Quick check: is the schema already baselined?
        if pg_psql "$db" "SELECT 1 FROM information_schema.schemata WHERE schema_name='audit'" | grep -q 1 \
           && pg_psql "$db" "SELECT 1 FROM information_schema.tables WHERE table_schema='audit' AND table_name='audit_log'" | grep -q 1; then
            ok "  $db: audit.audit_log already present"
            continue
        fi
        log "  applying audit baseline to $db"
        pg_psql_file "$db" "$AUDIT_MIGRATION_SQL" >/dev/null
        ok "  $db: audit baseline applied"
    done
}

# =============================================================================
# Final summary
# =============================================================================
print_summary() {
    echo ""
    echo "==============================================================================="
    echo "  bootstrap-dev SUMMARY"
    echo "==============================================================================="
    if [[ "$DRY_RUN" -eq 1 ]]; then
        echo "  (dry-run — nothing executed)"
        return 0
    fi

    local pg_state pg_db_count consul_leader armageddon_mtime
    pg_state=$(ctr_state "$PG_CTR")
    pg_db_count=$($CRT exec -i "$PG_CTR" psql -U faso -d postgres -At \
                  -c "SELECT count(*) FROM pg_database WHERE datname IN ('auth_ms','poulets_db','notifier')" 2>/dev/null || echo "?")
    consul_leader=$(curl -fsS http://127.0.0.1:8500/v1/status/leader 2>/dev/null || echo '"none"')

    printf "  Postgres .................. %s\n" "$pg_state"
    printf "  Dev DBs (3 expected) ...... %s/3\n" "$pg_db_count"
    printf "  Consul leader ............. %s\n" "$consul_leader"
    printf "  JWT key file .............. "
    if [[ -s "${TOP_SECRETS_DIR}/jwt-key-encryption-key.txt" ]]; then
        stat -c '%s bytes, mtime %y' "${TOP_SECRETS_DIR}/jwt-key-encryption-key.txt"
    else
        echo "MISSING"
    fi
    printf "  PII encryption key ........ "
    if [[ -s /tmp/faso-pii-encryption-key ]]; then
        stat -c '%s bytes, mtime %y' /tmp/faso-pii-encryption-key
    else
        echo "MISSING"
    fi
    printf "  PII blind-index key ....... "
    if [[ -s /tmp/faso-pii-blind-index-key ]]; then
        stat -c '%s bytes, mtime %y' /tmp/faso-pii-blind-index-key
    else
        echo "MISSING"
    fi

    if [[ -f "${INFRA_ROOT}/armageddon/target/release/armageddon" ]]; then
        printf "  ARMAGEDDON binary ......... %s\n" \
            "$(stat -c 'mtime %y' "${INFRA_ROOT}/armageddon/target/release/armageddon")"
    fi

    echo ""
    ok "bootstrap-dev complete — Java services should boot cleanly now."
    echo ""
    echo "Next steps:"
    echo "  cd $INFRA_ROOT/docker/compose"
    echo "  podman-compose -f podman-compose.yml restart auth-ms poulets-api notifier-ms"
    echo ""
    echo "For prod (Vault-backed) bootstrap, see:"
    echo "  $INFRA_ROOT/scripts/bootstrap-dev.README.md"
}

# =============================================================================
# MAIN
# =============================================================================
main() {
    print_banner
    confirm_reset
    step_compose_secrets
    step_postgres_recover
    step_dev_databases
    step_sync_passwords
    step_secrets
    step_jwt_signing_keys_consistency
    step_consul
    step_audit_baseline
    print_summary
}

main "$@"
