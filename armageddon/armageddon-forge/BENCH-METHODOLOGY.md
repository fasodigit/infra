<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# BENCH-METHODOLOGY — Pingora vs hyper throughput / latency

ARMAGEDDON-FORGE Vague 2 B1 — **gate M5 (#107) support doc for #106**.

This doc describes how the numbers quoted in gate #106 ("P99 ≤ 80 % hyper
OR throughput ≥ 130 % hyper") are measured. Every row in a results CSV
must be reproducible by following this protocol verbatim.

---

## 1. Targets

**Either of the following qualifies Pingora for M6 cutover** (not both, not
an average — a single run passing one threshold is enough when the other
run is within 5 %):

| Criterion          | Threshold            | Interpretation                          |
|--------------------|----------------------|-----------------------------------------|
| P99 latency        | Pingora ≤ 80 % hyper | 20 % latency reduction at the tail      |
| RPS (throughput)   | Pingora ≥ 130 % hyper| 30 % throughput uplift at same load     |

The shadow-mode run (see `SHADOW-MODE.md`) is a separate parity gate. This
doc covers **synthetic benchmarks** only.

---

## 2. Upstream fixture

A minimal HTTP/1.1 keep-alive server returning a fixed 2 KB body.

Two implementations are accepted (whichever is installed on the bench host):

- **Python 3.11+** — `python3 -m http.server` is NOT suitable (too slow).
  The fixture embedded in `benches/pingora_vs_hyper.sh` uses
  `http.server.ThreadingHTTPServer` with a minimal handler and `HTTP/1.1`
  protocol version (keep-alive).
- **Rust hyper example** — `cargo run --release --example echo2k -p
  armageddon-bench` (TODO #106, planned to land alongside the M5 harness).

The fixture must:

- Return exactly 2048 bytes (pre-allocated static buffer).
- Set `Content-Length: 2048` and `Connection: keep-alive`.
- Suppress access logging.
- Pin itself to a dedicated CPU core via `taskset -c <upstream_cpu>` so it
  does not compete with the backend under test.

Why 2 KB: representative of a typical JSON payload from
`poulets-api` (median 1.2 KB) and `notifier` (median 2.4 KB). At
sub-1 KB, wrk becomes syscall-bound; above 10 KB, NIC offload dominates.

---

## 3. Matrix

The full matrix evaluated at M5:

```
{hyper, pingora}
    × {plain, TLS, mTLS}
    × {engines_off, engines_5_parallel}
    × {connections: 1, 100, 1000}
```

`engines_5_parallel` simulates the production security pipeline:
SENTINEL + ARBITER + VEIL + ORACLE + AI adapters all returning
`Decision::Continue` (no actual ML eval — that is a separate bench).

Not all 36 cells are run every cycle. Priority tiers:

| Tier | Cells                                                            | Cadence       |
|------|------------------------------------------------------------------|---------------|
| P0   | both backends × plain × engines_off × {C=1, 100, 1000}           | every run     |
| P1   | both backends × plain × engines_5_parallel × {C=100, 1000}       | every run     |
| P2   | both backends × TLS × engines_off × {C=100, 1000}                | weekly        |
| P3   | both backends × mTLS × engines_5_parallel × {C=100, 1000}        | pre-cutover   |

P0 must pass before P1 is even attempted.

---

## 4. Tool

**wrk 4.2.0**, pinned. Chosen over:

- `ab` — no HdrHistogram latency percentiles, single-threaded.
- `h2load` — HTTP/2-only, our baseline is HTTP/1.1.
- `hey` — Go runtime GC jitter pollutes tail latency.

Install recipe (bench host):

```bash
git clone https://github.com/wg/wrk /tmp/wrk
cd /tmp/wrk
git checkout 4.2.0
make -j
sudo cp wrk /usr/local/bin/wrk
wrk --version    # confirm: wrk [4.2.0]
```

Invocation is encapsulated in `benches/pingora_vs_hyper.sh`. Never call
wrk directly from a runbook — always through the script, so JSON output
is normalised.

---

## 5. Host prep

The bench host **must not** be a developer laptop. Minimum spec:

- Dedicated bare-metal or VM with CPU governor = `performance`
  (`cpupower frequency-set -g performance`).
- Turbo boost disabled (`echo 1 | sudo tee /sys/devices/system/cpu/intel_pstate/no_turbo`).
- Isolated cores for the backend under test (`isolcpus=2-5` kernel cmdline)
  so the scheduler does not migrate the process.
- Separate cores for the upstream fixture (`isolcpus` includes its core
  too, e.g. core 6).
- Separate cores for wrk itself (e.g. cores 8-11).
- Swap disabled (`swapoff -a`).
- THP set to `madvise` (avoids latency spikes from background khugepaged).
- `net.core.somaxconn=4096`, `net.ipv4.tcp_tw_reuse=1`.

Pre-bench warmup: 10 s at low C (`wrk -t2 -c10 -d10s`). This populates
Pingora's connection pool and warms the upstream fixture cache.

Run parameters:

- **Duration**: 60 s per run (shorter runs hide warmup/cooldown effects).
- **Runs per cell**: 3, report **median** (not mean — RPS has a long tail).
- **Rest between runs**: 10 s, to let sockets TIME_WAIT drain.

---

## 6. Output format

Each wrk run produces one JSON file under
`benches/results/YYYYMMDD-HHMMSS-${backend}-c${connections}.json`:

```json
{
  "ts": "2026-04-19T12:34:56Z",
  "backend": "pingora",
  "connections": 100,
  "threads": 4,
  "duration": "60s",
  "warmup": "10s",
  "rps": 54321.0,
  "latency": {
    "p50": "1.23ms",
    "p75": "1.89ms",
    "p90": "2.77ms",
    "p99": "8.11ms",
    "p999": "12.44ms"
  },
  "errors": 0
}
```

Aggregation to CSV for reporting:

```bash
jq -r '[.ts, .backend, .connections, .rps, .latency.p50, .latency.p99] | @csv' \
    benches/results/*.json > benches/results/summary.csv
```

---

## 7. Reporting

PR-comment template (the M5 gate #106 author pastes this into the PR):

```
## Bench run YYYY-MM-DD

Host: <hostname>  kernel: <version>  wrk: 4.2.0

| Backend | C    | RPS    | p50    | p95    | p99    | Errors |
|---------|------|--------|--------|--------|--------|--------|
| hyper   | 1    | …      | …      | …      | …      | 0      |
| pingora | 1    | …      | …      | …      | …      | 0      |
| hyper   | 100  | …      | …      | …      | …      | 0      |
| pingora | 100  | …      | …      | …      | …      | 0      |
| hyper   | 1000 | …      | …      | …      | …      | 0      |
| pingora | 1000 | …      | …      | …      | …      | 0      |

Target check:
- [ ] P99(pingora) ≤ 80% P99(hyper) at C=100  → …
- [ ] RPS(pingora) ≥ 130% RPS(hyper) at C=100 → …

Raw JSON: `benches/results/YYYYMMDD-*/` (committed in this PR).
```

Results are committed to the repo (under
`armageddon-forge/benches/results/`) so history survives the branch.

---

## 8. Anti-patterns

Things that invalidate a run:

1. **`ab` instead of wrk** — no HdrHistogram, tail numbers are noise.
2. **Laptop bench host** — thermal throttling makes the second run
   systematically slower than the first.
3. **Single run** — one run is an anecdote. Three runs median is the
   minimum.
4. **No warmup** — Pingora's connection pool ramp artificially inflates
   p99 of the first ~5 s.
5. **Shared NIC with other tenants** — hypervisor noise adds ±10 % jitter.
6. **Upstream and backend on the same core** — you are benchmarking
   context-switch overhead, not the proxy.
7. **Debug build** — always `--release`. wrk against a debug Rust binary
   is ~5× slower and the ratio is not preserved.
8. **Mixing wrk versions** — even 4.1 vs 4.2 changes histogram bucket
   boundaries. Pin 4.2.0.
9. **Forgetting `--latency` flag** — wrk then only reports mean+stddev,
   not percentiles.
10. **Comparing P99 across runs with different error counts** — if
    `errors > 0`, throw the run out and investigate.

---

## 9. References

- `benches/pingora_vs_hyper.sh` — the actual harness.
- `benches/pingora_filter_chain_micro.rs` — Criterion micro-bench for
  the synchronous chain walker (complementary, not a replacement).
- `SHADOW-MODE.md` — parity validation (orthogonal to throughput).
- `PINGORA-MIGRATION.md` §Limitations #1 — why Criterion cannot drive
  end-to-end.
- Cloudflare Pingora paper (2023) for the RPS uplift expected from
  the persistent pool + lock-free scheduler design.
