// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Prometheus metrics for `armageddon-xds`.
//!
//! Metrics are registered once against the global Prometheus registry on first
//! access via `std::sync::OnceLock`.  Callers simply reference the
//! `static` handles defined here; registration panics at startup if it fails
//! (which only happens if two crates register the same metric name — a
//! programming error).

use prometheus::{
    exponential_buckets, register_histogram, register_int_counter, Histogram, IntCounter,
};
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// Debouncer metrics
// ---------------------------------------------------------------------------

/// Total number of batches flushed by the debouncer.
///
/// A batch can contain 1..N items.  Monitoring the rate of this counter vs.
/// the raw xDS response rate tells you how effective the debounce is.
pub static XDS_DEBOUNCE_BATCHES_TOTAL: OnceLock<IntCounter> = OnceLock::new();

/// Distribution of items per flushed batch.
///
/// Buckets cover 1, 2, 4, 8 … 256 items; observe against this histogram to
/// understand batch size distribution in production.
pub static XDS_DEBOUNCE_ITEMS_PER_BATCH: OnceLock<Histogram> = OnceLock::new();

/// Initialize (or retrieve) the `xds_debounce_batches_total` counter.
pub fn debounce_batches_total() -> &'static IntCounter {
    XDS_DEBOUNCE_BATCHES_TOTAL.get_or_init(|| {
        register_int_counter!(
            "xds_debounce_batches_total",
            "Total number of xDS resource batches flushed by the debouncer"
        )
        .expect("failed to register xds_debounce_batches_total")
    })
}

/// Initialize (or retrieve) the `xds_debounce_items_per_batch` histogram.
pub fn debounce_items_per_batch() -> &'static Histogram {
    XDS_DEBOUNCE_ITEMS_PER_BATCH.get_or_init(|| {
        register_histogram!(
            "xds_debounce_items_per_batch",
            "Number of xDS resource items in each debounced batch",
            exponential_buckets(1.0, 2.0, 9).expect("valid bucket spec")
        )
        .expect("failed to register xds_debounce_items_per_batch")
    })
}

/// Convenience: increment `xds_debounce_batches_total`.
#[inline]
pub fn inc_batches() {
    debounce_batches_total().inc();
}

/// Convenience: observe one batch flush with `n` items.
#[inline]
pub fn observe_batch_size(n: usize) {
    debounce_items_per_batch().observe(n as f64);
}
