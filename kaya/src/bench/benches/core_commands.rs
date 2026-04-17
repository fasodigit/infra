// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// Criterion benchmarks for KAYA core commands. Run with `cargo bench -p kaya-bench`.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use kaya_compress::CompressConfig;
use kaya_store::{Store, StoreConfig};

fn new_store() -> Store {
    Store::new(StoreConfig::default(), CompressConfig::default())
}

fn bench_set(c: &mut Criterion) {
    let store = new_store();
    let mut group = c.benchmark_group("set");
    for &size in &[10usize, 256, 4096, 65536] {
        let value = vec![b'a'; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let key = format!("k{}", black_box(size));
                store.set(key.as_bytes(), &value, None).unwrap();
            });
        });
    }
    group.finish();
}

fn bench_get(c: &mut Criterion) {
    let store = new_store();
    for i in 0..10_000u32 {
        let k = format!("bench:get:{i}");
        store.set(k.as_bytes(), &[b'v'; 64], None).unwrap();
    }
    let mut group = c.benchmark_group("get");
    group.bench_function("hit", |b| {
        let mut i = 0u32;
        b.iter(|| {
            i = (i + 1) % 10_000;
            let k = format!("bench:get:{i}");
            let _ = black_box(store.get(k.as_bytes()).unwrap());
        });
    });
    group.bench_function("miss", |b| {
        b.iter(|| {
            let _ = black_box(store.get(b"nope").unwrap());
        });
    });
    group.finish();
}

fn bench_del(c: &mut Criterion) {
    let store = new_store();
    c.bench_function("del_batch_10", |b| {
        b.iter(|| {
            for i in 0..10 {
                let k = format!("del:{i}");
                store.set(k.as_bytes(), b"v", None).unwrap();
            }
            let keys: Vec<Vec<u8>> = (0..10).map(|i| format!("del:{i}").into_bytes()).collect();
            let refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
            let _ = black_box(store.del(&refs));
        });
    });
}

fn bench_mget(c: &mut Criterion) {
    let store = new_store();
    for i in 0..100u32 {
        let k = format!("mg:{i}");
        store.set(k.as_bytes(), &[b'x'; 128], None).unwrap();
    }
    let keys: Vec<Vec<u8>> = (0..100).map(|i| format!("mg:{i}").into_bytes()).collect();
    let refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
    c.bench_function("mget_100", |b| {
        b.iter(|| {
            let _ = black_box(store.mget(&refs));
        });
    });
}

fn bench_incr(c: &mut Criterion) {
    let store = new_store();
    c.bench_function("incr", |b| {
        store.set(b"counter", b"0", None).unwrap();
        b.iter(|| {
            let _ = black_box(store.incr(b"counter").unwrap());
        });
    });
}

criterion_group!(benches, bench_set, bench_get, bench_del, bench_mget, bench_incr);
criterion_main!(benches);
