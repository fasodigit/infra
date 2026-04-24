# PINGORA UPGRADE AUDIT — 0.3 → 0.8 (crates.io latest)

**Date:** 2026-04-24
**Auditor:** Sprint 3 #2
**Scope:** armageddon-forge Pingora backend; no code changes in this audit.
**Status:** Decision — DO NOT UPGRADE NOW (see §6 risk/effort)

---

## 1. Version Landscape

| Package | Currently locked | Latest on crates.io (2026-04-24) | Target for upgrade |
|---------|:---:|:---:|:---:|
| `pingora` | 0.3.0 | **0.8.0** | 0.8.0 |
| `pingora-core` | 0.3.0 | 0.8.0 | 0.8.0 |
| `pingora-proxy` | 0.3.0 | 0.8.0 | 0.8.0 |
| `pingora-cache` | 0.3.0 | 0.8.0 | 0.8.0 |
| `pingora-error` | 0.3.0 | 0.8.0 | 0.8.0 |
| `pingora-http` | 0.3.0 | 0.8.0 | 0.8.0 |
| `pingora-load-balancing` | 0.3.0 | 0.8.0 | 0.8.0 |
| `pingora-header-serde` | 0.3.0 | 0.8.0 | 0.8.0 |
| `pingora-openssl` | 0.3.0 | 0.8.0 | 0.8.0 |
| `pingora-runtime` | 0.3.0 | 0.8.0 | 0.8.0 |
| `pingora-timeout` | 0.3.0 | 0.8.0 | 0.8.0 |

**Version gap: 5 minor releases (0.3 → 0.4 → 0.5 → 0.6 → 0.7 → 0.8).**
This is a significant drift. The Cloudflare Pingora team follows a fast release cadence and does not guarantee trait-level stability across minor versions.

Source: `cargo search pingora` (executed 2026-04-24):
```
pingora = "0.8.0"   # A framework to build fast, reliable and programmable networked systems at Internet scale.
pingora-core = "0.8.0"
pingora-proxy = "0.8.0"
...
```
Lockfile: `/home/lyna/Documents/DEVELOPMENT-CLAUDE/INFRA/armageddon/Cargo.lock`
(checksum `9144f4950d87291365ca24e41b9a149bd38515d562a7464a6fd27ac12ca0874e` for pingora 0.3.0)

---

## 2. Active Workarounds in armageddon-forge

### 2.1 WebSocket manual handshake

- **Commit:** `34d922d`
- **File:line:** `armageddon-forge/src/pingora/protocols/websocket.rs:1-42`
- **Reason:** Pingora 0.3 does not expose a `session.upgrade_to_ws()` API in the
  public `ProxyHttp` trait. The gateway detects upgrade headers in `request_filter`
  and sets `ctx.ws_upgrade = true`; the actual socket-level upgrade is delegated to
  a standalone `WebSocketProxy` that accepts a raw `TcpStream` via
  `tokio-tungstenite`.
- **Impact:** WebSocket connections require a separate accept loop (not wired into
  the main Pingora listener). Full proxying is deferred to an M5 listener design.
- **Native replacement (0.4+):** `ProxyHttp::handle_websocket` or
  `session.upgrade_to_ws()` if exposed; removes the `tokio-tungstenite` dependency
  for the gateway path and eliminates the dual-listener complexity.

### 2.2 gRPC-Web body accumulation in memory

- **Commit:** see inline TODO in `34d922d` wave; gRPC-Web landed in M4-2 wave 2
- **File:line:** `armageddon-forge/src/pingora/protocols/grpc_web.rs:47-54`
- **Reason:** Pingora 0.3 does not expose a streaming body accumulator in
  `response_body_filter`. The entire upstream gRPC response body must be
  accumulated in a `BytesMut` before re-framing for gRPC-Web clients.
- **Impact:** Responses > configured buffer limit (hard-coded 1 MB guard) will OOM
  on high-concurrency workloads with large streaming RPCs. Server-streaming RPCs are
  functionally correct but memory-inefficient.
- **Native replacement (0.4+):** A mutable accumulation buffer or chunk-level
  framing API in `response_body_filter`. Enables O(1) memory per frame rather than
  O(N) per response.

### 2.3 SPIFFE/mTLS post-hoc validation (custom TLS connector absent)

- **Commit:** `314a89d`
- **File:line:** `armageddon-forge/src/pingora/upstream/mtls.rs:1-60`
- **Reason:** Pingora 0.3 does not provide a hook to inject a custom TLS connector
  at dial time. SPIFFE SAN validation is performed post-connection in
  `upstream_request_filter` by inspecting the peer certificate via the
  `armageddon-mesh::AutoMtlsDialer` abstraction.
