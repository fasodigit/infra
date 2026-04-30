// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! # Admin + TERROIR Prometheus metrics for ARMAGEDDON
//!
//! All counters / histograms exposed on the admin loopback port
//! (port-policy range 9900-9999, cf. ARMAGEDDON port-policy.yaml).
//!
//! ## Admin metrics
//!
//! | Metric | Type | Labels |
//! |--------|------|--------|
//! | `armageddon_admin_requests_total` | Counter | `path`, `method`, `status` |
//! | `armageddon_admin_keto_check_duration_seconds` | Histogram | `decision` |
//! | `armageddon_admin_settings_cache_hits_total` | Counter | — |
//! | `armageddon_admin_otp_rate_limit_blocked_total` | Counter | — |
//!
//! ## TERROIR metrics (P0.H)
//!
//! | Metric | Type | Labels | Notes |
//! |--------|------|--------|-------|
//! | `armageddon_terroir_requests_total` | CounterVec | `path`, `method`, `status` | Per-route request counter |
//! | `armageddon_terroir_keto_check_duration_seconds` | HistogramVec | `decision` | Latency of TERROIR Keto checks |
//! | `armageddon_terroir_eudr_cache_hit_total` | Counter | — | Incremented when `X-Eudr-Cache-Status: HIT` |
//! | `armageddon_terroir_ws_sync_messages_total` | CounterVec | `direction` | CRDT sync WS frames |
//!
//! ## Failure modes
//!
//! Metric registration uses `OnceLock` and `get_or_init` — safe to call from
//! multiple threads at startup; subsequent calls return the existing handle.
//! A duplicate-registration error (e.g. in tests that register twice) is
//! silently tolerated because the counter/histogram value is still correct.

use prometheus::{
    register_counter, register_counter_vec, register_histogram_vec, Counter, CounterVec,
    HistogramOpts, HistogramVec,
};
use std::sync::OnceLock;

// ── admin_requests_total ──────────────────────────────────────────────────────

/// Counter for every request hitting an `/api/admin/*` or `/.well-known/jwks.json` route.
pub fn admin_requests_total() -> &'static CounterVec {
    static ONCE: OnceLock<CounterVec> = OnceLock::new();
    ONCE.get_or_init(|| {
        register_counter_vec!(
            "armageddon_admin_requests_total",
            "Total requests through admin routes, labelled by path, method and HTTP status",
            &["path", "method", "status"]
        )
        .unwrap_or_else(|_| {
            // Already registered (e.g. in integration harness) — create a
            // detached counter that still counts correctly.
            CounterVec::new(
                prometheus::Opts::new("armageddon_admin_requests_total_fallback", "fallback"),
                &["path", "method", "status"],
            )
            .expect("CounterVec::new must succeed")
        })
    })
}

// ── keto_check_duration_seconds ───────────────────────────────────────────────

/// Histogram for the round-trip time of a Keto `relation_tuples.check` call.
///
/// Label `decision` is one of `"allowed"` or `"denied"`.  This lets you
/// build separate latency percentiles for the two code paths.
pub fn keto_check_duration_seconds() -> &'static HistogramVec {
    static ONCE: OnceLock<HistogramVec> = OnceLock::new();
    ONCE.get_or_init(|| {
        register_histogram_vec!(
            HistogramOpts::new(
                "armageddon_admin_keto_check_duration_seconds",
                "Latency of Keto authz check calls for admin routes",
            )
            .buckets(vec![
                0.001, 0.005, 0.010, 0.025, 0.050, 0.100, 0.250, 0.500, 1.0
            ]),
            &["decision"]
        )
        .unwrap_or_else(|_| {
            HistogramVec::new(
                HistogramOpts::new(
                    "armageddon_admin_keto_check_duration_seconds_fallback",
                    "fallback",
                ),
                &["decision"],
            )
            .expect("HistogramVec::new must succeed")
        })
    })
}

