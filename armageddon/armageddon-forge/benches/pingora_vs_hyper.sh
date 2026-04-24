#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# ARMAGEDDON-FORGE — Pingora vs hyper external throughput harness.
#
# Criterion cannot drive the Pingora scheduler inside a tokio Runtime
# (documented in PINGORA-MIGRATION.md Limitations #1), so end-to-end
# throughput / latency numbers are captured with `wrk` against a live
# `cargo run --release` server process.
#
# See also: BENCH-METHODOLOGY.md for the full measurement protocol
# (matrix, CPU pinning, warmup, reporting cadence).
#
# Required host packages:
#   * wrk (>= 4.2.0)            — https://github.com/wg/wrk
#   * jq  (>= 1.6)              — JSON parsing of the summary file
#   * numactl / taskset         — optional CPU pinning (noise isolation)
#   * bash >= 4.4               — associative arrays + `set -u` safety
#   * coreutils (date, mktemp)
#
# Usage
# -----
#   pingora_vs_hyper.sh [--features pingora|hyper]
#                       [--connections N]
#                       [--duration 60s]
#                       [--threads N]
#                       [--warmup 10s]
#                       [--upstream-port 18081]
#                       [--backend-port  18080]
#                       [--results-dir ./benches/results]
#                       [--cpu-pin "0,2"]
#                       [--skip-backend]  # use an externally-started backend
#
# Exit codes
# ----------
#   0   — wrk ran, JSON written to results dir
#   2   — missing host tool (wrk / jq)
#   3   — backend binary does not exist (TODO #106); script prints the
#         expected `cargo run ...` invocation and exits cleanly
#   4   — wrk failed / parse error
#
# The script is designed to be idempotent and safe to run back-to-back:
# every invocation creates a new timestamped JSON file under
# `benches/results/`, and upstream / backend PIDs are reaped in an EXIT
# trap even when the harness is interrupted (`kill -INT`).

set -euo pipefail

# ── defaults ──────────────────────────────────────────────────────────────
BACKEND="pingora"          # pingora | hyper
CONNECTIONS=100
DURATION="60s"
THREADS=4
WARMUP="10s"
UPSTREAM_PORT=18081
BACKEND_PORT=18080
RESULTS_DIR=""             # resolved below relative to this script
CPU_PIN=""                 # e.g. "0,2" for `taskset -c 0,2`
SKIP_BACKEND=0

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
WORKSPACE_DIR="$(cd "${CRATE_DIR}/.." && pwd)"

# ── arg parse ─────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --features)       BACKEND="$2"; shift 2 ;;
        --connections)    CONNECTIONS="$2"; shift 2 ;;
        --duration)       DURATION="$2"; shift 2 ;;
        --threads)        THREADS="$2"; shift 2 ;;
        --warmup)         WARMUP="$2"; shift 2 ;;
        --upstream-port)  UPSTREAM_PORT="$2"; shift 2 ;;
        --backend-port)   BACKEND_PORT="$2"; shift 2 ;;
        --results-dir)    RESULTS_DIR="$2"; shift 2 ;;
        --cpu-pin)        CPU_PIN="$2"; shift 2 ;;
        --skip-backend)   SKIP_BACKEND=1; shift 1 ;;
        -h|--help)
            sed -n '1,60p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "[pingora_vs_hyper] unknown arg: $1" >&2
            exit 1
            ;;
    esac
done

case "${BACKEND}" in
    pingora|hyper) ;;
    *)
        echo "[pingora_vs_hyper] --features must be 'pingora' or 'hyper' (got ${BACKEND})" >&2
        exit 1
        ;;
esac

RESULTS_DIR="${RESULTS_DIR:-${CRATE_DIR}/benches/results}"
mkdir -p "${RESULTS_DIR}"

# ── required tools ────────────────────────────────────────────────────────
for tool in wrk jq; do
    if ! command -v "${tool}" >/dev/null 2>&1; then
        echo "[pingora_vs_hyper] missing required tool: ${tool}" >&2
        echo "[pingora_vs_hyper] install: sudo apt-get install ${tool}" >&2
        exit 2
    fi
done

PIN_PREFIX=()
if [[ -n "${CPU_PIN}" ]]; then
    if command -v taskset >/dev/null 2>&1; then
        PIN_PREFIX=(taskset -c "${CPU_PIN}")
    else
        echo "[pingora_vs_hyper] taskset not found; ignoring --cpu-pin" >&2
    fi
fi

# ── state + cleanup ───────────────────────────────────────────────────────
UPSTREAM_PID=""
BACKEND_PID=""
TMP_WRK_OUT=""

cleanup() {
    local ec=$?
    if [[ -n "${UPSTREAM_PID}" ]] && kill -0 "${UPSTREAM_PID}" 2>/dev/null; then
        kill -TERM "${UPSTREAM_PID}" 2>/dev/null || true
        wait "${UPSTREAM_PID}" 2>/dev/null || true
    fi
    if [[ -n "${BACKEND_PID}" ]] && kill -0 "${BACKEND_PID}" 2>/dev/null; then
        kill -TERM "${BACKEND_PID}" 2>/dev/null || true
        wait "${BACKEND_PID}" 2>/dev/null || true
    fi
    [[ -n "${TMP_WRK_OUT}" && -f "${TMP_WRK_OUT}" ]] && rm -f "${TMP_WRK_OUT}"
    exit "${ec}"
}
trap cleanup EXIT INT TERM