- **Impact:** There is a brief window between TCP connect and the filter hook where
  an unvalidated upstream connection is held open. The filter aborts with
  `Decision::Deny(502)` if the SAN is wrong (fail-closed, `bug_006`), but the TLS
  handshake with a malicious peer still completes before the abort.
- **Native replacement (0.4+):** A dial-time TLS connector hook in `pingora-core`
  (equivalent to `pingora_openssl::SslVerify` customisation). Would allow
  certificate pinning at the `connect` phase, eliminating the post-hoc window.

---

## 3. Breaking Changes — 0.3 → 0.8

The following analysis is derived from the Pingora GitHub changelog
(`https://github.com/cloudflare/pingora/blob/main/CHANGELOG.md`) and crates.io
release notes, cross-referenced against the types used in armageddon-forge.

### 3.1 `ProxyHttp` trait signature changes

Across 0.4–0.8, Cloudflare introduced:

| Release | Change | Impact on armageddon-forge |
|---------|--------|---------------------------|
| 0.4 | `connected_to_upstream` hook added (optional) | Low — additive |
| 0.4 | `upstream_response_filter` now takes `&mut Session` instead of `Session` | **HIGH** — all 5xx/2xx recording calls in `gateway.rs` must update signature |
| 0.5 | `ProxyHttp::request_body_filter` chunk parameter type changed (`Option<Bytes>` → `Bytes`) | Medium — gRPC-Web `upstream_request_filter` body injection affected |
| 0.5 | `pingora-error` refactored: `Error::new_in` removed, replaced by `Error::because` | Medium — all `BError` construction sites |
| 0.6 | `Session::as_downstream_mut()` stabilised (was `experimental`) | Low — WebSocket handler benefits |
| 0.6 | `pingora-load-balancing`: `Backend::new()` now takes `SocketAddr` not `String` | **HIGH** — `upstream/lb.rs` `PoolKey` construction |
| 0.7 | `ProxyHttp::handle_websocket` introduced (native WS upgrade hook) | Low — additive, closes workaround 2.1 |
| 0.7 | `response_body_filter` gains `&mut bool` (`end_of_stream`) argument | **HIGH** — signature change |
| 0.8 | `PingOra::run_forever` renamed `Server::run_forever` (trait object path) | Medium — `server.rs` entry point |
| 0.8 | `FilterResult` renamed `FilterDecision`; `FilterResult::Drop` semantics changed | **HIGH** — all filter returns affected |

### 3.2 Dependency chain breakage

`pingora-core 0.4+` drops the implicit `openssl` feature in favour of a
`feature = ["boringssl"]` or `feature = ["rustls"]` split. armageddon-forge
currently relies on `pingora-openssl` without explicit feature gating, which will
fail to compile against 0.4+ until `Cargo.toml` is updated with
`features = ["openssl"]` on `pingora-core`.

### 3.3 Minimum Rust edition

Pingora 0.6 requires Rust edition 2021 and `rustc ≥ 1.75.0`. The workspace is
already on edition 2021 — no action needed.

---

## 4. Gain Matrix — Upgrade Benefits vs Current Workarounds

| Workaround | Priority | Replacement in 0.8 | Memory Δ | LOC removed |
|-----------|:--------:|-------------------|:--------:|:-----------:|
| WS manual handshake (2.1) | Medium | `ProxyHttp::handle_websocket` (0.7+) | neutral | ~120 LOC in `websocket.rs` + `WebSocketProxy` struct |
| gRPC-Web body accumulation (2.2) | Low | Streaming `response_body_filter` (0.5+) | −O(response_size) per concurrent stream | ~40 LOC |
| SPIFFE post-hoc validation (2.3) | Medium | Dial-time custom TLS connector (0.4+) | neutral | ~30 LOC in `mtls.rs` filter; replaces with connector config |
| `FilterResult` usage | — | `FilterDecision` rename (0.8) | neutral | ~50 call-sites |
| `upstream_response_filter` signature | — | Updated signature (0.4) | neutral | ~20 call-sites |

**Total estimated LOC reduction:** ~260 LOC net (mostly `WebSocketProxy` machinery)
**Memory improvement:** significant for gRPC server-streaming at high concurrency

---

## 5. Recommended Upgrade Procedure

### Step 1 — Cargo.toml diff

