// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Criterion benchmarks for KAYA Streams (XADD / XREAD / XREADGROUP).
//!
//! Each benchmark group keeps measurement time under 15 s to stay well within
//! the 60 s-per-bench budget.
//!
//! Run with:
//!   cargo bench --bench streams -p kaya-bench

use bytes::Bytes;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use kaya_streams::{StreamId, StreamManager};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fields(n: usize) -> Vec<(Bytes, Bytes)> {
    (0..n)
        .map(|i| {
            (
                Bytes::from(format!("field:{i}")),
                Bytes::copy_from_slice(b"value-payload-bench"),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// XADD
// ---------------------------------------------------------------------------

fn bench_xadd(c: &mut Criterion) {
    let mut group = c.benchmark_group("XADD");
    group.measurement_time(std::time::Duration::from_secs(10));

    for &n_fields in &[1usize, 5, 20] {
        let f = fields(n_fields);
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("fields", n_fields),
            &n_fields,
            |b, _| {
                let mgr = StreamManager::default();
                b.iter(|| {
                    let _ = black_box(
                        mgr.xadd(
                            black_box("bench:stream"),
                            Some("*"),
                            black_box(f.clone()),
                        )
                        .unwrap(),
                    );
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// XADD bulk — measure sustained throughput over many appends
// ---------------------------------------------------------------------------

fn bench_xadd_bulk(c: &mut Criterion) {
    let mut group = c.benchmark_group("XADD/bulk");
    group.measurement_time(std::time::Duration::from_secs(10));

    for &n in &[100usize, 1_000, 10_000] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let mgr = StreamManager::default();
                let f = fields(3);
                for _ in 0..n {
                    let _ = mgr
                        .xadd("bench:bulk", Some("*"), black_box(f.clone()))
                        .unwrap();
                }
                black_box(mgr.stream_count())
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// XREAD — read N entries from a populated stream
// ---------------------------------------------------------------------------

fn bench_xread(c: &mut Criterion) {
    let mut group = c.benchmark_group("XREAD");
    group.measurement_time(std::time::Duration::from_secs(10));

    for &n_entries in &[10usize, 100, 1_000] {
        let mgr = StreamManager::default();
        let f = fields(3);
        for _ in 0..n_entries {
            mgr.xadd("bench:read", Some("*"), f.clone()).unwrap();
        }

        group.throughput(Throughput::Elements(n_entries as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(n_entries),
            &n_entries,
            |b, _| {
                b.iter(|| {
                    let _ = black_box(
                        mgr.xread(
                            &[("bench:read".to_string(), StreamId::ZERO)],
                            None,
                        )
                        .unwrap(),
                    );
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// XREADGROUP — consumer group read
// ---------------------------------------------------------------------------

fn bench_xreadgroup(c: &mut Criterion) {
    let mut group = c.benchmark_group("XREADGROUP");
    group.measurement_time(std::time::Duration::from_secs(10));
    group.sample_size(50);

    for &n_entries in &[10usize, 100, 1_000] {
        // Fresh manager per parameter to avoid PEL growing unbounded.
        let mgr = StreamManager::default();
        let f = fields(3);
        for _ in 0..n_entries {
            mgr.xadd("bench:grp", Some("*"), f.clone()).unwrap();
        }
        mgr.xgroup_create("bench:grp", "grp1", StreamId::ZERO)
            .unwrap();

        group.throughput(Throughput::Elements(n_entries as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(n_entries),
            &n_entries,
            |b, _| {
                // Re-create the manager each iter so consumer sees pending messages.
                b.iter(|| {
                    let m2 = StreamManager::default();
                    let f2 = fields(3);
                    for _ in 0..n_entries {
                        m2.xadd("bench:grp2", Some("*"), f2.clone()).unwrap();
                    }
                    m2.xgroup_create("bench:grp2", "grp1", StreamId::ZERO)
                        .unwrap();
                    let _ = black_box(
                        m2.xreadgroup("bench:grp2", "grp1", "consumer-1", None)
                            .unwrap(),
                    );
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_xadd,
    bench_xadd_bulk,
    bench_xread,
    bench_xreadgroup,
);
criterion_main!(benches);
