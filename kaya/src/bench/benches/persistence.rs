// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Criterion benchmarks for KAYA persistence layer.
//!
//! Strategy: the WAL writer is async and requires real I/O, which Criterion
//! cannot drive efficiently in its synchronous harness. Instead, we benchmark
//! the hot path components in isolation:
//!
//!   1. `WalRecord::encode` — serialises a record into a `BytesMut` buffer.
//!      This is the CPU-bound part that runs on every write; the OS async I/O
//!      work is external.
//!
//!   2. "snapshot_scan" — iterates every key across all shards, simulating
//!      the read side of a `BGSAVE` (serialising entries into a byte buffer
//!      without actual disk I/O).
//!
//! Run with:
//!   cargo bench --bench persistence -p kaya-bench

use bytes::{Bytes, BytesMut};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use kaya_compress::CompressConfig;
use kaya_store::persistence::wal::{WalOp, WalRecord};
use kaya_store::{Store, StoreConfig};

// ---------------------------------------------------------------------------
// WAL record encode
// ---------------------------------------------------------------------------

fn bench_wal_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("WAL/encode");
    group.measurement_time(std::time::Duration::from_secs(10));

    for &val_size in &[16usize, 256, 4096] {
        let record = WalRecord {
            op: WalOp::Set,
            shard_id: 0,
            logical_ts: 42,
            key: Bytes::copy_from_slice(b"bench:wal:key"),
            value: Bytes::from(vec![b'v'; val_size]),
            extra: 0,
        };
        let mut buf = BytesMut::with_capacity(256 + val_size);

        group.throughput(Throughput::Bytes(val_size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(val_size), &val_size, |b, _| {
            b.iter(|| {
                buf.clear();
                black_box(&record).encode(black_box(&mut buf));
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// WAL batch encode — simulates a pipeline of N consecutive records
// ---------------------------------------------------------------------------

fn bench_wal_encode_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("WAL/encode_batch");
    group.measurement_time(std::time::Duration::from_secs(10));

    for &n in &[100usize, 1_000, 10_000] {
        let records: Vec<WalRecord> = (0..n)
            .map(|i| WalRecord {
                op: WalOp::Set,
                shard_id: (i % 64) as u32,
                logical_ts: i as u64,
                key: Bytes::copy_from_slice(format!("k:{i:08}").as_bytes()),
                value: Bytes::copy_from_slice(b"value-payload-64-bytes-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"),
                extra: 0,
            })
            .collect();

        let mut buf = BytesMut::with_capacity(128 * n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                buf.clear();
                for rec in black_box(&records) {
                    rec.encode(black_box(&mut buf));
                }
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Snapshot scan — iterate all shards (simulates BGSAVE read pass)
// ---------------------------------------------------------------------------

fn bench_snapshot_scan(c: &mut Criterion) {
    let store = Store::new(StoreConfig::default(), CompressConfig::default());

    // Pre-populate 100 000 keys.
    for i in 0..100_000u64 {
        let k = format!("snap:{i:08}").into_bytes();
        store.set(&k, &[b'x'; 64], None).unwrap();
    }

    let mut group = c.benchmark_group("Snapshot/scan");
    group.measurement_time(std::time::Duration::from_secs(15));
    group.sample_size(20); // I/O-adjacent, keep low to stay under 60 s

    group.bench_function("full_scan_100k", |b| {
        b.iter(|| {
            let mut count = 0usize;
            for shard_idx in 0..store.num_shards() {
                let shard = store.shard_at(shard_idx);
                for entry in shard.iter_kv() {
                    // Simulate serialising the key + raw (compressed) value bytes.
                    // `entry` is dashmap::RefMulti<Vec<u8>, Entry>; `.value()`
                    // returns `&Entry`, then `.value` is the `Bytes` field.
                    let _ = black_box(entry.key().len() + entry.value().value.len());
                    count += 1;
                }
            }
            black_box(count)
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_wal_encode,
    bench_wal_encode_batch,
    bench_snapshot_scan,
);
criterion_main!(benches);
