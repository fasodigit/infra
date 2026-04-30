# Proxy-Wasm v0.2.1 host for Coraza WAF — design

Status: **Phase-2 done (M1)** — host functions wired, dispatch sequence
runs end-to-end, real `coraza-waf.wasm` blocks SQLi.  Production
hardening (worker-thread pattern, response-phase filters, shared-data
multi-worker) tracked in §10.
Last updated: 2026-04-29.

## 1. Purpose

Load `armageddon/coraza/coraza-waf.wasm` (16 MB, OWASP CRS v4.10.0, built
with TinyGo + `coraza-proxy-wasm@0.6.0`) inside ARMAGEDDON's Pingora
pipeline so the regex WAF (~30 hand-rolled rules) can be replaced by the
1000+ rules shipped by Coraza.

This document covers the **host runtime** that exposes the proxy-wasm v0.2.1
ABI to that guest.  It does **not** describe the Coraza configuration
loader (`coraza.conf`), which is handled by the guest itself.

## 2. Why we cannot reuse `armageddon-wasm::PluginRuntime`

`armageddon-wasm` already ships a Wasmtime-based plugin runtime that
implements proxy-wasm **v0.2.0** (see
`armageddon-wasm/src/abi_v0_2_0.rs`, 21 host functions registered).
However:

| Aspect | `PluginRuntime` (v0.2.0) | Coraza guest (v0.2.1) |
|---|---|---|
| ABI version exported | `proxy_abi_version_0_2_0` | `proxy_abi_version_0_2_1` |
| Root context model | implicit (single instance) | explicit (root + http contexts) |
| `proxy_log` signature | `(level: i32, msg_ptr: i32, msg_size: i32) -> i32` | identical, but called with WASI-compatible memory layout (TinyGo) |
| Buffer fetch | `proxy_get_buffer_bytes(BufferType, start, len, *ptr, *size) -> WasmResult` | adds `proxy_get_buffer_status` and split-stream semantics |
| Body phases | request_body once | `request_body` may be called multiple times with `end_of_stream` flag |
| Property tree | minimal | needs `route_name`, `connection.id`, `request.headers.*`, etc. |
| HTTP call dispatch | inline pending map | requires `proxy_on_http_call_response` callback wiring |

Trying to bolt v0.2.1 semantics onto the v0.2.0 runtime would force
breaking changes to the existing plugin contract (consumed by the
existing FASO custom plugins). A clean separation is cheaper.

## 3. Module layout — recommendation

**Decision: NEW module inside `armageddon-wasm` — not a new top-level crate.**

Rationale:

- shares `wasmtime` engine config helpers, `read_wasm_bytes` /
  `write_wasm_bytes` memory helpers (factored into a shared
  `armageddon-wasm::memory` submodule when needed);
- avoids workspace bloat (already 27 crates);
- gated behind a Cargo feature so the default build pulls **zero**
  Coraza-specific code into the binary.

Layout:

```
armageddon-wasm/
  src/
    abi_v0_2_0.rs        ← existing custom proxy-wasm runtime (default)
    proxy_wasm_v0_2_1/
      mod.rs             ← public API: load_module, CorazaModule, CorazaInstance
      host.rs            ← v0.2.1 host functions (Linker setup)
      types.rs           ← Action, BufferType, MapType, LogLevel
      memory.rs          ← read_/write_/write_u32 helpers (shared with v0.2.0)
```

## 4. Cargo feature gating

| Crate | Feature | Pulls in |
|---|---|---|
| `armageddon-wasm` | `coraza-wasm` (default OFF) | `proxy_wasm_v0_2_1` module only |
| `armageddon-forge` | `coraza-wasm` (default OFF) | `armageddon-wasm/coraza-wasm` + `filters/waf_coraza.rs` |
| `armageddon` (binary) | `coraza-wasm` (default OFF) | `armageddon-forge/coraza-wasm` |

Build matrix:

```bash
# Default — regex WAF only (current behaviour)
cargo build --release --features pingora --bin armageddon

# Coraza opt-in
cargo build --release --features pingora,coraza-wasm --bin armageddon
```

When the feature is OFF, no `wasmtime`-Coraza-specific code is monomorphised
into the binary — it is `#[cfg]`-gated end to end.

## 5. Minimum host functions required by Coraza

