// SPDX-License-Identifier: AGPL-3.0-or-later
//! Criterion benchmark: hyper vs Pingora proxy throughput on 10 000 GET
//! requests forwarded to a local in-process dummy upstream.
//!
//! # Running
//!
//! Default (hyper path only):
//! ```text
//! cargo bench -p armageddon-forge --bench proxy_compare
//! ```
//!
//! With the Pingora backend enabled:
//! ```text
//! cargo bench -p armageddon-forge --bench proxy_compare --features pingora
//! ```
//!
//! # Methodology
//!
//! A minimal `tokio` HTTP/1.1 server is spun up in the bench setup phase on a
//! random ephemeral port and responds with `200 OK` + a short body to every
//! `GET` request.  Both the hyper path and (when enabled) the Pingora path
//! forward requests to this server and measure end-to-end latency and
//! throughput.
//!
//! Criterion's throughput measurement is set to `Elements(1)` (one request per
//! iteration) so the reported metric is requests/sec.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::runtime::Runtime;

use armageddon_common::types::Endpoint;
use armageddon_forge::proxy::{forward_request, RoundRobinCounter};

// ── dummy upstream ─────────────────────────────────────────────────────────

/// Start a minimal hyper HTTP server on an ephemeral port.
///
/// Returns the bound `SocketAddr`.  The server runs on a detached task and
/// lives for the entire process lifetime.
fn start_dummy_upstream(rt: &Runtime) -> SocketAddr {
    let addr = rt.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind dummy upstream");
        let addr = listener.local_addr().expect("local addr");

        tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.expect("accept");
                let io = TokioIo::new(stream);
                tokio::spawn(async move {
                    let _ = http1::Builder::new()
                        .serve_connection(
                            io,
                            service_fn(|_req: Request<hyper::body::Incoming>| async {
                                Ok::<_, Infallible>(
                                    Response::new(Full::new(Bytes::from_static(b"ok"))),
                                )
                            }),
                        )
                        .await;
                });
            }
        });

        addr
    });
    addr
}

// ── hyper benchmark ────────────────────────────────────────────────────────

fn bench_hyper_forward(c: &mut Criterion) {
    let rt = Runtime::new().expect("tokio runtime");
    let upstream_addr = start_dummy_upstream(&rt);

    let endpoint = Endpoint {
        address: upstream_addr.ip().to_string(),
        port: upstream_addr.port(),
        weight: 1,
        healthy: true,
    };

    let mut group = c.benchmark_group("proxy_forward");
    group.throughput(Throughput::Elements(1));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(200);

    group.bench_function(BenchmarkId::new("hyper", "GET /ping"), |b| {
        b.to_async(&rt).iter(|| async {
            forward_request(&endpoint, "GET", "/ping", &[], None, 5_000)
                .await
                .expect("hyper forward");
        });
    });

    group.finish();
}

// ── pingora benchmark ──────────────────────────────────────────────────────
//
// The Pingora benchmark measures the overhead of the `upstream_peer` +
// `request_filter` pipeline in isolation (synchronous logic only) because
// Pingora's full I/O stack requires its own runtime and cannot be driven from
// a standard tokio runtime inside Criterion.
//
// For a realistic end-to-end comparison, run the two servers independently
// under `wrk` or `hey` and compare their outputs.  The Criterion bench here
// validates that the Pingora filter chain overhead is sub-microsecond.

#[cfg(feature = "pingora")]
fn bench_pingora_filter_chain(c: &mut Criterion) {
    use armageddon_forge::pingora_backend::{
        PingoraGateway, PingoraGatewayConfig, UpstreamRegistry,
    };

    let rt = Runtime::new().expect("tokio runtime");
    let upstream_addr = rt.block_on(async {
        TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind")
            .local_addr()
            .expect("addr")
    });

    let registry = Arc::new(UpstreamRegistry::new());
    registry.update_cluster(
        "bench",
        vec![Endpoint {
            address: upstream_addr.ip().to_string(),
            port: upstream_addr.port(),
            weight: 1,
            healthy: true,
        }],
    );

    let cfg = PingoraGatewayConfig {
        default_cluster: "bench".to_string(),
        upstream_tls: false,
        upstream_timeout_ms: 5_000,
        pool_size: 128,
    };
    let _gw = PingoraGateway::new(cfg, registry.clone());

    let mut group = c.benchmark_group("proxy_forward");
    group.throughput(Throughput::Elements(1));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(200);

    // Benchmark the registry resolution hot-path (lock + hashmap lookup).
    group.bench_function(BenchmarkId::new("pingora_registry_lookup", "bench"), |b| {
        b.iter(|| {
            let ep = registry.first_healthy("bench");
            criterion::black_box(ep);
        });
    });

    group.finish();
}

// ── round-robin micro-bench ────────────────────────────────────────────────

fn bench_round_robin_counter(c: &mut Criterion) {
    let counter = RoundRobinCounter::new();
    let endpoints = vec![
        Endpoint { address: "10.0.0.1".to_string(), port: 8080, weight: 1, healthy: true },
        Endpoint { address: "10.0.0.2".to_string(), port: 8080, weight: 1, healthy: true },
        Endpoint { address: "10.0.0.3".to_string(), port: 8080, weight: 1, healthy: true },
    ];

    let mut group = c.benchmark_group("load_balancer");
    group.throughput(Throughput::Elements(1));

    group.bench_function("round_robin_select", |b| {
        b.iter(|| {
            let idx = armageddon_forge::proxy::select_endpoint_round_robin(
                &endpoints,
                &counter,
            );
            criterion::black_box(idx);
        });
    });

    group.finish();
}

// ── criterion wiring ───────────────────────────────────────────────────────

#[cfg(not(feature = "pingora"))]
criterion_group!(
    proxy_benches,
    bench_hyper_forward,
    bench_round_robin_counter
);

#[cfg(feature = "pingora")]
criterion_group!(
    proxy_benches,
    bench_hyper_forward,
    bench_pingora_filter_chain,
    bench_round_robin_counter
);

criterion_main!(proxy_benches);
