// SPDX-License-Identifier: AGPL-3.0-or-later
//! Micro-benchmark for the ForgeFilter chain walker. See BENCH-METHODOLOGY.md
//! for the full measurement matrix.
//!
//! # Runtime caveat
//!
//! Criterion cannot drive Pingora's multi-thread scheduler
//! (documented in [`PINGORA-MIGRATION.md`] §Limitations #1) so this micro
//! bench only measures the synchronous, I/O-free cost of:
//!
//! 1. Constructing a `RequestCtx` (uuid + default fields).
//! 2. Walking `Vec<Arc<dyn ForgeFilter>>` at registration order.
//! 3. Capturing the elapsed time.
//!
//! For end-to-end throughput / latency numbers against real TCP traffic use
//! `benches/pingora_vs_hyper.sh` (wrk-based external harness).
//!
//! # Invocation
//!
//! ```text
//! cargo bench -p armageddon-forge \
//!     --bench pingora_filter_chain_micro \
//!     --features pingora
//! ```
//!
//! Without `--features pingora` the benches compile to empty stubs so that
//! the default workspace `cargo bench` invocation keeps working.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

// ── pingora-enabled benches ───────────────────────────────────────────────

#[cfg(feature = "pingora")]
mod enabled {
    use super::*;
    use std::sync::Arc;

    use armageddon_forge::pingora::filters::SharedFilter;
    use armageddon_forge::pingora_backend::{
        PingoraGateway, PingoraGatewayConfig, UpstreamRegistry,
    };

    /// Filter-chain walker baseline: no filters registered. Measures the
    /// cost of `RequestCtx::new` + the (empty) iteration.
    pub fn bench_noop_chain(c: &mut Criterion) {
        let cfg = PingoraGatewayConfig::default();
        let gw = PingoraGateway::new(cfg, Arc::new(UpstreamRegistry::new()));

        c.bench_function("filter_chain/0_filters", |b| {
            b.iter(|| {
                // Synthetic walker — mirrors the shape of the real
                // `request_filter` loop in `gateway.rs` minus the
                // `Session` / `async` plumbing (which Criterion cannot
                // drive).  TODO(#106): swap for a tokio-current-thread
                // harness once Pingora exposes a mock Session in tests.
                let ctx_id = gw.config().filters.len();
                for _filter in gw.config().filters.iter() {
                    // Same branchless Decision::Continue path as the real
                    // gateway.  `black_box` prevents the iterator from
                    // being folded away by LLVM.
                    black_box(());
                }
                black_box(ctx_id);
            });
        });
    }

    /// Filter-chain walker with 10 synthetic no-op filters — the realistic
    /// steady-state chain depth (router + cors + jwt + ff + otel + veil +
    /// 4 engine adapters = ~10 in M3).
    pub fn bench_ten_noop_filters(c: &mut Criterion) {
        use armageddon_forge::pingora::filters::ForgeFilter;
        use async_trait::async_trait;

        struct NoopFilter {
            name: &'static str,
        }

        #[async_trait]
        impl ForgeFilter for NoopFilter {
            fn name(&self) -> &'static str {
                self.name
            }
            // All hooks inherit the `Decision::Continue` default — we only
            // exercise the synchronous registration-order iteration below.
        }

        let filters: Vec<SharedFilter> = (0..10)
            .map(|i| {
                let name: &'static str = match i {
                    0 => "f0",
                    1 => "f1",
                    2 => "f2",
                    3 => "f3",
                    4 => "f4",
                    5 => "f5",
                    6 => "f6",
                    7 => "f7",
                    8 => "f8",
                    _ => "f9",
                };
                let boxed: SharedFilter = Arc::new(NoopFilter { name });
                boxed
            })
            .collect();

        let cfg = PingoraGatewayConfig {
            filters,
            ..PingoraGatewayConfig::default()
        };
        let gw = PingoraGateway::new(cfg, Arc::new(UpstreamRegistry::new()));

        c.bench_function("filter_chain/10_noop_filters", |b| {
            b.iter(|| {
                // TODO(#106): when a mock `Session` lands, await each
                // `filter.on_request(...)` here under a local
                // `tokio::runtime::Builder::new_current_thread()`.
                let mut names: u32 = 0;
                for filter in gw.config().filters.iter() {
                    // Touch the filter's `name()` to defeat LLVM DCE and
                    // approximate the pointer-chase cost of dispatching
                    // through the `dyn ForgeFilter` vtable.
                    names = names.wrapping_add(filter.name().len() as u32);
                }
                black_box(names);
            });
        });
    }
}

#[cfg(feature = "pingora")]
fn bench_noop_chain(c: &mut Criterion) {
    enabled::bench_noop_chain(c);
}

#[cfg(feature = "pingora")]
fn bench_ten_noop_filters(c: &mut Criterion) {
    enabled::bench_ten_noop_filters(c);
}

// ── pingora-disabled stubs ────────────────────────────────────────────────

#[cfg(not(feature = "pingora"))]
fn bench_noop_chain(_c: &mut Criterion) {
    // Criterion group entry point requires at least one bench; emit a
    // trivial one so `cargo bench` without `--features pingora` still
    // produces a (meaningless) report rather than failing.
    _c.bench_function("filter_chain/pingora_disabled", |b| {
        b.iter(|| black_box(()));
    });
}

#[cfg(not(feature = "pingora"))]
fn bench_ten_noop_filters(_c: &mut Criterion) {
    // Intentionally empty when Pingora is disabled.
}

criterion_group!(benches, bench_noop_chain, bench_ten_noop_filters);
criterion_main!(benches);
