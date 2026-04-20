# PINGORA-MIGRATION

ARMAGEDDON-FORGE Vague 2 B1 — Pingora feature-gated proxy backend.

Master tracker: [#108](https://github.com/faso-digitalisation/armageddon/issues/108).
Current gate: [#101 — M0](https://github.com/faso-digitalisation/armageddon/issues/101).

## M0 — Scaffolding (completed)

Gate [#101](https://github.com/faso-digitalisation/armageddon/issues/101)
introduces the Pingora module tree, filter trait surface, per-request
context and runtime bridge.  **No hyper-path behaviour is ported** — every
filter / upstream / engine / protocol sub-module is a stub that compiles
and is wired into the chain so that M1 – M5 can land content without
further refactoring.

### New module tree

Located under `armageddon-forge/src/pingora/`:

```text
pingora/
├── mod.rs              — feature-gated entry point, re-exports
├── ctx.rs              — RequestCtx with all forward-compatible fields
├── gateway.rs          — PingoraGateway + ProxyHttp impl + filter chain
├── server.rs           — build_server() bootstrap
├── runtime.rs          — isolated tokio Runtime on a dedicated OS thread
├── RUNTIME.md          — Option A design rationale (this dir)
├── filters/
│   ├── mod.rs          — ForgeFilter trait + Decision enum
│   ├── router.rs       — stub (M1 #95)
│   ├── cors.rs         — stub (M1 #96)
│   ├── jwt.rs          — stub (M1 #97)
│   ├── feature_flag.rs — stub (M1 #98)
│   ├── otel.rs         — stub (M1 #99)
│   └── veil.rs         — stub (M1 #100)
├── upstream/
│   ├── mod.rs
│   ├── selector.rs     — stub (M2 #103)
│   ├── mtls.rs         — stub (M2)
│   ├── circuit_breaker.rs — stub (M2)
│   ├── health.rs       — stub (M2)
│   ├── lb.rs           — stub (M2)
│   └── retry.rs        — stub (M2)
├── engines/
│   ├── mod.rs
│   └── pipeline.rs     — SecurityEngine evaluate() no-op (M3 #104)
└── protocols/
    ├── mod.rs
    ├── grpc_web.rs     — stub (M4)
    ├── websocket.rs    — stub (M4)
    ├── compression.rs  — stub (M4)
    └── traffic_split.rs — stub (M4)
```

The 471-line `src/pingora_backend.rs` file has been removed; its public
types (`PingoraGateway`, `PingoraGatewayConfig`, `UpstreamRegistry`,
`build_server`) are re-exported through a backward-compat shim
(`pub mod pingora_backend { pub use crate::pingora::… }`) so internal
benches and external callers keep compiling.

### `ForgeFilter` trait + `Decision` enum

The filter trait (in `pingora::filters::mod.rs`) defines four hooks
mapped 1:1 onto Pingora's `ProxyHttp` callbacks:

| `ProxyHttp` hook           | `ForgeFilter` method     |
|----------------------------|--------------------------|
| `request_filter`           | `on_request`             |
| `upstream_request_filter`  | `on_upstream_request`    |
| `response_filter`          | `on_response`            |
| `logging`                  | `on_logging`             |

Each hook returns a `Decision`:

```rust
pub enum Decision {
    Continue,
    ShortCircuit(Box<pingora::http::ResponseHeader>),
    Deny(u16),
}
```

The `PingoraGateway` walks the filter chain in registration order at
every hook; the first non-`Continue` decision aborts the chain.
`on_logging` fans out to **all** filters regardless (access-log phase).

### Runtime bridge

Pingora's scheduler is not a tokio runtime.  Tokio-native code paths
(security engines, KAYA RESP3, xDS gRPC, SPIFFE certs) run on a
dedicated multi-threaded tokio runtime spawned on first access to
`pingora::runtime::tokio_handle()`.

See `src/pingora/RUNTIME.md` for the full design, call pattern and
forbidden pitfalls (never `block_on()` from inside a `ProxyHttp` async
method).

### Tests ported / added

- `test_gateway_init_with_defaults`
- `test_gateway_init_custom_config`
- `test_upstream_registry_resolves_healthy`
- `test_upstream_registry_all_unhealthy_returns_none`
- `test_upstream_registry_unknown_cluster`
- `test_upstream_registry_hot_reload`
- `test_hop_by_hop_list_does_not_strip_content_type`
- `test_hop_by_hop_list_contains_connection`
- `test_build_server_constructs_without_panic`
- **new** `test_pingora_gateway_accepts_filter_chain` — verifies the
  `PingoraGateway` accepts and stores a `Vec<Arc<dyn ForgeFilter>>` in
  registration order and produces a populated `RequestCtx` from
  `new_ctx`.

### Follow-up gates

| Gate | Issue   | Scope                                                |
|------|---------|------------------------------------------------------|
| M1   | #95–#100| Port router / CORS / JWT / FF / OTEL / VEIL filters  |
| M2   | #103    | Upstream selector, mTLS, CB, health, LB, retry       |
| M3   | #104    | Security-engine pipeline (SENTINEL → AI)             |
| M4   | #105    | gRPC-Web, WebSocket, compression, traffic-split      |
| M5   | #107    | Graceful drain + bridge cooperative shutdown         |

## Decision matrix

| Criterion | hyper 1.x (default) | Pingora 0.3 (feature) |
|---|---|---|
| Build dependency | Always compiled | Only with `--features pingora` |
| Connection pooling | Per-request new connection (hyper keep-alive via `Client`) | Built-in persistent pool per upstream peer |
| TLS | Bring-your-own (rustls wrapper) | Native BoringSSL/OpenSSL via `pingora-openssl` |
| Graceful restart | Manual (drain + restart) | First-class SIGUSR2 upgrade, zero-downtime |
| Worker model | Tokio green-threads, single executor | Multi-threaded I/O scheduler, one thread per CPU |
| HTTP/2 upstream | Yes (hyper-util) | Yes (HTTP/2 framing built-in) |
| Observability hooks | Custom (prometheus counters in handler) | `ProxyHttp` filter chain, native hook points |
| Maturity / production use | Cloudflare-vetted via Pingora OSS origin | Same origin, ARMAGEDDON-specific wiring is new |
| Compile time | Fast | Slower (C++ BoringSSL linkage) |
| Binary size increase | — | ~2-4 MB (estimated) |

Recommendation: run Pingora in shadow mode (traffic mirroring) for 2 sprints before promoting to default.

## Build instructions

### Default build (hyper 1.x, no Pingora)

```bash
cd /home/lyna/Documents/DEVELOPMENT-CLAUDE/INFRA/armageddon
cargo build -p armageddon-forge
cargo check -p armageddon-forge
```

### Pingora-enabled build

```bash
cargo build --release -p armageddon-forge --features pingora
cargo check -p armageddon-forge --features pingora
```

### Run benchmarks

Hyper only:
```bash
cargo bench -p armageddon-forge --bench proxy_compare
```

With Pingora filter-chain micro-bench:
```bash
cargo bench -p armageddon-forge --bench proxy_compare --features pingora
```

HTML reports are written to `target/criterion/`.

### Integration test

```bash
cargo test -p armageddon-forge --features pingora
```

## Architecture overview

```
armageddon-forge
├── src/proxy.rs              # hyper 1.x path (default, always compiled)
├── src/pingora_backend.rs    # Pingora path (cfg(feature = "pingora"))
│   ├── UpstreamRegistry      # thread-safe endpoint map, hot-reload capable
│   ├── PingoraGatewayConfig  # pool size, TLS, timeout
│   ├── PingoraGateway        # implements ProxyHttp
│   │   ├── upstream_peer()   # resolves healthy endpoint from registry
│   │   ├── request_filter()  # strips hop-by-hop, injects x-forge-id
│   │   └── response_filter() # appends x-forge-via: armageddon-pingora
│   └── build_server()        # wires gateway into Pingora Server
└── benches/proxy_compare.rs  # criterion: hyper vs pingora throughput
```

## Wiring the Pingora server at runtime

```rust
#[cfg(feature = "pingora")]
{
    use armageddon_forge::pingora_backend::{
        build_server, PingoraGateway, PingoraGatewayConfig, UpstreamRegistry,
    };
    use std::sync::Arc;

    let registry = Arc::new(UpstreamRegistry::new());
    // Push healthy endpoints (e.g. from xDS or config file).
    registry.update_cluster("api", endpoints);

    let gw = PingoraGateway::new(PingoraGatewayConfig::default(), registry);
    let mut server = build_server(gw, "0.0.0.0:8080")?;
    server.run_forever(); // blocks; SIGUSR2 triggers graceful restart
}
```

## Upstream registry hot-reload

```rust
// From the xDS RDS handler or an admin endpoint:
gateway.upstream_registry().update_cluster("api", new_endpoints);
```

The `RwLock` inside `UpstreamRegistry` ensures writer starvation cannot occur
during steady-state reads.

## Limitations

1. **Pingora runtime incompatibility with tokio benches.** Pingora uses its own
   multi-threaded scheduler and cannot be driven by a standard tokio `Runtime`
   inside Criterion. The `bench_pingora_filter_chain` bench therefore measures
   only the synchronous registry-lookup hot-path, not end-to-end I/O. Use `wrk`
   or `hey` against a live instance for realistic throughput numbers.

2. **TLS upstream.** Setting `PingoraGatewayConfig.upstream_tls = true` requires
   that the system OpenSSL (or BoringSSL via `pingora-boringssl`) is available at
   link time. The default `upstream_tls = false` path uses plain TCP only.

3. **HTTP/2 downstream is not yet wired.** `build_server` currently calls
   `proxy.add_tcp(addr)` (HTTP/1.1). HTTP/2 downstream requires
   `proxy.add_tls(addr, cert, key)` and a valid TLS certificate — planned for
   Vague 2 B2.

4. **No direct RESP3 / KAYA integration.** The Pingora path does not yet
   consult KAYA for JWT-cache or session-cache lookups. That wiring mirrors
   the hyper path's `jwt.rs` and is planned for Vague 2 B2.

5. **Feature flag is additive.** Enabling `pingora` does not disable or
   replace the hyper path. Both can coexist in the same binary; the operator
   chooses which `ForgeServer` instance to bind.

6. **`armageddon-bench` workspace member.** The workspace `Cargo.toml` lists
   `armageddon-bench` but its crate directory does not yet exist. This causes
   `cargo check` at the workspace level to fail until the crate is scaffolded.
   Run `cargo check -p armageddon-forge` (crate-scoped) to avoid this.

## Benchmark target

Target: +30% throughput vs hyper on 10 000 sequential GET requests.

Expected result: Pingora's persistent connection pool eliminates the
TCP-setup overhead present in the hyper `Client::build_http()` path
(which re-negotiates keep-alive per batch). At low concurrency the
difference is modest; at C=100 connections the pool benefit is
significant. Actual numbers depend on kernel TCP stack and NIC offload.

## License

AGPL-3.0-or-later. See `LICENSE` at the workspace root.