// ── settings_cache_hits_total ─────────────────────────────────────────────────

/// Counter incremented each time a hot-path settings lookup hits the
/// `AdminSettingsCache` in-memory store (avoids a DB/BFF round-trip).
pub fn settings_cache_hits_total() -> &'static Counter {
    static ONCE: OnceLock<Counter> = OnceLock::new();
    ONCE.get_or_init(|| {
        register_counter!(
            "armageddon_admin_settings_cache_hits_total",
            "Number of admin settings lookups served from local in-memory cache"
        )
        .unwrap_or_else(|_| {
            Counter::new(
                "armageddon_admin_settings_cache_hits_total_fallback",
                "fallback",
            )
            .expect("Counter::new must succeed")
        })
    })
}

// ── ws_connections_active ─────────────────────────────────────────────────────

/// Gauge tracking the number of live `/ws/admin/approval` WebSocket connections
/// proxied by ARMAGEDDON to auth-ms at any given moment.
pub fn ws_connections_active() -> &'static prometheus::Gauge {
    use prometheus::{register_gauge, Gauge};
    static ONCE: OnceLock<Gauge> = OnceLock::new();
    ONCE.get_or_init(|| {
        register_gauge!(
            "armageddon_admin_ws_connections_active",
            "Number of active WebSocket push-approval connections proxied to auth-ms"
        )
        .unwrap_or_else(|_| {
            Gauge::new(
                "armageddon_admin_ws_connections_active_fallback",
                "fallback",
            )
            .expect("Gauge::new must succeed")
        })
    })
}

// ── ws_messages_total ─────────────────────────────────────────────────────────

/// Counter for WebSocket frames forwarded through the push-approval proxy.
///
/// Label `direction` is one of:
/// - `"client_to_server"` — frames from the browser to auth-ms.
/// - `"server_to_client"` — frames from auth-ms to the browser.
pub fn ws_messages_total() -> &'static CounterVec {
    static ONCE: OnceLock<CounterVec> = OnceLock::new();
    ONCE.get_or_init(|| {
        register_counter_vec!(
            "armageddon_admin_ws_messages_total",
            "Total WebSocket frames forwarded through the push-approval proxy",
            &["direction"]
        )
        .unwrap_or_else(|_| {
            CounterVec::new(
                prometheus::Opts::new("armageddon_admin_ws_messages_total_fallback", "fallback"),
                &["direction"],
            )
            .expect("CounterVec::new must succeed")
        })
    })
}

// ── otp_rate_limit_blocked_total ──────────────────────────────────────────────

/// Counter incremented each time a `POST /api/admin/otp/issue` request is
/// blocked by the per-user 3-req/5-min rate limiter.
pub fn otp_rate_limit_blocked_total() -> &'static Counter {
    static ONCE: OnceLock<Counter> = OnceLock::new();
    ONCE.get_or_init(|| {
        register_counter!(
            "armageddon_admin_otp_rate_limit_blocked_total",
            "Number of OTP issue requests blocked by per-user rate limit"
        )
        .unwrap_or_else(|_| {
            Counter::new(
                "armageddon_admin_otp_rate_limit_blocked_total_fallback",
                "fallback",
            )
            .expect("Counter::new must succeed")
        })
    })
}

// ══════════════════════════════════════════════════════════════════════════════
// TERROIR metrics (P0.H)
// ══════════════════════════════════════════════════════════════════════════════

// ── armageddon_terroir_requests_total ─────────────────────────────────────────