# ── step 1: upstream echo server on :${UPSTREAM_PORT} ─────────────────────
start_upstream() {
    # A minimal 2KB fixed-body responder. Python 3 is assumed present on the
    # bench host; swap for a hyper example if the host is Python-less.
    python3 - "${UPSTREAM_PORT}" >/dev/null 2>&1 <<'PY' &
import http.server
import socket
import sys

PORT = int(sys.argv[1])
BODY = b"x" * 2048  # 2KB — matches BENCH-METHODOLOGY.md fixture

class Echo(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Type", "application/octet-stream")
        self.send_header("Content-Length", str(len(BODY)))
        self.send_header("Connection", "keep-alive")
        self.end_headers()
        self.wfile.write(BODY)
    def log_message(self, *a, **kw):
        pass

class ReusableServer(http.server.ThreadingHTTPServer):
    allow_reuse_address = True

srv = ReusableServer(("127.0.0.1", PORT), Echo)
srv.serve_forever()
PY
    UPSTREAM_PID=$!
    # Wait up to 5s for the port to listen.
    for _ in $(seq 1 50); do
        if (exec 3<>/dev/tcp/127.0.0.1/"${UPSTREAM_PORT}") 2>/dev/null; then
            exec 3<&-
            exec 3>&-
            return 0
        fi
        sleep 0.1
    done
    echo "[pingora_vs_hyper] upstream did not bind :${UPSTREAM_PORT}" >&2
    return 1
}

# ── step 2: FORGE backend under test on :${BACKEND_PORT} ──────────────────
start_backend() {
    local bin_name cargo_features
    if [[ "${BACKEND}" == "pingora" ]]; then
        bin_name="pingora_bench_server"
        cargo_features="--features pingora"
    else
        bin_name="hyper_bench_server"
        cargo_features=""
    fi

    # TODO(#106): these bench-only binaries are planned for M5. When they
    # land they must accept two env vars:
    #     FORGE_BENCH_LISTEN=127.0.0.1:${BACKEND_PORT}
    #     FORGE_BENCH_UPSTREAM=127.0.0.1:${UPSTREAM_PORT}
    # and forward every incoming request to the upstream with no filter chain
    # (pure baseline). A filter-on variant will come as a second bin later.
    local manifest_path="${WORKSPACE_DIR}/Cargo.toml"
    local marker
    marker=$(cargo metadata --manifest-path "${manifest_path}" --no-deps --format-version 1 2>/dev/null \
        | jq -r --arg b "${bin_name}" \
            '.packages[] | select(.name=="armageddon-forge") | .targets[] | select(.kind[]=="bin") | select(.name==$b) | .name' \
        | head -n1 || true)

    if [[ -z "${marker}" ]]; then
        cat >&2 <<EOF
[pingora_vs_hyper] backend binary '${bin_name}' not found in armageddon-forge.
[pingora_vs_hyper] This is the M5 TODO (#106). To run this harness today,
[pingora_vs_hyper] you must either:
[pingora_vs_hyper]   a) land the bench bins (see PINGORA-MIGRATION.md §M5), or
[pingora_vs_hyper]   b) start your own backend on 127.0.0.1:${BACKEND_PORT}
[pingora_vs_hyper]      forwarding to 127.0.0.1:${UPSTREAM_PORT} and re-run
[pingora_vs_hyper]      this script with --skip-backend.
[pingora_vs_hyper]
[pingora_vs_hyper] Expected invocation once #106 lands:
[pingora_vs_hyper]   FORGE_BENCH_LISTEN=127.0.0.1:${BACKEND_PORT} \\
[pingora_vs_hyper]   FORGE_BENCH_UPSTREAM=127.0.0.1:${UPSTREAM_PORT} \\
[pingora_vs_hyper]   cargo run -p armageddon-forge --release ${cargo_features} --bin ${bin_name}
EOF
        return 3
    fi

    (
        cd "${WORKSPACE_DIR}"
        FORGE_BENCH_LISTEN="127.0.0.1:${BACKEND_PORT}" \
        FORGE_BENCH_UPSTREAM="127.0.0.1:${UPSTREAM_PORT}" \
        "${PIN_PREFIX[@]}" cargo run -p armageddon-forge --release ${cargo_features} --bin "${bin_name}" \
            >/tmp/forge-bench-${BACKEND}.log 2>&1
    ) &
    BACKEND_PID=$!

    # Wait up to 60s for compile + bind.
    for _ in $(seq 1 600); do
        if (exec 3<>/dev/tcp/127.0.0.1/"${BACKEND_PORT}") 2>/dev/null; then
            exec 3<&-
            exec 3>&-
            return 0
        fi
        sleep 0.1
        if ! kill -0 "${BACKEND_PID}" 2>/dev/null; then
            echo "[pingora_vs_hyper] backend crashed during startup (see /tmp/forge-bench-${BACKEND}.log)" >&2
            return 4
        fi
    done
    echo "[pingora_vs_hyper] backend did not bind :${BACKEND_PORT} in 60s" >&2
    return 4
}