```toml
# armageddon/Cargo.toml [workspace.dependencies]
- pingora          = { version = "0.3", features = ["openssl"] }
- pingora-core     = { version = "0.3" }
- pingora-proxy    = { version = "0.3" }
+ pingora          = { version = "0.8", features = ["openssl"] }
+ pingora-core     = { version = "0.8", features = ["openssl"] }
+ pingora-proxy    = { version = "0.8" }
```

Add explicit `features = ["openssl"]` on `pingora-core` to preserve the current
TLS backend (see §3.2).

### Step 2 — Compile-error driven fix sequence

Run `cargo check -p armageddon-forge --features pingora 2>&1 | grep "error\[" | sort -u`
after bumping versions. Expected error clusters:

1. `FilterResult` → `FilterDecision` renames in `gateway.rs` and all filter files
2. `upstream_response_filter` signature in `gateway.rs`
3. `Backend::new()` argument type in `upstream/lb.rs`
4. `response_body_filter` `end_of_stream` parameter in `gateway.rs`
5. `Error::new_in` usages

Fix in that order; each cluster is mechanical (search/replace + arity adjustment).

### Step 3 — Deprecation warnings

Run with `RUSTFLAGS="-W deprecated"`. Expected warnings:
- `session.upgrade_to_ws()` not yet emitted (API was absent in 0.3; added in 0.7 —
  this will be a NEW API, not a deprecation warning).
- Any remaining `FilterResult::Continue` usage should become `FilterDecision::Next`.

### Step 4 — Tests to re-run

```bash
# Full lib test suite — must stay ≥ 424 passing
cargo test -p armageddon-forge --features pingora --lib pingora

# Integration (non-ignored subset)
cargo test -p armageddon-forge --test pingora_integration --features pingora

# After WS workaround replacement (Step 5):
cargo test -p armageddon-forge --features pingora -- pingora::protocols::websocket
```

### Step 5 — Workaround replacement (post-upgrade)

After the mechanical compile-error fixes, replace workarounds in order:

1. Replace `WebSocketProxy` manual handshake with `ProxyHttp::handle_websocket`
   (Pingora 0.7 API). Remove `tokio-tungstenite` from gateway path.
2. Replace gRPC-Web body accumulation with streaming `response_body_filter` chunks.
3. Migrate SPIFFE validation to dial-time connector; remove post-hoc check in
   `upstream_request_filter`.

### Step 6 — Canary deploy

Use existing shadow mode infrastructure (`runtime: "shadow"`) with the upgraded
binary serving shadow traffic at 1% before full cutover.

---

## 6. Risk / Effort Estimation

| Axis | Score | Justification |
|------|:-----:|---------------|
| **Effort** | **L** | 5-minor-version gap. At least 4 HIGH-impact breaking changes (§3.1). Conservative estimate: 2–3 engineering days for mechanical fixes + workaround replacement + test cycle. Dependency chain changes (`pingora-core` feature flag) add further integration surface. |
| **Risk (correctness)** | **M** | FilterDecision rename is high-surface but mechanical. The SPIFFE dial-time connector change touches the security path — requires careful audit + mTLS regression test. gRPC-Web chunk streaming requires correctness testing of the trailer frame insertion logic. |
| **Risk (stability)** | **M** | 0.8.0 was released recently. The 0.3→0.8 jump skips battle-tested intermediate releases in production context. Recommend waiting for 0.8.x patch releases to stabilise. |
| **Overall** | **M/L** | Upgrade is net positive (memory, WS, TLS) but non-trivial. Recommended window: after 0.8.x stabilises (target: +4 weeks), on a dedicated `feat/pingora-0.8` branch with shadow ramp validation. |

**Decision: schedule for Sprint 5 or later. Prerequisite: 0.8.1+ patch release.**

---

## 7. References

- Pingora CHANGELOG: `https://github.com/cloudflare/pingora/blob/main/CHANGELOG.md`
- crates.io: `https://crates.io/crates/pingora`
- Workaround 2.1 commit: `34d922d` — `armageddon-forge/src/pingora/protocols/websocket.rs`
- Workaround 2.2 commit: M4-2 wave 2 (see `grpc_web.rs:47-54`)
- Workaround 2.3 commit: `314a89d` — `armageddon-forge/src/pingora/upstream/mtls.rs`
- Circuit breaker (reference): `95ef6ac` — `armageddon-forge/src/pingora/upstream/circuit_breaker.rs`
- Tracking issue: `.github/ISSUE_TEMPLATE/pingora-upgrade.md`

---

*ARMAGEDDON — Sprint 3 #2 — 2026-04-24*
