// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Criterion benchmarks for KAYA core string commands.
//!
//! Covers SET, GET, INCR, MSET, MGET, DEL at various value sizes.
//! Run with:
//!   cargo bench --bench commands -p kaya-bench

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
// SET
// ---------------------------------------------------------------------------

fn bench_set(c: &mut Criterion) {
    let store = new_store();
    let mut group = c.benchmark_group("SET");
    group.measurement_time(std::time::Duration::from_secs(10));

    for &size in &[16usize, 256, 4096, 65536] {
        let value = vec![b'x'; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            let key = format!("set:key:{size}");
            b.iter(|| {
                store
                    .set(black_box(key.as_bytes()), black_box(&value), None)
                    .unwrap();
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// GET
// ---------------------------------------------------------------------------

fn bench_get(c: &mut Criterion) {
    let store = new_store();
    // Pre-populate 10 000 keys with 64-byte values.
    for i in 0..10_000u32 {
        let k = format!("get:key:{i:05}");
        store.set(k.as_bytes(), &[b'v'; 64], None).unwrap();
    }

    let mut group = c.benchmark_group("GET");
    group.measurement_time(std::time::Duration::from_secs(10));

    group.bench_function("hit", |b| {
        let mut i = 0u32;
        b.iter(|| {
            i = (i + 1) % 10_000;
            let k = format!("get:key:{i:05}");
            let _ = black_box(store.get(k.as_bytes()).unwrap());
        });
    });

    group.bench_function("miss", |b| {
        b.iter(|| {
            let _ = black_box(store.get(b"get:key:no-such-key").unwrap());
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// INCR
// ---------------------------------------------------------------------------

fn bench_incr(c: &mut Criterion) {
    let store = new_store();
    store.set(b"incr:counter", b"0", None).unwrap();
    c.bench_function("INCR", |b| {
        b.iter(|| {
            let _ = black_box(store.incr(b"incr:counter").unwrap());
        });
    });
}

// ---------------------------------------------------------------------------
// MSET
// ---------------------------------------------------------------------------

fn bench_mset(c: &mut Criterion) {
    let store = new_store();
    let mut group = c.benchmark_group("MSET");
    group.measurement_time(std::time::Duration::from_secs(10));

    for &n in &[10usize, 100, 1000] {
        let pairs_owned: Vec<(Vec<u8>, Vec<u8>)> = (0..n)
            .map(|i| (format!("mset:{n}:{i}").into_bytes(), vec![b'v'; 64]))
            .collect();
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let pairs: Vec<(&[u8], &[u8])> = pairs_owned
                    .iter()
                    .map(|(k, v)| (k.as_slice(), v.as_slice()))
                    .collect();
                store.mset(black_box(&pairs)).unwrap();
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// MGET
// ---------------------------------------------------------------------------

fn bench_mget(c: &mut Criterion) {
    let store = new_store();
    let mut group = c.benchmark_group("MGET");
    group.measurement_time(std::time::Duration::from_secs(10));

    for &n in &[10usize, 100, 1000] {
        let keys_owned: Vec<Vec<u8>> = (0..n)
            .map(|i| format!("mget:{n}:{i}").into_bytes())
            .collect();
        for k in &keys_owned {
            store.set(k, &[b'v'; 64], None).unwrap();
        }
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let refs: Vec<&[u8]> = keys_owned.iter().map(|k| k.as_slice()).collect();
                let _ = black_box(store.mget(&refs));
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// DEL
// ---------------------------------------------------------------------------

fn bench_del(c: &mut Criterion) {
    let store = new_store();
    let mut group = c.benchmark_group("DEL");
    group.measurement_time(std::time::Duration::from_secs(10));

    for &n in &[1usize, 10, 100] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                // Re-insert before every deletion to keep the measurement symmetric.
                for i in 0..n {
                    let k = format!("del:{n}:{i}").into_bytes();
                    store.set(&k, b"v", None).unwrap();
                }
                let keys_owned: Vec<Vec<u8>> = (0..n)
                    .map(|i| format!("del:{n}:{i}").into_bytes())
                    .collect();
                let refs: Vec<&[u8]> = keys_owned.iter().map(|k| k.as_slice()).collect();
                let _ = black_box(store.del(&refs));
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
    bench_set,
    bench_get,
    bench_incr,
    bench_mset,
    bench_mget,
    bench_del
);
criterion_main!(benches);