# ── step 3: warmup + bench ────────────────────────────────────────────────
run_wrk() {
    local target="http://127.0.0.1:${BACKEND_PORT}/"
    echo "[pingora_vs_hyper] warmup ${WARMUP} against ${target}"
    wrk -t2 -c10 -d"${WARMUP}" --latency "${target}" >/dev/null 2>&1 || true

    TMP_WRK_OUT="$(mktemp -t wrk-out.XXXXXX)"
    echo "[pingora_vs_hyper] bench t=${THREADS} c=${CONNECTIONS} d=${DURATION}"
    if ! wrk -t"${THREADS}" -c"${CONNECTIONS}" -d"${DURATION}" --latency "${target}" \
            >"${TMP_WRK_OUT}" 2>&1; then
        echo "[pingora_vs_hyper] wrk failed:" >&2
        cat "${TMP_WRK_OUT}" >&2
        return 4
    fi
    cat "${TMP_WRK_OUT}"
}

# ── step 4: parse wrk stdout → JSON ───────────────────────────────────────
# wrk --latency prints lines like:
#     Latency    12.34ms    4.56ms   100.10ms   85.32%
#     Req/Sec     5.67k   800.12    10.20k    75.00%
#     ...
#     Latency Distribution
#        50%    10.11ms
#        75%    15.22ms
#        90%    22.33ms
#        99%    55.44ms
#     Requests/sec:  12345.67
parse_wrk_to_json() {
    local raw="$1"
    local out="$2"

    local rps p50 p75 p90 p99 p999 errors
    rps=$(grep -E '^Requests/sec:' "${raw}" | awk '{print $2}')
    p50=$(awk '/Latency Distribution/{flag=1;next}/^[[:space:]]*50%/ && flag{print $2; exit}' "${raw}")
    p75=$(awk '/Latency Distribution/{flag=1;next}/^[[:space:]]*75%/ && flag{print $2; exit}' "${raw}")
    p90=$(awk '/Latency Distribution/{flag=1;next}/^[[:space:]]*90%/ && flag{print $2; exit}' "${raw}")
    p99=$(awk '/Latency Distribution/{flag=1;next}/^[[:space:]]*99%/ && flag{print $2; exit}' "${raw}")
    # wrk only prints p999 when --latency + enough samples; default to null.
    p999=$(awk '/Latency Distribution/{flag=1;next}/^[[:space:]]*99\.9%/ && flag{print $2; exit}' "${raw}")
    errors=$(grep -E 'Non-2xx|connect errors|read errors|write errors|timeout' "${raw}" \
                | awk '{for(i=1;i<=NF;i++) if($i ~ /^[0-9]+$/){s+=$i}} END{print s+0}')

    jq -n \
        --arg ts "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
        --arg backend "${BACKEND}" \
        --argjson connections "${CONNECTIONS}" \
        --argjson threads "${THREADS}" \
        --arg duration "${DURATION}" \
        --arg warmup "${WARMUP}" \
        --arg rps "${rps:-null}" \
        --arg p50 "${p50:-null}" \
        --arg p75 "${p75:-null}" \
        --arg p90 "${p90:-null}" \
        --arg p99 "${p99:-null}" \
        --arg p999 "${p999:-null}" \
        --arg errors "${errors:-0}" \
        '{
            ts: $ts,
            backend: $backend,
            connections: $connections,
            threads: $threads,
            duration: $duration,
            warmup: $warmup,
            rps: ($rps | tonumber? // null),
            latency: {
                p50: $p50,
                p75: $p75,
                p90: $p90,
                p99: $p99,
                p999: $p999
            },
            errors: ($errors | tonumber? // 0)
        }' > "${out}"
}

# ── orchestrate ───────────────────────────────────────────────────────────
start_upstream || exit 4

if [[ "${SKIP_BACKEND}" -eq 0 ]]; then
    if ! start_backend; then
        rc=$?
        exit "${rc}"
    fi
else
    echo "[pingora_vs_hyper] --skip-backend set; assuming a backend is already on :${BACKEND_PORT}"
fi

WRK_RAW="$(mktemp -t wrk-raw.XXXXXX)"
run_wrk | tee "${WRK_RAW}" >/dev/null

TS_SLUG="$(date -u +%Y%m%d-%H%M%S)"
OUT_FILE="${RESULTS_DIR}/${TS_SLUG}-${BACKEND}-c${CONNECTIONS}.json"
parse_wrk_to_json "${WRK_RAW}" "${OUT_FILE}"
rm -f "${WRK_RAW}"

echo "[pingora_vs_hyper] wrote ${OUT_FILE}"
jq . "${OUT_FILE}"
