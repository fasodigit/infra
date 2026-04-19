// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Criterion benchmarks for KAYA collection data types.
//!
//! Covers: SADD, SMEMBERS, SCARD, ZADD, ZRANGE, ZRANGEBYSCORE.
//!
//! Note: HSET/HGET and LPUSH/LRANGE are not yet exposed on the public
//! `Store` API (tracked in TODO: hash + list shard methods). These benches
//! will be extended once those commands land.
//!
//! Run with:
//!   cargo bench --bench datatypes -p kaya-bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use kaya_compress::CompressConfig;
use kaya_store::{Store, StoreConfig};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn new_store() -> Store {
    Store::new(StoreConfig::default(), CompressConfig::default())
}

// ---------------------------------------------------------------------------
// SADD
// ---------------------------------------------------------------------------

fn bench_sadd(c: &mut Criterion) {
    let store = new_store();
    let mut group = c.benchmark_group("SADD");
    group.measurement_time(std::time::Duration::from_secs(10));

    for &n in &[1usize, 10, 100] {
        let members_owned: Vec<Vec<u8>> = (0..n)
            .map(|i| format!("member:{i:04}").into_bytes())
            .collect();
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let members: Vec<&[u8]> =
                    members_owned.iter().map(|m| m.as_slice()).collect();
                let _ = black_box(store.sadd(b"sadd:key", &members).unwrap());
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// SMEMBERS
// ---------------------------------------------------------------------------

fn bench_smembers(c: &mut Criterion) {
    let store = new_store();
    let mut group = c.benchmark_group("SMEMBERS");
    group.measurement_time(std::time::Duration::from_secs(10));

    for &n in &[10usize, 1_000, 10_000] {
        let key = format!("smembers:key:{n}");
        let members_owned: Vec<Vec<u8>> = (0..n)
            .map(|i| format!("m:{i:06}").into_bytes())
            .collect();
        let refs: Vec<&[u8]> = members_owned.iter().map(|m| m.as_slice()).collect();
        store.sadd(key.as_bytes(), &refs).unwrap();

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let _ = black_box(store.smembers(key.as_bytes()));
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// SCARD
// ---------------------------------------------------------------------------

fn bench_scard(c: &mut Criterion) {
    let store = new_store();
    let members_owned: Vec<Vec<u8>> = (0..1000)
        .map(|i| format!("m:{i:04}").into_bytes())
        .collect();
    let refs: Vec<&[u8]> = members_owned.iter().map(|m| m.as_slice()).collect();
    store.sadd(b"scard:key", &refs).unwrap();

    c.bench_function("SCARD", |b| {
        b.iter(|| {
            let _ = black_box(store.scard(b"scard:key"));
        });
    });
}

// ---------------------------------------------------------------------------
// ZADD
// ---------------------------------------------------------------------------

fn bench_zadd(c: &mut Criterion) {
    let store = new_store();
    let mut group = c.benchmark_group("ZADD");
    group.measurement_time(std::time::Duration::from_secs(10));

    for &n in &[1usize, 10, 100] {
        let members_owned: Vec<Vec<u8>> = (0..n)
            .map(|i| format!("zm:{i:04}").into_bytes())
            .collect();
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let pairs: Vec<(f64, &[u8])> = members_owned
                    .iter()
                    .enumerate()
                    .map(|(i, m)| (i as f64, m.as_slice()))
                    .collect();
                let _ = black_box(store.zadd(b"zadd:key", &pairs));
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// ZRANGE
// ---------------------------------------------------------------------------

fn bench_zrange(c: &mut Criterion) {
    let store = new_store();
    let mut group = c.benchmark_group("ZRANGE");
    group.measurement_time(std::time::Duration::from_secs(10));

    for &n in &[100usize, 1_000, 10_000] {
        let key = format!("zrange:key:{n}");
        let members_owned: Vec<Vec<u8>> = (0..n)
            .map(|i| format!("zm:{i:06}").into_bytes())
            .collect();
        let pairs: Vec<(f64, &[u8])> = members_owned
            .iter()
            .enumerate()
            .map(|(i, m)| (i as f64, m.as_slice()))
            .collect();
        let _ = store.zadd(key.as_bytes(), &pairs);

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let _ = black_box(store.zrange(key.as_bytes(), 0, -1));
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// ZRANGEBYSCORE
// ---------------------------------------------------------------------------

fn bench_zrangebyscore(c: &mut Criterion) {
    let store = new_store();
    let n = 10_000usize;
    let members_owned: Vec<Vec<u8>> = (0..n)
        .map(|i| format!("zbs:{i:06}").into_bytes())
        .collect();
    let pairs: Vec<(f64, &[u8])> = members_owned
        .iter()
        .enumerate()
        .map(|(i, m)| (i as f64, m.as_slice()))
        .collect();
    let _ = store.zadd(b"zrangebyscore:key", &pairs);

    let mut group = c.benchmark_group("ZRANGEBYSCORE");
    group.measurement_time(std::time::Duration::from_secs(10));

    for &(lo, hi, label) in &[
        (0.0f64, 100.0, "top_1pct"),
        (0.0f64, 1000.0, "top_10pct"),
        (0.0f64, 9999.0, "all"),
    ] {
        group.bench_with_input(BenchmarkId::new("score_range", label), &(lo, hi), |b, &(lo, hi)| {
            b.iter(|| {
                let _ = black_box(
                    store.zrangebyscore(b"zrangebyscore:key", lo, hi, None),
                );
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_sadd,
    bench_smembers,
    bench_scard,
    bench_zadd,
    bench_zrange,
    bench_zrangebyscore
);
criterion_main!(benches);
