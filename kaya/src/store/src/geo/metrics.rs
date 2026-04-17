// SPDX-License-Identifier: AGPL-3.0-or-later
//! Prometheus metrics for the KAYA Geo module.
//!
//! KAYA Geo module, RESP3-compatible GEO commands, 100% Rust, no external
//! geospatial service. All metrics are registered lazily the first time a
//! geo operation is performed to avoid paying the cost on servers that do
//! not use the feature.

use once_cell::sync::Lazy;
use prometheus::{
    Histogram, HistogramOpts, HistogramVec, IntCounterVec, IntGauge, Opts,
};

/// Number of geo indexes currently alive (one per collection key).
pub static GEO_INDEXES_GAUGE: Lazy<IntGauge> = Lazy::new(|| {
    prometheus::register_int_gauge!(
        "kaya_geo_indexes_gauge",
        "Number of live KAYA geo indexes across all shards"
    )
    .unwrap_or_else(|_| {
        IntGauge::with_opts(Opts::new(
            "kaya_geo_indexes_gauge_fallback",
            "fallback gauge",
        ))
        .expect("construct fallback int gauge")
    })
});

/// Total points indexed, labelled by collection.
pub static GEO_POINTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    prometheus::register_int_counter_vec!(
        "kaya_geo_points_total",
        "Total points added to KAYA geo indexes",
        &["collection"]
    )
    .unwrap_or_else(|_| {
        IntCounterVec::new(
            Opts::new(
                "kaya_geo_points_total_fallback",
                "fallback counter",
            ),
            &["collection"],
        )
        .expect("construct fallback counter vec")
    })
});

/// GEOSEARCH duration histogram (milliseconds).
pub static GEO_SEARCH_DURATION_MS: Lazy<Histogram> = Lazy::new(|| {
    prometheus::register_histogram!(
        "kaya_geo_search_duration_ms",
        "Duration of GEOSEARCH queries in milliseconds",
        vec![0.05, 0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0]
    )
    .unwrap_or_else(|_| {
        Histogram::with_opts(HistogramOpts::new(
            "kaya_geo_search_duration_ms_fallback",
            "fallback histogram",
        ))
        .expect("construct fallback histogram")
    })
});

/// GEOSEARCH result set size histogram.
pub static GEO_SEARCH_RESULTS_RETURNED: Lazy<Histogram> = Lazy::new(|| {
    prometheus::register_histogram!(
        "kaya_geo_search_results_returned",
        "Number of members returned per GEOSEARCH call",
        vec![1.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1_000.0, 5_000.0, 10_000.0]
    )
    .unwrap_or_else(|_| {
        Histogram::with_opts(HistogramOpts::new(
            "kaya_geo_search_results_returned_fallback",
            "fallback histogram",
        ))
        .expect("construct fallback histogram")
    })
});

/// Unused guard to keep `HistogramVec` import alive if metrics change shape.
#[allow(dead_code)]
fn _unused_histogram_vec(_: &HistogramVec) {}
