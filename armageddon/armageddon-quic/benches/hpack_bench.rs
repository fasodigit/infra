// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Criterion benchmark: HPACK Huffman decoder — scalar vs SIMD.
//!
//! Decodes 10 MB of Huffman-encoded HTTP/2 headers representative of
//! production traffic at the ARMAGEDDON gateway.
//!
//! # Running
//!
//! ```bash
//! cargo bench -p armageddon-quic --bench hpack_bench
//! ```
//!
//! Criterion writes HTML reports to `target/criterion/hpack/`.
//! Target: SIMD path ≥ 30 % faster than scalar on x86_64 with AVX2.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use armageddon_quic::{decode_huffman_scalar, decode_huffman_simd, encode_huffman};

// ---------------------------------------------------------------------------
// Corpus generation
// ---------------------------------------------------------------------------

/// Typical HTTP/2 header name+value pairs seen at the ARMAGEDDON ingress.
/// The corpus is intentionally skewed toward lower-case ASCII (common in HTTP
/// pseudo-headers) to stress the fast-decode path that SIMD exploits.
static HEADER_CORPUS: &[(&str, &str)] = &[
    (":method",          "GET"),
    (":scheme",          "https"),
    (":authority",       "api.faso.bf"),
    (":path",            "/v1/poulets?page=1&limit=20"),
    ("accept",           "application/json"),
    ("accept-encoding",  "gzip, deflate, br"),
    ("accept-language",  "fr-BF,fr;q=0.9,en;q=0.8"),
    ("cache-control",    "no-cache"),
    ("content-type",     "application/json; charset=utf-8"),
    ("user-agent",       "ARMAGEDDON-client/1.1 (Burkina Faso)"),
    ("x-request-id",     "550e8400-e29b-41d4-a716-446655440000"),
    ("x-forwarded-for",  "196.1.42.100"),
    ("authorization",    "Bearer eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9"),
    ("content-length",   "1024"),
    ("x-correlation-id", "01HV8ABCDEFG01234567890"),
];

/// Build a raw (unencoded) corpus of `target_bytes` bytes by cycling through
/// `HEADER_CORPUS` entries.
fn build_raw_corpus(target_bytes: usize) -> Vec<u8> {
    let mut buf = Vec::with_capacity(target_bytes + 256);
    let pattern: Vec<u8> = HEADER_CORPUS
        .iter()
        .flat_map(|(k, v)| {
            let mut entry = Vec::new();
            entry.extend_from_slice(k.as_bytes());
            entry.push(b':');
            entry.push(b' ');
            entry.extend_from_slice(v.as_bytes());
            entry.push(b'\r');
            entry.push(b'\n');
            entry
        })
        .collect();

    while buf.len() < target_bytes {
        buf.extend_from_slice(&pattern);
    }
    buf.truncate(target_bytes);
    buf
}

// ---------------------------------------------------------------------------
// Benchmark groups
// ---------------------------------------------------------------------------

/// Benchmark decoding 10 MB of encoded headers.
fn bench_decode_10mb(c: &mut Criterion) {
    const TARGET_BYTES: usize = 10 * 1024 * 1024; // 10 MB raw (decoded) size

    let raw = build_raw_corpus(TARGET_BYTES);
    let encoded = encode_huffman(&raw);

    let encoded_len = encoded.len();

    let mut group = c.benchmark_group("hpack/decode_10mb");
    // Report throughput in terms of *encoded* bytes consumed per second.
    group.throughput(Throughput::Bytes(encoded_len as u64));
    group.sample_size(20);

    // -- Scalar path ---------------------------------------------------------
    group.bench_function("scalar", |b| {
        b.iter(|| {
            let mut out = Vec::with_capacity(raw.len());
            decode_huffman_scalar(black_box(&encoded), &mut out).expect("scalar decode");
            black_box(out)
        });
    });

    // -- SIMD path (auto-selects AVX2 / NEON / scalar based on CPU) ----------
    group.bench_function("simd", |b| {
        b.iter(|| {
            let mut out = Vec::with_capacity(raw.len());
            decode_huffman_simd(black_box(&encoded), &mut out).expect("simd decode");
            black_box(out)
        });
    });

    group.finish();
}

/// Benchmark decoding varying corpus sizes to characterise scaling.
fn bench_decode_scaling(c: &mut Criterion) {
    let sizes: &[usize] = &[
        1_024,          //  1 KB
        16_384,         // 16 KB
        65_536,         // 64 KB
        262_144,        // 256 KB
        1_048_576,      //   1 MB
    ];

    let mut group = c.benchmark_group("hpack/decode_scaling");

    for &sz in sizes {
        let raw = build_raw_corpus(sz);
        let encoded = encode_huffman(&raw);

        group.throughput(Throughput::Bytes(encoded.len() as u64));

        group.bench_with_input(BenchmarkId::new("scalar", sz), &encoded, |b, enc| {
            b.iter(|| {
                let mut out = Vec::with_capacity(raw.len());
                decode_huffman_scalar(black_box(enc), &mut out).expect("scalar");
                black_box(out)
            });
        });

        group.bench_with_input(BenchmarkId::new("simd", sz), &encoded, |b, enc| {
            b.iter(|| {
                let mut out = Vec::with_capacity(raw.len());
                decode_huffman_simd(black_box(enc), &mut out).expect("simd");
                black_box(out)
            });
        });
    }

    group.finish();
}

/// Benchmark the encoder (to understand full round-trip cost).
fn bench_encode_10mb(c: &mut Criterion) {
    const TARGET_BYTES: usize = 10 * 1024 * 1024;
    let raw = build_raw_corpus(TARGET_BYTES);

    let mut group = c.benchmark_group("hpack/encode_10mb");
    group.throughput(Throughput::Bytes(raw.len() as u64));
    group.sample_size(10);

    group.bench_function("scalar", |b| {
        b.iter(|| {
            let out = encode_huffman(black_box(&raw));
            black_box(out)
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion entry-point
// ---------------------------------------------------------------------------

criterion_group!(benches, bench_decode_10mb, bench_decode_scaling, bench_encode_10mb);
criterion_main!(benches);
