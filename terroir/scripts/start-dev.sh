#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# start-dev.sh — démarre la stack TERROIR en mode dev natif (cargo run).
#
# Mode containerisé : utiliser plutôt :
#   cd INFRA/docker/compose
#   podman-compose -f podman-compose.yml \
#                  -f ../../terroir/podman-compose.terroir.yml \
#                  --profile terroir up -d
#
# Mode dev natif (ce script) :
#   - lance chaque crate via `cargo run -p <crate>` en background ;
#   - log dans `/tmp/terroir-<crate>.log` ;
#   - PIDs persistés dans `/tmp/terroir-<crate>.pid`.
#
# Pour stopper proprement :
#   bash INFRA/terroir/scripts/stop-dev.sh   (TODO — script complémentaire)
# ou manuellement :
#   for f in /tmp/terroir-*.pid; do kill -TERM "$(cat "$f")" 2>/dev/null; done

set -Eeuo pipefail

TERROIR_ROOT="${TERROIR_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
LOG_DIR="${LOG_DIR:-/tmp}"

log() { printf '[start-dev] %s\n' "$*" >&2; }

# Liste des crates à démarrer en dev natif. ussd / buyer / mobile-bff sont
# inclus pour valider le boot ; le code métier arrive aux phases ultérieures.
CRATES=(
    "terroir-admin"        # P0 — :9904 loopback
    "terroir-ussd-simulator"  # P0 — :1080 loopback
    "terroir-core"         # P1 — :8830
    "terroir-mobile-bff"   # P1 — :8833
    "terroir-eudr"         # P1 — :8831
    "terroir-buyer"        # P3 — :8835
    "terroir-ussd"         # P3 — :8834
)

start_crate() {
    local crate="$1"
    local log_file="${LOG_DIR}/${crate}.log"
    local pid_file="${LOG_DIR}/${crate}.pid"

    if [[ -f "${pid_file}" ]] && kill -0 "$(cat "${pid_file}")" 2>/dev/null; then
        log "${crate} déjà actif (PID $(cat "${pid_file}")) — skip"
        return 0
    fi

    log "starting ${crate} → ${log_file}"
    (
        cd "${TERROIR_ROOT}"
        cargo run --quiet -p "${crate}" >"${log_file}" 2>&1 &
        echo $! > "${pid_file}"
    )
    sleep 1
}

main() {
    log "TERROIR — dev native start (cargo run, no containers)"
    log "TERROIR_ROOT=${TERROIR_ROOT}"
    log "LOG_DIR=${LOG_DIR}"
    log "Crates: ${CRATES[*]}"

    # Pré-build une fois pour réduire le temps de démarrage de chaque crate.
    log "pre-build workspace (cargo build, dev profile)"
    (cd "${TERROIR_ROOT}" && cargo build --workspace) || {
        log "FATAL: cargo build a échoué — corrige les erreurs avant de relancer."
        exit 1
    }

    for crate in "${CRATES[@]}"; do
        start_crate "${crate}"
    done

    log "Stack TERROIR (dev native) démarrée."
    log "Logs : ${LOG_DIR}/terroir-*.log"
    log "Healthchecks :"
    log "  curl http://127.0.0.1:9904/health/ready    # terroir-admin"
    log "  curl http://127.0.0.1:1080/health/ready    # terroir-ussd-simulator"
    log "  curl http://127.0.0.1:8830/health/ready    # terroir-core"
    log "Stop : kill -TERM \$(cat ${LOG_DIR}/terroir-<crate>.pid)"
}

main "$@"