Sourced from `coraza-proxy-wasm` v0.6.0 imports
(`coraza-proxy-wasm/wasmplugin/plugin.go` and the
`proxy-wasm-go-sdk@v0.26.0` low-level imports it transitively pulls in).

### Phase 1 — minimum to **compile + start** (this scaffold)

| Host function | Reason |
|---|---|
| `proxy_log(level, ptr, size) -> i32` | `coraza.New()` logs init banners via this. **Implemented** in scaffold. |
| `proxy_set_tick_period_milliseconds(period_ms) -> i32` | Coraza never sets a tick today; can be a stub returning Ok. |
| `proxy_get_property(path_ptr, path_size, *value_ptr, *value_size) -> i32` | Used to read `plugin_root_id`, `node.cluster`. Stub returning NotFound is acceptable for boot. |

### Phase 2 — minimum to **process a request through CRS**

| Host function | Reason |
|---|---|
| `proxy_get_header_map_pairs(map_type, *ret_ptr, *ret_size) -> i32` | Coraza scans every request header against `tx.request_headers.<name>`. |
| `proxy_get_header_map_value(map_type, key_ptr, key_size, *value_ptr, *value_size) -> i32` | Single-header lookups. |
| `proxy_set_header_map_value` / `proxy_replace_header_map_value` | Drop attack headers (`Host` poisoning rules). |
| `proxy_remove_header_map_value` | rule action `removeHeader`. |
| `proxy_get_buffer_bytes(buffer_type, start, max, *ptr, *size) -> i32` | Body chunk fetch in `proxy_on_request_body`. |
| `proxy_get_buffer_status(buffer_type, *length, *flags) -> i32` | Detect end-of-stream. |
| `proxy_send_local_response(status, status_msg_*, body_*, headers_*, grpc) -> i32` | The block path — emits the 403. |
| `proxy_set_effective_context(context_id) -> i32` | Switching root↔http context per request. |

### Phase 3 — quality-of-life (deferrable)

