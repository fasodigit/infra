// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//! Benchmark: W3C traceparent extract + inject overhead.
//!
//! # Goal
//!
//! Verify that the combined `extract_context` + `inject_context` round-trip
//! adds less than **2 %** overhead compared to the baseline of building a plain
//! [`http::HeaderMap`] with no OTel processing.
//!
//! # Running
//!
//! ```text
//! cargo bench -p armageddon-bench --bench otel_propagation
//! ```
//!
//! # Interpretation
//!
//! Compare the `otel/extract_inject` result against `baseline/header_clone`.
//! The OTel path should be within ≤ 2 % of the baseline on modern hardware.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use http::HeaderMap;

use armageddon_oracle::otel_propagation::{extract_context, inject_context};

// ---------------------------------------------------------------------------
// Fixture: a realistic W3C traceparent header
// ---------------------------------------------------------------------------

const TRACEPARENT: &str =
    "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

const BAGGAGE: &str =
    "x-faso-tenant=bf-prod,x-faso-user-id=user-42,x-request-id=req-abc123";

// ---------------------------------------------------------------------------
// Baseline: clone a HeaderMap with equivalent headers (no OTel work)
// ---------------------------------------------------------------------------

fn bench_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline");
    group.throughput(Throughput::Elements(1));

    group.bench_function("header_clone", |b| {
        let mut src = HeaderMap::new();
        src.insert("traceparent", TRACEPARENT.parse().unwrap());
        src.insert("baggage", BAGGAGE.parse().unwrap());

        b.iter(|| {
            let cloned = src.clone();
            criterion::black_box(cloned)
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// OTel extract_context benchmark
// ---------------------------------------------------------------------------

fn bench_extract(c: &mut Criterion) {
    let mut group = c.benchmark_group("otel");
    group.throughput(Throughput::Elements(1));

    let mut headers = HeaderMap::new();
    headers.insert("traceparent", TRACEPARENT.parse().unwrap());
    headers.insert("baggage", BAGGAGE.parse().unwrap());

    group.bench_function("extract_context", |b| {
        b.iter(|| {
            let ctx = extract_context(&headers);
            criterion::black_box(ctx)
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// OTel extract + inject round-trip benchmark (critical path)
// ---------------------------------------------------------------------------

fn bench_extract_inject(c: &mut Criterion) {
    let mut group = c.benchmark_group("otel");
    group.throughput(Throughput::Elements(1));

    let mut incoming = HeaderMap::new();
    incoming.insert("traceparent", TRACEPARENT.parse().unwrap());
    incoming.insert("baggage", BAGGAGE.parse().unwrap());

    group.bench_function("extract_inject", |b| {
        b.iter(|| {
            // Extract from incoming headers.
            let ctx = extract_context(&incoming);

            // Inject into outgoing headers.
            let mut outgoing = HeaderMap::new();
            inject_context(&ctx, &mut outgoing);

            criterion::black_box(outgoing)
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// OTel forge middleware: begin_request_trace round-trip
// ---------------------------------------------------------------------------

fn bench_begin_request_trace(c: &mut Criterion) {
    use armageddon_forge::otel_middleware::begin_request_trace;

    let mut group = c.benchmark_group("otel");
    group.throughput(Throughput::Elements(1));

    let mut incoming = HeaderMap::new();
    incoming.insert("traceparent", TRACEPARENT.parse().unwrap());
    incoming.insert("baggage", BAGGAGE.parse().unwrap());

    group.bench_function("begin_request_trace", |b| {
        b.iter(|| {
            let guard = begin_request_trace(
                &incoming,
                "POST",
                "/api/graphql",
                "dgs-gateway",
            );
            criterion::black_box(guard)
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion groups
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_baseline,
    bench_extract,
    bench_extract_inject,
    bench_begin_request_trace,
);
criterion_main!(benches);
