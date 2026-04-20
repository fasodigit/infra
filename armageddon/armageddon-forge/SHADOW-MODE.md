<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# SHADOW-MODE — Pingora ↔ hyper parity validation

ARMAGEDDON-FORGE Vague 2 B1 — **gate M5 (#107)**.

Tracker: [#108](https://github.com/faso-digitalisation/armageddon/issues/108)
Gate : [#106](https://github.com/faso-digitalisation/armageddon/issues/106)

---

## 1. Objective

Before promoting Pingora to the default backend (gate M6), we must prove
behavioural parity with the hyper path on **real prod-preview traffic** for a
sustained 48 h window. The objective is bit-exact response comparison on a
representative 10 % sample: same status, same headers (after normalisation),
same body bytes.

Success criterion (hard gate on #106):

| Bucket               | Target (48 h rolling) |
|----------------------|-----------------------|
| `identical`          | ≥ 99.9 %              |
| `header_differ`      | ≤ 0.05 %              |
| `body_differ`        | ≤ 0.01 %              |
| `status_differ`      | 0 (hard zero)         |
| `timeout_on_pingora` | ≤ `timeout_on_hyper`  |

Failure to meet any row → rollback per §6 and re-open #106.

---

## 2. Topology

```text
                       ┌──────── downstream client ────────┐
                       │                                    │
                       ▼                                    │
                ┌──────────────┐                            │
                │  armageddon  │                            │
                │    entry     │                            │
                └──────┬───────┘                            │
                       │                                    │
              ┌────────┴────────┐                           │
              │                 │                           │
              ▼                 ▼                           │
    hyper listener :8080   mirror filter ──── 10 % ───► pingora listener :8081
    (serves the live response to the client)                │
              │                                              │
              ▼                                              ▼
         upstream cluster (shared)               diff-queue writer (async)
```

Both listeners bind to the same upstream cluster (via `UpstreamRegistry`).
The hyper listener is authoritative for the downstream client — the
shadow listener's response is **never** returned to the caller.

Mirroring is implemented in `armageddon-forge` at the edge, via either:

- a dedicated `TeeFilter` registered at the head of the hyper handler chain
  (preferred: keeps the logic testable in isolation), OR
- an extension to `src/traffic_split.rs` adding a `mirror_percent` knob.

The chosen path is decided at the start of M5 — whichever is easier to
atomically disable at runtime (see §6).

---

## 3. Sampling

**Rate**: 10 % of inbound requests.
**Selector**: stable hash-based so retries / idempotency-key reuse produce
the same decision (otherwise we'd amplify flakes).

```rust
let digest = blake3::hash(ctx.request_id.as_bytes());
let bucket = u32::from_le_bytes(digest.as_bytes()[0..4].try_into().unwrap()) % 100;
let mirror = bucket < shadow_sample_percent; // default: 10
```

`shadow_sample_percent` is readable atomically at every request
(`AtomicU32`) so §6 rollback flips it to 0 in one write, without redeploy.

Excluded from mirroring (hard opt-out):

- `Upgrade: websocket` — both stacks would open two upstream sockets,
  amplifying load.
- Requests > 1 MiB body — mirror storage cost / diff cost prohibitive.
- `X-Shadow-Opt-Out: 1` header — escape hatch for debugging.
- Health check paths (`/health`, `/ready`) — noise, no signal.

---

## 4. Response-diff pipeline

### Producer (in-process, hot path)

Both listeners record into a shared in-memory DashMap keyed by
`ctx.request_id`:

```rust
struct InFlight {
    hyper_resp: Option<MirroredResponse>,
    pingora_resp: Option<MirroredResponse>,
    started_at: Instant,
}

struct MirroredResponse {
    status: u16,
    headers: Vec<(String, Vec<u8>)>, // normalised: sorted, lowercased names
    body_hash: [u8; 32],             // blake3 of body bytes
    body_len: usize,
    finished_at: Instant,
}
```

When both slots fill (or a 30 s TTL elapses), a background worker classifies
the pair into a bucket and pushes a `ShadowEvent` onto the diff queue.

### Transport options

Two transports are supported; the operator picks one via
`ARMAGEDDON_SHADOW_SINK` env var at boot:

1. **Redpanda** (`shadow.raw` topic) — preferred for production preview,
   JSON-encoded events, 7-day retention.
2. **Local sqlite** (`/var/lib/armageddon/shadow.db`) — preferred for local
   soak tests, single-file, no infra dependency.

Both produce the same schema:

```json
{
  "ts": "2026-04-19T12:34:56Z",
  "request_id": "uuid-v4",
  "method": "GET",
  "path": "/api/v1/…",
  "hyper_status": 200,
  "pingora_status": 200,
  "bucket": "identical",
  "header_diff": null,
  "body_len_hyper": 1234,
  "body_len_pingora": 1234,
  "body_hash_hyper": "blake3-hex",
  "body_hash_pingora": "blake3-hex",
  "latency_us_hyper": 4321,
  "latency_us_pingora": 4010
}
```

### Bucket classification

Deterministic and ordered (first match wins):

1. `timeout_on_pingora` — pingora slot missing after 30 s.
2. `timeout_on_hyper` — hyper slot missing after 30 s.
3. `status_differ` — status codes differ.
4. `body_differ` — body hashes differ (includes length mismatch).
5. `header_differ` — header sets differ after normalisation.
   - Normalisation strips: `date`, `server`, `x-forge-id`, `x-forge-via`,
     `x-request-id`, `x-envoy-*`, `x-amzn-*` (infrastructure-added).
6. `identical` — everything matches.

---

## 5. Telemetry

Prometheus metrics (exposed on `/metrics` of the admin listener):

| Metric                                   | Type      | Labels                      |
|------------------------------------------|-----------|-----------------------------|
| `shadow_requests_total`                  | counter   | `outcome` (= bucket)        |
| `shadow_requests_sampled_total`          | counter   | —                           |
| `shadow_requests_skipped_total`          | counter   | `reason` (ws / size / optout)|
| `shadow_pingora_latency_seconds`         | histogram | `path_class`                |
| `shadow_hyper_latency_seconds`           | histogram | `path_class`                |
| `shadow_divergence_rate`                 | gauge     | —                           |
| `shadow_diff_queue_depth`                | gauge     | —                           |
| `shadow_diff_dropped_total`              | counter   | `reason`                    |

`shadow_divergence_rate` is computed by a 1 min recording rule:

```promql
sum(rate(shadow_requests_total{outcome!="identical"}[1m]))
  /
sum(rate(shadow_requests_total[1m]))
```

Grafana dashboard id: `forge-shadow` (under
`INFRA/observability/grafana/dashboards/`, landed with #106). Screenshot
of the first 48 h window is attached to gate #106.

---

## 6. Rollback criteria

**Automatic** (driven by alertmanager rule `ShadowDivergenceHigh`):

- `shadow_divergence_rate > 0.001` sustained for 15 min → fire the
  `forge-shadow-disable` webhook which writes
  `shadow_sample_percent = 0` via the admin API.

**Manual** (operator decision):

- Any `status_differ` event at all — one is one too many, inspect
  immediately.
- `body_differ` spike > 0.1 % sustained 5 min even if overall divergence
  stays under threshold.
- Pingora `p99 > 1.5 × hyper p99` for 10 min.
- Upstream error rate on the shadow path > 5× the hyper path.

Rollback is **reversible**: writing `shadow_sample_percent = 10` re-enables
mirroring. No restart, no redeploy.

---

## 7. Cleanup

Post-48 h procedure (whether success or failure):

1. Flip `shadow_sample_percent` to 0.
2. Wait 60 s for in-flight mirrored requests to complete / time out.
3. Drain the diff queue (worker flushes remaining `InFlight` entries).
4. Run `armageddon-admin-api shadow summary --window 48h` to emit the
   bucket table + p50/p95/p99 comparison into
   `INFRA/observability/reports/shadow-YYYYMMDD.md`.
5. Attach that report + Grafana dashboard snapshot to gate #106.
6. Retention:
   - Raw diff events → 30 d (Redpanda compaction / sqlite archive).
   - Summary report → indefinite, committed to `INFRA/observability/reports/`.
7. Tear down only if M6 accepts: remove the `TeeFilter`, close the
   `:8081` listener, land a follow-up PR revoking the shadow code paths.

If M6 is rejected: keep the shadow infrastructure in place, iterate on the
Pingora stack, re-run another 48 h window with a fresh tracking issue.

---

## 8. Test plan

Unit:

- `shadow_sampling_respects_percent` — set `shadow_sample_percent = 50`,
  check ≈ 50 % of 10 k synthetic requests are sampled.
- `shadow_bucket_classifier_is_deterministic` — fixed pair → stable bucket.
- `shadow_header_normalisation_drops_infra_headers` — `date` / `server`
  removed before comparison.

Integration:

- `tests/shadow_tee_roundtrip.rs` — stand up both listeners against a mock
  upstream, send 100 requests, assert all 100 produce an `identical`
  event in the sqlite sink.

Chaos:

- Kill the Pingora listener mid-flight → bucket = `timeout_on_pingora`.
- Kill the hyper listener mid-flight → overall request fails downstream
  (acceptable: hyper is authoritative), no ghost events in diff queue.

---

## 9. Limitations

1. Streaming responses (chunked, SSE, WebSocket) are **not** diffed — body
   hashing would require buffering which defeats streaming. Such responses
   are captured as `{body_hash: null, body_len: null}` and classified in
   the `streaming` bucket (not yet listed in §4 — added in M5).
2. Upstream side-effects (POST / PUT / DELETE) are mirrored: the upstream
   sees two writes. For the 48 h window this is accepted on prod-preview
   (which runs against a sandbox DB). **Do not enable shadow mode on an
   environment backed by a production DB.**
3. Cookies and idempotency-key-sensitive endpoints may produce spurious
   `body_differ` (e.g. server-rendered CSRF tokens). Add such paths to the
   `body_diff_ignore` list in `shadow.yaml`.

---

## 10. References

- `PINGORA-MIGRATION.md` — migration design doc.
- `BENCH-METHODOLOGY.md` — throughput / latency bench protocol.
- `src/traffic_split.rs` — existing percentage-based split logic.
- Grafana dashboard `forge-shadow` (observability/grafana/dashboards/).
- Alert rules `INFRA/observability/alertmanager/rules/forge-shadow.yaml`.
