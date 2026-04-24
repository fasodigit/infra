# FORGE-Pingora runtime bridge

Design notes for `src/pingora/runtime.rs`.

Tracker: [GitHub #108](https://github.com/faso-digitalisation/armageddon/issues/108).
Gate: M0 (#101).

---

## 1. Why a bridge exists

Pingora (Cloudflare) ships with its own I/O scheduler. That scheduler is
**not** a tokio runtime — it drives the network socket ready-state + Pingora
worker tasks via a custom future executor that does **not** register a
`tokio::runtime::Runtime` thread-local.

Consequently, any code path that expects a tokio reactor to exist (because it
uses `tokio::time::sleep`, `tokio::net::TcpStream`, `tokio::sync::Mutex::lock`,
`redis::aio::ConnectionManager`, …) will panic or deadlock if invoked from
inside a Pingora hook.

The FASO gateway must embed:

- **Tokio-native security engines** — `armageddon-sentinel`, `-arbiter`,
  `-oracle`, `-aegis`, `-nexus`, `-veil`, `-wasm`, `-ai`. All use
  `tokio::spawn`, `tokio::sync::*`, and `tokio::time::*`.
- **Tokio-native clients** — KAYA (`redis` crate, tokio-comp), xDS gRPC
  (`tonic`), SPIFFE cert fetcher (`armageddon-mesh`, `spiffe` crate),
  GrowthBook SSE (hyper client), OTEL exporter (OTLP over tonic).

None of those are portable to Pingora's scheduler without massive rewrites.

## 2. Chosen design — Option A: isolated tokio runtime

Pingora stays the **main process**: it owns the listener sockets, the TLS
accept loop, the HTTP/1/2 framing, the graceful-restart (SIGUSR2) state
machine. An **isolated** multi-threaded tokio runtime runs on a dedicated OS
thread spawned on first access.

```
                      ┌──────────────────────────────────┐
                      │  Pingora main process            │
                      │  (pingora-proxy scheduler)       │
┌──────────────┐      │                                  │
│ kernel epoll │──────► listener ─► HttpProxy ─► ctx     │
└──────────────┘      │                  │               │
                      │          spawn(bridge)           │
                      │                  │               │
                      └─────────────┬────┴───────────────┘
                                    ▼
                      ┌──────────────────────────────────┐
                      │  OS thread: armageddon-forge-    │
                      │  bridge                          │
                      │  ┌────────────────────────────┐  │
                      │  │ tokio multi-thread rt      │  │
                      │  │  (worker_threads = N)      │  │
                      │  │  park: pending::<()>()     │  │
                      │  └────────────────────────────┘  │
                      └──────────────────────────────────┘
```

Trade-offs considered and rejected:

| Option                                    | Why rejected                                            |
|-------------------------------------------|---------------------------------------------------------|
| B. Replace Pingora scheduler with tokio   | Loses graceful restart, zero-copy connect, TLS tuning   |
| C. Rewrite engines without tokio          | ~40 kLOC rewrite; 8 crates; delays all M1–M5 work       |
| D. Port engines to `async-std` / `smol`   | Double runtime cost, ecosystem half as deep             |

## 3. Exposed API

```rust
pub fn tokio_handle() -> tokio::runtime::Handle;
```

Lazy-initialised via `std::sync::OnceLock` (stable since Rust 1.70). First
call spawns the dedicated OS thread and hands back a cloned
`tokio::runtime::Handle`. Subsequent calls return a clone of the cached
handle; all callers share one runtime.

The runtime lives for the lifetime of the process — the worker thread parks
inside `rt.block_on(std::future::pending::<()>())` so the runtime itself is
never dropped.

Thread count defaults to **4** but can be overridden by the operator via
`ARMAGEDDON_FORGE_BRIDGE_THREADS`. Security engines are bursty CPU but
low-duration per request; empirically 4 cores suffice up to ~20 kRPS.

## 4. Call pattern — the ONLY pattern

```rust
async fn my_proxy_hook(
    session: &mut pingora_proxy::Session,
    ctx: &mut crate::pingora::ctx::RequestCtx,
) -> pingora_core::Result<bool> {
    let handle = crate::pingora::runtime::tokio_handle();

    // 1. Spawn the tokio-native work on the bridge.
    let join: tokio::task::JoinHandle<u32> = handle.spawn(async move {
        armageddon_sentinel::score_request(/* borrowed bits */).await
    });

    // 2. Await the JoinHandle from *this* (Pingora) async context.
    //    `JoinHandle: Future<Output = Result<T, JoinError>>` is Send + 'static,
    //    so Pingora's executor can poll it safely.
    let score = join.await.unwrap_or_default();

    ctx.waf_score = score as f32;
    Ok(false)
}
```

## 5. Forbidden patterns

### 5.1. `block_on` from inside a Pingora hook

```rust
// DO NOT DO THIS:
handle.block_on(async { /* work */ });
```

The hook is already on an executor (Pingora). Calling `block_on` from inside
an async task of a different runtime may deadlock because:

- `block_on` parks the current thread waiting for the future, but
- the future cannot make progress if its I/O wakers land on the Pingora
  reactor, not the tokio one.

Always use `spawn` + `await JoinHandle`.

### 5.2. Cross-runtime borrows

```rust
// DO NOT DO THIS:
let ctx_ref = &mut *ctx;
handle.spawn(async move { ctx_ref.user_id = Some("x".into()); });
```

`ctx` lives on the Pingora stack; the bridge can't borrow it. Instead, pass
owned copies of the needed fields and merge the results back after
`JoinHandle::await`.

### 5.3. Blocking syscalls on bridge workers without `spawn_blocking`

Same rule as any well-behaved tokio runtime — file I/O, DNS, JNI calls go
via `handle.spawn_blocking(...)` to avoid starving the worker pool.

## 6. Observability

Every bridge-spawned future should be wrapped in a
`tracing::Instrument::in_current_span()` call at the call-site so the trace
context (W3C `traceparent`) propagates into engine logs. A follow-up in M3
#104 adds a bridge-level Prometheus histogram for `bridge_task_duration_ms`
and `bridge_queue_depth`.

## 7. Shutdown

For M0 the bridge leaks on process exit (the OS reclaims everything). When
Pingora triggers SIGUSR2 graceful-restart, the bridge sends an
inherited-socket handshake but its internal task queue is **not** drained —
tasks in flight may be cancelled mid-flight. M5 (#107) adds a cooperative
drain channel.

## 8. References

- Pingora design doc: https://blog.cloudflare.com/pingora-open-source/
- Master tracker: [#108](https://github.com/faso-digitalisation/armageddon/issues/108)
- M0 gate: [#101](https://github.com/faso-digitalisation/armageddon/issues/101)
- Hyper path (reference): `src/proxy.rs`, `src/jwt.rs`