/// Counter for every request hitting a `/api/terroir/*` or `/ws/terroir/sync`
/// route, labelled by path prefix, HTTP method, and response status code.
///
/// The `path` label uses the matched route prefix (e.g. `/api/terroir/core/`)
/// rather than the full URL to bound cardinality.
pub fn terroir_requests_total() -> &'static CounterVec {
    static ONCE: OnceLock<CounterVec> = OnceLock::new();
    ONCE.get_or_init(|| {
        register_counter_vec!(
            "armageddon_terroir_requests_total",
            "Total requests through TERROIR routes, labelled by path prefix, method and HTTP status",
            &["path", "method", "status"]
        )
        .unwrap_or_else(|_| {
            CounterVec::new(
                prometheus::Opts::new(
                    "armageddon_terroir_requests_total_fallback",
                    "fallback",
                ),
                &["path", "method", "status"],
            )
            .expect("CounterVec::new must succeed")
        })
    })
}

// ── armageddon_terroir_keto_check_duration_seconds ───────────────────────────

/// Histogram for the round-trip time of a Keto `relation_tuples.check` call
/// issued for TERROIR routes (namespace `Tenant`, `Cooperative`, `Parcel`).
///
/// Label `decision` is one of `"allowed"` or `"denied"`.
pub fn terroir_keto_check_duration_seconds() -> &'static HistogramVec {
    static ONCE: OnceLock<HistogramVec> = OnceLock::new();
    ONCE.get_or_init(|| {
        register_histogram_vec!(
            HistogramOpts::new(
                "armageddon_terroir_keto_check_duration_seconds",
                "Latency of Keto authz check calls for TERROIR routes",
            )
            .buckets(vec![
                0.001, 0.005, 0.010, 0.025, 0.050, 0.100, 0.250, 0.500, 1.0,
            ]),
            &["decision"]
        )
        .unwrap_or_else(|_| {
            HistogramVec::new(
                HistogramOpts::new(
                    "armageddon_terroir_keto_check_duration_seconds_fallback",
                    "fallback",
                ),
                &["decision"],
            )
            .expect("HistogramVec::new must succeed")
        })
    })
}

// ── armageddon_terroir_eudr_cache_hit_total ───────────────────────────────────

/// Counter incremented each time `terroir-eudr` returns a response whose
/// `X-Eudr-Cache-Status` header equals `"HIT"`.
///
/// This is read from the upstream response header in the access-log filter's
/// `on_logging` hook; it lets operators monitor the Hansen GFC tile cache
/// effectiveness without touching the service directly.
pub fn terroir_eudr_cache_hit_total() -> &'static Counter {
    static ONCE: OnceLock<Counter> = OnceLock::new();
    ONCE.get_or_init(|| {
        register_counter!(
            "armageddon_terroir_eudr_cache_hit_total",
            "Number of EUDR validation responses served from the Hansen GFC tile cache (X-Eudr-Cache-Status: HIT)"
        )
        .unwrap_or_else(|_| {
            Counter::new(
                "armageddon_terroir_eudr_cache_hit_total_fallback",
                "fallback",
            )
            .expect("Counter::new must succeed")
        })
    })
}

// ── armageddon_terroir_ws_sync_messages_total ─────────────────────────────────

/// Counter for WebSocket frames forwarded through the `/ws/terroir/sync`
/// CRDT delta-sync proxy.
///
/// Label `direction` is one of:
/// - `"client_to_server"` — CRDT delta updates from the mobile agent to
///   `terroir-mobile-bff`.
/// - `"server_to_client"` — conflict-resolved patches or server-initiated
///   deltas sent back to the agent.
pub fn terroir_ws_sync_messages_total() -> &'static CounterVec {
    static ONCE: OnceLock<CounterVec> = OnceLock::new();
    ONCE.get_or_init(|| {
        register_counter_vec!(
            "armageddon_terroir_ws_sync_messages_total",
            "Total WebSocket frames forwarded through the CRDT delta-sync proxy",
            &["direction"]
        )
        .unwrap_or_else(|_| {
            CounterVec::new(
                prometheus::Opts::new(
                    "armageddon_terroir_ws_sync_messages_total_fallback",
                    "fallback",
                ),
                &["direction"],
            )
            .expect("CounterVec::new must succeed")
        })
    })
}
