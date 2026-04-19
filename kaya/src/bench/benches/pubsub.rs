// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Criterion benchmarks for KAYA Pub/Sub fanout.
//!
//! Measures PUBLISH throughput as the subscriber count grows from 1 to 1 000.
//! All channels are bounded; slow-subscriber drop paths are not exercised here
//! (we drain all receivers between iterations).
//!
//! `PubSubBroker::publish` is async; each iteration drives a single-threaded
//! Tokio runtime via `rt.block_on(...)`.
//!
//! Run with:
//!   cargo bench --bench pubsub -p kaya-bench

use bytes::Bytes;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use kaya_pubsub::{PubSubBroker, DEFAULT_SUBSCRIBER_CAPACITY};
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_broker_with_n_subscribers(
    n: usize,
) -> (
    PubSubBroker,
    Vec<mpsc::Receiver<kaya_pubsub::PubSubMessage>>,
) {
    let broker = PubSubBroker::new();
    let channel: Bytes = Bytes::from_static(b"bench:pubsub:channel");
    let mut receivers = Vec::with_capacity(n);

    for _ in 0..n {
        let (tx, rx) = mpsc::channel(DEFAULT_SUBSCRIBER_CAPACITY);
        broker.subscribe(channel.clone(), tx);
        receivers.push(rx);
    }

    (broker, receivers)
}

// ---------------------------------------------------------------------------
// PUBLISH throughput — vary subscriber count
// ---------------------------------------------------------------------------

fn bench_publish_fanout(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("tokio rt");

    let mut group = c.benchmark_group("PUBLISH/fanout");
    group.measurement_time(std::time::Duration::from_secs(10));

    for &n_subs in &[1usize, 10, 100, 1_000] {
        let (broker, mut receivers) = build_broker_with_n_subscribers(n_subs);
        let payload: Bytes = Bytes::from_static(b"hello-from-publisher-bench-payload");

        group.throughput(Throughput::Elements(n_subs as u64));
        group.bench_with_input(
            BenchmarkId::new("subscribers", n_subs),
            &n_subs,
            |b, _| {
                b.iter(|| {
                    let delivered = rt.block_on(async {
                        broker
                            .publish(
                                black_box(b"bench:pubsub:channel"),
                                black_box(payload.clone()),
                            )
                            .await
                    });
                    // Drain each receiver to prevent bounded channels filling up.
                    for rx in receivers.iter_mut() {
                        let _ = rx.try_recv();
                    }
                    black_box(delivered)
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// PUBLISH latency — single subscriber, varying payload size
// ---------------------------------------------------------------------------

fn bench_publish_payload_size(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("tokio rt");

    let mut group = c.benchmark_group("PUBLISH/payload_size");
    group.measurement_time(std::time::Duration::from_secs(10));

    for &size in &[16usize, 256, 4096, 65536] {
        let (broker, mut receivers) = build_broker_with_n_subscribers(1);
        let payload = Bytes::from(vec![b'p'; size]);

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let delivered = rt.block_on(async {
                    broker
                        .publish(
                            black_box(b"bench:pubsub:payload"),
                            black_box(payload.clone()),
                        )
                        .await
                });
                for rx in receivers.iter_mut() {
                    let _ = rx.try_recv();
                }
                black_box(delivered)
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// SUBSCRIBE + UNSUBSCRIBE roundtrip overhead
// ---------------------------------------------------------------------------

fn bench_subscribe_unsubscribe(c: &mut Criterion) {
    let broker = PubSubBroker::new();

    c.bench_function("SUBSCRIBE/UNSUBSCRIBE_roundtrip", |b| {
        b.iter(|| {
            let (tx, _rx) = mpsc::channel(16);
            let sub_id =
                broker.subscribe(black_box(Bytes::from_static(b"bench:sub_unsub")), tx);
            broker.unsubscribe(black_box(sub_id));
        });
    });
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_publish_fanout,
    bench_publish_payload_size,
    bench_subscribe_unsubscribe,
);
criterion_main!(benches);
