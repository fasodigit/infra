// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//! Benchmark: `tokio::net` TCP vs `io_uring` TCP — write throughput comparison.
//!
//! # Goal
//!
//! Measure the wall-clock time to echo a fixed payload over a localhost TCP
//! connection using two back-ends:
//!
//! 1. **tokio-net** — standard `tokio::net::TcpListener` / `TcpStream` with
//!    `write_all` and `read_exact`.
//! 2. **io_uring** (Linux only, `--features io_uring`) — the same payload
//!    routed through [`armageddon_forge::io_uring_backend::IoUringProxyServer`]
//!    with [`armageddon_forge::io_uring_backend::EchoHandler`].
//!
//! # Running
//!
//! ```text
//! # Standard tokio back-end only:
//! cargo bench -p armageddon-bench --bench uring_compare
//!
//! # With io_uring back-end (Linux, kernel >= 5.13 recommended):
//! cargo bench -p armageddon-bench --bench uring_compare --features io_uring
//! ```
//!
//! # Target
//!
//! The `io_uring` path is expected to deliver ≥ +30 % higher write throughput
//! than the tokio-net path on write-heavy workloads (repeated 64 KiB sends).

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

// ---------------------------------------------------------------------------
// Payload sizes to benchmark
// ---------------------------------------------------------------------------

const PAYLOAD_SIZES: &[usize] = &[
    1_024,        // 1 KiB — small messages
    16_384,       // 16 KiB — medium frames
    65_536,       // 64 KiB — large writes (primary target)
];

// ---------------------------------------------------------------------------
// tokio-net echo server helper
// ---------------------------------------------------------------------------

/// Spin up a tokio-net echo server on an ephemeral port.
///
/// Returns the server address.  The server runs as a detached task and stops
/// when the tokio runtime shuts down.
async fn spawn_tokio_echo(rt_handle: tokio::runtime::Handle) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    rt_handle.spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 65_536 * 2];
                loop {
                    let n = match stream.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => n,
                    };
                    if stream.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            });
        }
    });

    addr
}

// ---------------------------------------------------------------------------
// io_uring echo server helper (Linux only, feature-gated)
// ---------------------------------------------------------------------------

#[cfg(all(target_os = "linux", feature = "io_uring"))]
fn spawn_io_uring_echo() -> SocketAddr {
    use armageddon_forge::io_uring_backend::{
        EchoHandler, IoUringMetrics, IoUringProxyServer, IoUringServerConfig,
    };

    // Grab an ephemeral port.
    let tmp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = tmp.local_addr().unwrap();
    drop(tmp);

    let cfg = IoUringServerConfig::new(addr);
    let handler = Arc::new(EchoHandler);
    let metrics = IoUringMetrics::new();
    let srv = IoUringProxyServer::new(cfg, handler, metrics);

    std::thread::spawn(move || {
        if let Err(e) = srv.run() {
            eprintln!("io_uring bench server error: {e}");
        }
    });

    // Give the io_uring executor a moment to bind.
    std::thread::sleep(std::time::Duration::from_millis(50));
    addr
}

// ---------------------------------------------------------------------------
// Benchmark body: send N bytes to an echo server, receive them back
// ---------------------------------------------------------------------------

/// Send `payload_size` bytes to `addr`, receive the echo, return the elapsed
/// time.  Called inside Criterion's measurement loop.
async fn echo_roundtrip(addr: SocketAddr, payload: &[u8]) {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream.write_all(payload).await.unwrap();

    let mut received = 0usize;
    let mut buf = vec![0u8; payload.len()];
    while received < payload.len() {
        let n = stream.read(&mut buf[received..]).await.unwrap();
        assert!(n > 0, "unexpected EOF");
        received += n;
    }
    assert_eq!(&buf[..], payload, "echo payload mismatch");
}

// ---------------------------------------------------------------------------
// Criterion benchmark groups
// ---------------------------------------------------------------------------

fn bench_tokio_net(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let addr = rt.block_on(spawn_tokio_echo(rt.handle().clone()));

    let mut group = c.benchmark_group("tcp_echo/tokio_net");

    for &size in PAYLOAD_SIZES {
        let payload = vec![0xABu8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &payload,
            |b, payload| {
                b.to_async(&rt)
                    .iter(|| echo_roundtrip(addr, payload));
            },
        );
    }

    group.finish();
}

#[cfg(all(target_os = "linux", feature = "io_uring"))]
fn bench_io_uring(c: &mut Criterion) {
    // io_uring server runs on its own OS thread; tokio is only used client-side.
    let addr = spawn_io_uring_echo();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let mut group = c.benchmark_group("tcp_echo/io_uring");

    for &size in PAYLOAD_SIZES {
        let payload = vec![0xABu8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &payload,
            |b, payload| {
                b.to_async(&rt)
                    .iter(|| echo_roundtrip(addr, payload));
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion main
// ---------------------------------------------------------------------------

#[cfg(all(target_os = "linux", feature = "io_uring"))]
criterion_group!(benches, bench_tokio_net, bench_io_uring);

#[cfg(not(all(target_os = "linux", feature = "io_uring")))]
criterion_group!(benches, bench_tokio_net);

criterion_main!(benches);