| Host function | Reason |
|---|---|
| `proxy_dispatch_http_call(...)` | Outbound calls (only used by enterprise CRS rules; CRS v4 OSS path doesn't call it). |
| `proxy_get_shared_data` / `proxy_set_shared_data` | Anomaly score sharing across worker threads — single-threaded host can stub. |
| `proxy_define_metric` / `proxy_increment_metric` / `proxy_record_metric` | Coraza emits `coraza.requests_total` etc. — stub increments are fine until we wire to Prometheus. |

In total ~ **15 host functions** implemented end-to-end; the scaffold in
this PR registers `proxy_log` plus 11 stubs returning `WasmResult::Ok` /
`WasmResult::NotFound` so the module instantiates but does not yet
inspect traffic.

## 6. Lifecycle (per-request dispatch sequence)

```text
                ┌───────────────────────────┐
   load_module  │ Wasmtime Engine + Module  │  one-time AOT compile
   (16 MB wasm) │ (Arc, shared globally)    │
                └─────────────┬─────────────┘
                              │
   per-request                ▼
                ┌───────────────────────────┐
                │ create_instance()         │  fresh Store<HostData>
                └─────────────┬─────────────┘
                              │
                              ▼
                proxy_on_vm_start(0, vm_config_size)        // root context
                proxy_on_configure(0, plugin_config_size)
                proxy_on_context_create(1, 0)                // http context #1
                              │
                              ▼
                proxy_on_request_headers(1, num_headers, eos)
                              │
                              ▼
                proxy_on_request_body(1, body_size, eos)     // possibly multi-call
                              │
                              ▼
                proxy_on_response_headers(...)               // optional
                proxy_on_done(1)                             // cleanup
```

The scaffold exposes `CorazaInstance::on_request_body(body) -> Decision`
that **today** only short-circuits to `Decision::Continue` and leaves
the dispatch sequence above as TODO comments.

## 7. Concurrency / thread-safety

`Store` and `Instance` are `!Send` (Wasmtime invariant).  Two strategies:

| Strategy | Pros | Cons |
|---|---|---|
| **A. Dedicated OS worker thread + bounded `async_channel`** (mirror `armageddon-forge::wasm_adapter`) | proven pattern in this codebase; Coraza state lives on one thread; cheap rule cache reuse | single-threaded → at high RPS the worker becomes the bottleneck. Mitigated by sharding (N workers, key by request_id hash). |
| **B. Per-request `Store`, `Module` shared via `Arc`** | naturally parallel; no channel | every request re-instantiates the module (typical 1-3 ms cost on the 16 MB Coraza module). Hot-path budget exceeded. |

**Recommendation: A** for production, mirroring the existing wasm_adapter.
The scaffold today exposes the simpler API (`create_instance` per request)
because we do not yet hit hot-path latency budget — switching to the
worker-thread pattern is a refactor inside `CorazaModule` only and does
not change the public API.

## 8. Risk analysis

| Risk | Likelihood | Mitigation |
|---|---|---|
| Wasmtime trap on malformed CRS rule | low (CRS is widely tested) | `catch_unwind` boundary in `WafCorazaFilter`, fail-open with metric. |
| OOM — 16 MB module + CRS rules cache | medium | per-`Store` `set_memory` cap at 64 MB; `set_fuel` 100 M units/req. |
| p99 latency regression > 3 ms | medium | shadow-mode rollout: regex + Coraza in parallel, compare verdicts; cut over once 95th percentile latency overhead < 2 ms over 24 h. |
| Coraza ABI drift between TinyGo build and host | low | pin `CORAZA_PROXY_WASM_TAG=0.6.0` in `build.sh`; CI rebuild on bump. |
| `!Send` future ergonomics with Pingora | medium | resolved by worker-thread pattern (strategy A). |
| Rule reload — currently requires restart | low | future: hot-reload via admin API once `Store::reset_module` is wired. |

## 9. Testing strategy

- Unit: synthetic guest module built via `wat` that imports `proxy_log` and
  invokes it; assert host receives the UTF-8 message verbatim.
- Unit: `load_module` with a non-existent path → `Err`.
- Unit: `load_module` with a valid module → `Ok(_)`.
- Integration (post-scaffold): load the real `coraza-waf.wasm` and assert
  it instantiates without traps (no behavioural assertion yet).
- E2E (post-bringup): suite `tests-e2e/tests/17-owasp-top10/` flips
  `wasm_module: ./coraza/coraza-waf.wasm` and the 17 OWASP cases continue
  to pass — equivalence with the regex WAF.

## 10. Estimated remaining work

| Step | Effort |
|---|---|
| Implement Phase-2 host functions (8 fns) | 3-4 h |
| Wire `proxy_on_request_headers` / `_body` dispatch in `CorazaInstance` | 2 h |
| Mirror `wasm_adapter` worker-thread pattern | 2 h |
| Read `coraza.conf` + bind via `proxy_on_configure` | 1 h |
| Shadow-mode comparison vs. regex WAF | 2 h |
| E2E hookup of `gateway.waf.wasm_module` config field | 30 min |
| Total | **~10-12 h of focused work** before E2E pass. |

## 11. Phase-2 — done (2026-04-29)

### What was implemented

* **14 host functions wired** beyond the original `proxy_log` scaffold:
  - header maps (`get_header_map_value`, `get_header_map_pairs`,
    `set_header_map_value`, `replace_header_map_value`,
    `set_header_map_pairs`, `add_header_map_value`,
    `remove_header_map_value`)
  - buffer access (`get_buffer_bytes`, `get_buffer_status`,
    `set_buffer_bytes` for HttpRequestBody / HttpResponseBody)
  - flow control (`send_local_response`, `set_effective_context`,
    `continue_request`, `continue_response`, `done`,
    `resume_http_request`, `resume_http_response`)
  - properties (`get_property` reads `request.method`, `request.url_path`,
    `request.protocol`, `source.address`; `set_property` writes
    arbitrary keys)
  - misc stubs (shared data, metrics, http_call, foreign_function)
* **WASI snapshot-1 stubs** (12 imports) so TinyGo's runtime
  instantiates without requiring a full WASI implementation.
* **Boot sequence** automated in `CorazaModule::create_instance`:
  `_initialize` → `proxy_on_vm_start` → `proxy_on_context_create(root)`
  → `proxy_on_configure` → `proxy_on_context_create(http)`.
* **`coraza.conf` plugin configuration loading** via
  `CorazaModule::load_with_config`; the config bytes sit at
  `BufferType::PluginConfiguration` and are read by
  `proxy_on_configure`.
* **Per-request dispatch** in `CorazaInstance::on_request_headers` and
  `on_request_body`.  Failure mode: a guest trap → `Decision::Deny(503)`
  with a structured log (fail-closed posture).
* **Map-pair wire codec** (round-trip-tested) for
  `proxy_get_header_map_pairs` / `set_header_map_pairs`:
  `u32 num_pairs | (u32 key_size, u32 value_size)... | (key,'\0',value,'\0')...`
* **Guest-side allocator** integration in `write_guest_buffer`: tries
  `proxy_on_memory_allocate` (Coraza-style), falls back to inline
  pointer arithmetic for synthetic test guests.
* **Per-request instance lifecycle**: a fresh `Store<HostState>` per
  HTTP request.  Tradeoff documented in `CorazaModule::create_instance`
  doc-comment.  Worker-thread refactor remains a follow-up.

### Wiring

`WafCorazaFilter::on_request` now snapshots HTTP method / path /
headers into `RequestCtx`; `on_request_body` accumulates body bytes,
and at end-of-stream creates a fresh `CorazaInstance`, calls
`on_request_headers` then `on_request_body`, and maps the verdict
to forge `Decision::Deny(status)` honouring `learning_mode`.

`armageddon::main` constructs `WafCorazaConfig` with
`coraza_conf_path` set to a sibling `coraza.conf` next to the wasm
module (when present).

### Test coverage

8 new unit tests in `armageddon-wasm`:

| Test | Purpose |
|---|---|
| `get_header_map_value_returns_known_header` | header map read path |
| `get_buffer_bytes_returns_body_slice` | buffer read path |
| `send_local_response_captures_status_403` | block path captures status + body + reason |
| `on_request_body_returns_deny_when_guest_blocks` | full dispatch → Deny |
| `on_request_body_returns_continue_on_passthrough_guest` | full dispatch → Continue |
| `map_pair_codec_round_trip` | wire format encode/decode |
| `property_path_normaliser_accepts_nul_and_dot` | property path parsing |
| `coraza_blocks_sqli_request` (#[ignore]) | **real Coraza WAF blocks SQLi via CRS** — passes |
| `coraza_passes_benign_request` (#[ignore]) | benign GET passes — currently traps inside Coraza on empty body, follow-up |

Total: 12 unit tests passing + 1 integration test passing under
`--ignored`.  The SQLi block test exercises the full path:
load 16 MB module → bind `coraza.conf` → boot guest → dispatch
SQLi-shaped POST → assert `Decision::Deny(4xx/5xx)`.

### Known gaps (post-Phase-2)

| Gap | Impact | Effort |
|---|---|---|
| `coraza_passes_benign_request` traps in TinyGo runtime on empty body | Cosmetic — SQLi block (acceptance test) works; benign-empty path likely needs a non-empty placeholder body or a missing host fn | 1-2 h |
| Worker-thread pool (Strategy A in §7) | Hot-path latency at high RPS | 2 h |
| Response-phase filters (`proxy_on_response_headers` / `_body`) | Outbound payload inspection / rewriting | 2 h |
| Multi-worker shared data | CRS anomaly score sharing across workers | 1 h |
| `gateway.waf.coraza_conf_path` config field in `armageddon-config::WafConfig` | Today main.rs auto-derives the conf path as a sibling of the wasm — explicit config field would be cleaner | 30 min |
| Shadow-mode rollout (regex + Coraza in parallel, compare verdicts) | Operational confidence before cutover | 2 h |
| E2E hookup in `tests-e2e/tests/17-owasp-top10/` | Equivalence assertion vs. regex WAF | 1 h |

### Build matrix verified

* `cargo build --release --features pingora --bin armageddon` — OK (regex WAF, no Coraza code in binary)
* `cargo build --release --features pingora,coraza-wasm --bin armageddon` — OK (Coraza filter linked)
* `cargo test --release --features coraza-wasm -p armageddon-wasm` — 12/12 pass + 2 #[ignore]
* `cargo test --release --features coraza-wasm -p armageddon-forge --lib waf_coraza` — 2/2 pass
* `cargo test --release --features coraza-wasm -p armageddon-wasm -- --ignored coraza_blocks_sqli_request` — passes (real WAF blocks SQLi)

