---
name: Pingora upgrade
about: Track the upgrade of the Pingora proxy backend in armageddon-forge
title: "feat(armageddon-forge): upgrade Pingora 0.3 → <TARGET_VERSION>"
labels: ["enhancement", "infrastructure", "breaking-change", "needs-shadow-validation"]
assignees: []
---

<!-- Sprint 3 #2 — generated from PINGORA-04-UPGRADE-AUDIT.md -->

## Summary

Upgrade `armageddon-forge`'s Pingora dependency from the current locked version
(`0.3.0`) to `<TARGET_VERSION>` and replace the documented workarounds with
native Pingora APIs.

**Audit reference:** `armageddon-forge/docs/PINGORA-04-UPGRADE-AUDIT.md`

---

## Prerequisite checklist

- [ ] Target Pingora release is ≥ `0.8.1` (wait for patch-level stabilisation)
- [ ] Rust toolchain ≥ `1.75.0` in workspace
- [ ] `feat/pingora-<TARGET_VERSION>` branch created from `main`
- [ ] Reviewed CHANGELOG between 0.3 and `<TARGET_VERSION>`

---

## Breaking changes to address

From the audit (§3.1):

- [ ] `FilterResult` → `FilterDecision` rename (~50 call-sites in `gateway.rs`,
  filter modules)
- [ ] `upstream_response_filter` signature update (`&mut Session` — ~20 sites)
- [ ] `response_body_filter` gains `end_of_stream: &mut bool` argument
- [ ] `Backend::new()` now takes `SocketAddr` instead of `String` (`upstream/lb.rs`)
- [ ] `pingora-core` explicit `features = ["openssl"]` in `Cargo.toml`
- [ ] `Error::new_in` → `Error::because` migration

---

## Workaround replacements

### W1 — WebSocket manual handshake (commit `34d922d`)

**File:** `armageddon-forge/src/pingora/protocols/websocket.rs`

- [ ] Implement `ProxyHttp::handle_websocket` hook (available since Pingora 0.7)
- [ ] Remove `WebSocketProxy` standalone struct (no longer needed for the
  main Pingora listener path)
- [ ] Remove `tokio-tungstenite` from the gateway feature dependency (keep in
  dev-dependencies if used by tests)
- [ ] Update WebSocket tests to validate the native upgrade path

### W2 — gRPC-Web body accumulation in memory (`grpc_web.rs:47-54`)

- [ ] Replace `BytesMut` full-body accumulation with chunk-level frame insertion
  in `response_body_filter`
- [ ] Validate correctness: trailer frame must still appear as last chunk
- [ ] Add test: server-streaming RPC with 10 000 frames, assert memory stays
  < 10 MB (no accumulation)

### W3 — SPIFFE post-hoc TLS validation (commit `314a89d`)

**File:** `armageddon-forge/src/pingora/upstream/mtls.rs`

- [ ] Implement dial-time TLS connector with SPIFFE SAN pinning using the
  `pingora-core` dial hook
- [ ] Preserve fail-closed invariant: connection must be aborted before any
  upstream bytes are exchanged if SAN mismatches
- [ ] Remove post-hoc check in `upstream_request_filter`
- [ ] Security regression test: mTLS with wrong SAN must yield `502`, not pass

---

## Test gates (must all pass before merge)

```bash
# Primary gate
cargo test -p armageddon-forge --features pingora --lib pingora
# Expected: ≥ 424 passed; 0 failed

# Integration gate
cargo test -p armageddon-forge --test pingora_integration --features pingora
# Expected: 10 passed; 2 ignored

# Full workspace
cargo check -p armageddon-forge --features pingora
cargo check -p armageddon

# Shadow validation (pre-merge)
# Deploy to staging with runtime: "shadow", 1% → 50% ramp via:
#   k6 run INFRA/load-testing/k6/scenarios/shadow-ramp.js
# Assert: identical ≥ 99.9%
```

---

## Rollback plan

If the upgrade introduces a regression in production:

1. Hot-reload via admin API: `POST /admin/config/runtime {"runtime":"hyper"}`
2. If hot-reload unavailable: update `armageddon.yaml` `gateway.runtime: hyper`
   and restart
3. Revert the version bump commit on the branch; `hyper-legacy` fallback remains
   compiled in

---

## Effort estimate

**L (Large)** — 2–3 engineering days.
See audit §6 for full breakdown.

---

## References

- Audit: `armageddon-forge/docs/PINGORA-04-UPGRADE-AUDIT.md`
- Pingora CHANGELOG: `https://github.com/cloudflare/pingora/blob/main/CHANGELOG.md`
- crates.io: `https://crates.io/crates/pingora`
- Shadow ramp scenario: `INFRA/load-testing/k6/scenarios/shadow-ramp.js`
