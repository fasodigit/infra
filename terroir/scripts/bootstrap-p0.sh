#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# bootstrap-p0.sh — orchestrateur Phase TERROIR P0
#
# Squelette P0.A. Les scripts appelés sont implémentés par les autres
# streams P0 (B/C/D/E/F) et n'existent pas encore — TODO ci-dessous.
#
# Ordre d'exécution (cf. ULTRAPLAN-TERROIR-2026-04-30.md §4 + RUNBOOK-P0) :
#
#   1. Vault Transit + PKI (P0.B)         → seed-vault-transit-terroir.sh
#   2. PostgreSQL extensions + schemas    → seed-postgres-terroir.sh (P0.C)
#   3. Keto namespaces + 1 tuple seed     → seed-keto-terroir.sh (P0.D)
#   4. Redpanda topics + Avro schemas     → seed-redpanda-terroir.sh (P0.E)
#   5. terroir-ussd-simulator             → start-ussd-simulator.sh (P0.F)
#   6. Tenant pilote (t_pilot)            → seed-tenant-pilot.sh (P0.C)
#
# Chaque sub-script doit être idempotent et signaler son état via exit code
# (0 = OK, 1 = erreur fatale, 2 = déjà appliqué — skip).

set -Eeuo pipefail

INFRA_ROOT="${INFRA_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
TERROIR_ROOT="${INFRA_ROOT}/terroir"
SCRIPTS="${TERROIR_ROOT}/scripts"

log()  { printf '[bootstrap-p0] %s\n' "$*" >&2; }
die()  { log "FATAL: $*"; exit 1; }
step() {
    local label="$1"
    local cmd="$2"
    log ">>> ${label}"
    if [[ ! -x "${cmd}" ]]; then
        log "    SKIP — ${cmd} absent ou non exécutable (TODO P0.B/C/D/E/F)"
        return 0
    fi
    if ! "${cmd}"; then
        die "${label} — script a échoué (${cmd})"
    fi
    log "    OK"
}

main() {
    log "TERROIR Phase P0 — bootstrap orchestrator"
    log "INFRA_ROOT=${INFRA_ROOT}"
    log "TERROIR_ROOT=${TERROIR_ROOT}"

    # 1. Vault Transit + PKI (P0.B)
    step "Vault Transit + PKI"           "${INFRA_ROOT}/vault/scripts/configure-transit.sh"
    step "Vault PKI terroir"             "${INFRA_ROOT}/vault/scripts/configure-pki-terroir.sh"

    # 2. PostgreSQL extensions + multi-tenancy (P0.C)
    step "PostgreSQL extensions + shared schema" "${SCRIPTS}/seed-postgres-terroir.sh"

    # 3. Keto namespaces (P0.D)
    step "Keto namespaces + tuple seed"  "${INFRA_ROOT}/ory/keto/scripts/seed-terroir-namespaces.sh"

    # 4. Redpanda topics (P0.E)
    step "Redpanda topics + Avro schemas" "${INFRA_ROOT}/scripts/seed-redpanda-terroir-topics.sh"

    # 5. ussd-simulator (P0.F)
    step "Start ussd-simulator (loopback)" "${SCRIPTS}/start-ussd-simulator.sh"

    # 6. Tenant pilote (P0.C, post-admin online)
    step "Seed tenant pilote (t_pilot)"  "${SCRIPTS}/seed-tenant-pilot.sh"

    log "Phase P0 bootstrap completed."
}

main "$@"
