// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Prometheus metrics bundle for the Pingora gateway subsystems.
//!
//! All metrics are registered at constructor time on the supplied
//! `prometheus::Registry` (or `prometheus::default_registry()` when `None`
//! is passed).  There are no `OnceLock` initialisations — every metric is
//! constructed and registered in [`PingoraMetrics::new`].
//!
//! # Subsystems
//!
//! | Subsystem | Key metrics |
//! |-----------|------------|
//! | Shadow mode | `armageddon_shadow_requests_total`, `armageddon_shadow_diverged_total`, `armageddon_shadow_latency_diff_seconds`, `armageddon_shadow_sample_rate` |
//! | Traffic split | `armageddon_traffic_split_decisions_total`, `armageddon_traffic_split_sticky_sessions` |
//! | Health checker | `armageddon_forge_endpoint_up`, `armageddon_forge_health_check_duration_seconds`, `armageddon_forge_health_transitions_total` |
//! | xDS watcher | `armageddon_xds_updates_total`, `armageddon_xds_current_version`, `armageddon_xds_nack_total` |
//! | SVID rotation | `armageddon_mesh_svid_rotations_total`, `armageddon_mesh_svid_expires_seconds`, `armageddon_mesh_svid_fetch_errors_total` |
//!
//! # Failure modes
//!
//! - Registration failure (duplicate name): the `new` constructor returns
//!   `Err(prometheus::Error)`.  The caller should fall back to a
//!   `NullPingoraMetrics` or log and continue with no-op counters.
//! - Label cardinality: `cluster`, `endpoint`, `spiffe_id`, `resource_type`
//!   labels are all bounded by deployment topology (< 100 values each).
//!   Never use unbounded labels such as `request_id`, `user_id`, or `path`.

use prometheus::{
    HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, Opts, Registry,
};

// ─────────────────────────────────────────────────────────────────────────────
// PingoraMetrics — the single shared bundle
// ─────────────────────────────────────────────────────────────────────────────

/// Prometheus metrics bundle covering all Pingora subsystems.
///
/// Construct once at gateway startup and share via `Arc<PingoraMetrics>`.
///
/// ```rust,ignore
/// let m = Arc::new(PingoraMetrics::new(registry.as_ref()).expect("metrics"));
/// ```
#[derive(Clone, Debug)]
pub struct PingoraMetrics {
    // ── Shadow mode ──────────────────────────────────────────────────────────
    /// `armageddon_shadow_requests_total{status}` — counter.
    ///
    /// `status` values: `accepted`, `rejected`, `diverged`, `match`.
    pub shadow_requests_total: IntCounterVec,

    /// `armageddon_shadow_diverged_total{field}` — counter.
    ///
    /// `field` values: `status`, `body_hash`, `headers`, `latency`.
    pub shadow_diverged_total: IntCounterVec,

    /// `armageddon_shadow_latency_diff_seconds` — histogram.
    ///
    /// Records the signed latency difference `pingora_latency - hyper_latency`
    /// in seconds.  Negative values mean Pingora was faster.
    pub shadow_latency_diff_seconds: HistogramVec,

    /// `armageddon_shadow_sample_rate` — gauge (0.0–1.0).
    pub shadow_sample_rate: IntGaugeVec,

    // ── Traffic split ────────────────────────────────────────────────────────
    /// `armageddon_traffic_split_decisions_total{route, variant, decision}` — counter.
    ///
    /// `decision` values: `primary`, `canary`, `shadow`.
    pub traffic_split_decisions_total: IntCounterVec,

    /// `armageddon_traffic_split_sticky_sessions` — gauge.
    ///
    /// Number of currently tracked sticky-session entries.
    pub traffic_split_sticky_sessions: IntGaugeVec,

    // ── Health checker ────────────────────────────────────────────────────────
    /// `armageddon_forge_endpoint_up{cluster, endpoint}` — gauge (0 or 1).
    pub forge_endpoint_up: IntGaugeVec,

    /// `armageddon_forge_health_check_duration_seconds{cluster, endpoint, type}` — histogram.
    pub forge_health_check_duration_seconds: HistogramVec,

    /// `armageddon_forge_health_transitions_total{cluster, endpoint, from, to}` — counter.
    ///
    /// `from` / `to` values: `healthy`, `unhealthy`.
    pub forge_health_transitions_total: IntCounterVec,

    // ── xDS watcher ─────────────────────────────────────────────────────────
    /// `armageddon_xds_updates_total{resource_type, action}` — counter.
    ///
    /// `resource_type` values: `cds`, `eds`, `lds`, `rds`, `sds`.
    /// `action` values: `cluster_added`, `route_modified`, `secret_rotated`,
    /// `endpoint_updated`, `listener_updated`, `nack`.
    pub xds_updates_total: IntCounterVec,

    /// `armageddon_xds_current_version{resource_type}` — gauge.
    ///
    /// Encodes the xDS version as an integer (incremented monotonically by the
    /// callback).  Useful for detecting stale configs.
    pub xds_current_version: IntGaugeVec,

    /// `armageddon_xds_nack_total{resource_type, reason}` — counter.
    pub xds_nack_total: IntCounterVec,

    // ── SVID rotation ────────────────────────────────────────────────────────
    /// `armageddon_mesh_svid_rotations_total{spiffe_id}` — counter.
    pub mesh_svid_rotations_total: IntCounterVec,

    /// `armageddon_mesh_svid_expires_seconds{spiffe_id}` — gauge.
    ///
    /// Unix timestamp when the current SVID for `spiffe_id` expires.
    pub mesh_svid_expires_seconds: IntGaugeVec,

    /// `armageddon_mesh_svid_fetch_errors_total{reason}` — counter.
    ///
    /// `reason` values: `timeout`, `spire_unavailable`, `parse_error`,
    /// `channel_lagged`.
    pub mesh_svid_fetch_errors_total: IntCounterVec,
}

impl PingoraMetrics {
    /// Register all Pingora metrics on `registry`.
    ///
    /// Pass `prometheus::default_registry()` for compatibility with crates
    /// that use the global `register_*!` macros.
    ///
    /// # Errors
    ///
    /// Returns `prometheus::Error` if a metric with the same name was already
    /// registered on the same registry.
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        // ── Shadow mode ──────────────────────────────────────────────────────
        let shadow_requests_total = IntCounterVec::new(
            Opts::new(
                "armageddon_shadow_requests_total",
                "Total shadow-mode request outcomes by status",
            ),
            &["status"],
        )?;
        registry.register(Box::new(shadow_requests_total.clone()))?;

        let shadow_diverged_total = IntCounterVec::new(
            Opts::new(
                "armageddon_shadow_diverged_total",
                "Shadow divergences by field (status / body_hash / headers / latency)",
            ),
            &["field"],
        )?;
        registry.register(Box::new(shadow_diverged_total.clone()))?;

        let shadow_latency_diff_seconds = HistogramVec::new(
            HistogramOpts::new(
                "armageddon_shadow_latency_diff_seconds",
                "Latency difference between Pingora and hyper responses (pingora - hyper), in seconds",
            )
            .buckets(vec![
                -1.0, -0.5, -0.25, -0.1, -0.05, -0.025, -0.01, -0.005, -0.001,
                0.0,
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0,
            ]),
            &["route"],
        )?;
        registry.register(Box::new(shadow_latency_diff_seconds.clone()))?;

        let shadow_sample_rate = IntGaugeVec::new(
            Opts::new(
                "armageddon_shadow_sample_rate",
                "Current shadow-mode sample rate as integer percentage (0-100)",
            ),
            &["mode"],
        )?;
        registry.register(Box::new(shadow_sample_rate.clone()))?;

        // ── Traffic split ────────────────────────────────────────────────────
        let traffic_split_decisions_total = IntCounterVec::new(
            Opts::new(
                "armageddon_traffic_split_decisions_total",
                "Total traffic-split routing decisions by route, variant label and decision type",
            ),
            &["route", "variant", "decision"],
        )?;
        registry.register(Box::new(traffic_split_decisions_total.clone()))?;

        let traffic_split_sticky_sessions = IntGaugeVec::new(
            Opts::new(
                "armageddon_traffic_split_sticky_sessions",
                "Number of currently active sticky-session entries per route",
            ),
            &["route"],
        )?;
        registry.register(Box::new(traffic_split_sticky_sessions.clone()))?;

        // ── Health checker ────────────────────────────────────────────────────
        let forge_endpoint_up = IntGaugeVec::new(
            Opts::new(
                "armageddon_forge_endpoint_up",
                "1 = endpoint healthy, 0 = endpoint unhealthy",
            ),
            &["cluster", "endpoint"],
        )?;
        registry.register(Box::new(forge_endpoint_up.clone()))?;

        let forge_health_check_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "armageddon_forge_health_check_duration_seconds",
                "Duration of individual upstream health-check probes",
            )
            .buckets(prometheus::exponential_buckets(0.001, 2.0, 14)?),
            &["cluster", "endpoint", "type"],
        )?;
        registry.register(Box::new(forge_health_check_duration_seconds.clone()))?;

        let forge_health_transitions_total = IntCounterVec::new(
            Opts::new(
                "armageddon_forge_health_transitions_total",
                "Total health-state transitions per endpoint",
            ),
            &["cluster", "endpoint", "from", "to"],
        )?;
        registry.register(Box::new(forge_health_transitions_total.clone()))?;

        // ── xDS watcher ─────────────────────────────────────────────────────
        let xds_updates_total = IntCounterVec::new(
            Opts::new(
                "armageddon_xds_updates_total",
                "Total xDS resource updates applied by resource type and action",
            ),
            &["resource_type", "action"],
        )?;
        registry.register(Box::new(xds_updates_total.clone()))?;

        let xds_current_version = IntGaugeVec::new(
            Opts::new(
                "armageddon_xds_current_version",
                "Monotonically increasing version counter for each xDS resource type",
            ),
            &["resource_type"],
        )?;
        registry.register(Box::new(xds_current_version.clone()))?;

        let xds_nack_total = IntCounterVec::new(
            Opts::new(
                "armageddon_xds_nack_total",
                "Total xDS NACKs sent by resource type and reason",
            ),
            &["resource_type", "reason"],
        )?;
        registry.register(Box::new(xds_nack_total.clone()))?;

        // ── SVID rotation ────────────────────────────────────────────────────
        let mesh_svid_rotations_total = IntCounterVec::new(
            Opts::new(
                "armageddon_mesh_svid_rotations_total",
                "Total successful SPIFFE SVID rotations by SPIFFE ID",
            ),
            &["spiffe_id"],
        )?;
        registry.register(Box::new(mesh_svid_rotations_total.clone()))?;

        let mesh_svid_expires_seconds = IntGaugeVec::new(
            Opts::new(
                "armageddon_mesh_svid_expires_seconds",
                "Unix timestamp (seconds) when the current SVID expires for each SPIFFE ID",
            ),
            &["spiffe_id"],
        )?;
        registry.register(Box::new(mesh_svid_expires_seconds.clone()))?;

        let mesh_svid_fetch_errors_total = IntCounterVec::new(
            Opts::new(
                "armageddon_mesh_svid_fetch_errors_total",
                "Total SVID fetch errors by reason",
            ),
            &["reason"],
        )?;
        registry.register(Box::new(mesh_svid_fetch_errors_total.clone()))?;

        Ok(Self {
            shadow_requests_total,
            shadow_diverged_total,
            shadow_latency_diff_seconds,
            shadow_sample_rate,
            traffic_split_decisions_total,
            traffic_split_sticky_sessions,
            forge_endpoint_up,
            forge_health_check_duration_seconds,
            forge_health_transitions_total,
            xds_updates_total,
            xds_current_version,
            xds_nack_total,
            mesh_svid_rotations_total,
            mesh_svid_expires_seconds,
            mesh_svid_fetch_errors_total,
        })
    }

    /// Construct using `prometheus::default_registry()`.
    ///
    /// Useful in tests and in the main binary when a dedicated registry is not
    /// threaded through.
    pub fn with_default_registry() -> Result<Self, prometheus::Error> {
        Self::new(prometheus::default_registry())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use prometheus::{Encoder, Registry, TextEncoder};

    fn isolated_registry() -> Registry {
        Registry::new()
    }

    /// All metrics must register without error on a fresh registry.
    #[test]
    fn metrics_register_successfully() {
        let r = isolated_registry();
        PingoraMetrics::new(&r).expect("metrics registration must succeed");
    }

    /// Double-registration on the same registry must fail (prometheus contract).
    #[test]
    fn double_registration_fails() {
        let r = isolated_registry();
        PingoraMetrics::new(&r).expect("first registration ok");
        assert!(
            PingoraMetrics::new(&r).is_err(),
            "second registration on the same registry must fail"
        );
    }

    /// Shadow requests counter increments and can be gathered.
    #[test]
    fn shadow_counter_increments() {
        let r = isolated_registry();
        let m = PingoraMetrics::new(&r).unwrap();
        m.shadow_requests_total
            .with_label_values(&["accepted"])
            .inc_by(3);

        let families = r.gather();
        let fam = families
            .iter()
            .find(|f| f.get_name() == "armageddon_shadow_requests_total")
            .expect("metric must be present");
        let val = fam
            .get_metric()
            .iter()
            .find(|m| {
                m.get_label()
                    .iter()
                    .any(|l| l.get_name() == "status" && l.get_value() == "accepted")
            })
            .map(|m| m.get_counter().get_value())
            .expect("accepted label must exist");
        assert_eq!(val, 3.0);
    }

    /// Health endpoint_up gauge can be set to 0 and 1.
    #[test]
    fn forge_endpoint_up_gauge() {
        let r = isolated_registry();
        let m = PingoraMetrics::new(&r).unwrap();

        m.forge_endpoint_up
            .with_label_values(&["api-cluster", "10.0.0.1:8080"])
            .set(1);
        m.forge_endpoint_up
            .with_label_values(&["api-cluster", "10.0.0.2:8080"])
            .set(0);

        let families = r.gather();
        let fam = families
            .iter()
            .find(|f| f.get_name() == "armageddon_forge_endpoint_up")
            .expect("gauge must be present");
        assert_eq!(fam.get_metric().len(), 2);
    }

    /// xDS counter and version gauge can be recorded.
    #[test]
    fn xds_counters_and_version() {
        let r = isolated_registry();
        let m = PingoraMetrics::new(&r).unwrap();

        m.xds_updates_total
            .with_label_values(&["cds", "cluster_added"])
            .inc();
        m.xds_current_version.with_label_values(&["cds"]).set(7);

        let families = r.gather();
        let updates = families
            .iter()
            .find(|f| f.get_name() == "armageddon_xds_updates_total")
            .expect("updates counter must be present");
        let _ = updates; // present, not zero

        let version = families
            .iter()
            .find(|f| f.get_name() == "armageddon_xds_current_version")
            .expect("version gauge must be present");
        let v = version
            .get_metric()
            .iter()
            .find(|m| {
                m.get_label()
                    .iter()
                    .any(|l| l.get_name() == "resource_type" && l.get_value() == "cds")
            })
            .map(|m| m.get_gauge().get_value())
            .expect("cds version must exist");
        assert_eq!(v, 7.0);
    }

    /// SVID rotation counter increments correctly.
    #[test]
    fn svid_rotation_counter() {
        let r = isolated_registry();
        let m = PingoraMetrics::new(&r).unwrap();
        let spiffe_id = "spiffe://faso.gov.bf/ns/armageddon/sa/gateway";
        m.mesh_svid_rotations_total
            .with_label_values(&[spiffe_id])
            .inc();

        let families = r.gather();
        let fam = families
            .iter()
            .find(|f| f.get_name() == "armageddon_mesh_svid_rotations_total")
            .expect("rotations counter must exist");
        let val = fam
            .get_metric()
            .first()
            .map(|m| m.get_counter().get_value())
            .unwrap_or(0.0);
        assert_eq!(val, 1.0);
    }

    /// All metrics appear in a full text encode.
    #[test]
    fn all_metric_names_appear_in_text_output() {
        let r = isolated_registry();
        let m = PingoraMetrics::new(&r).unwrap();

        // Touch each metric once so they appear in the output.
        m.shadow_requests_total.with_label_values(&["match"]).inc();
        m.shadow_diverged_total.with_label_values(&["status"]).inc();
        m.shadow_latency_diff_seconds
            .with_label_values(&["default"])
            .observe(0.001);
        m.shadow_sample_rate.with_label_values(&["shadow"]).set(10);
        m.traffic_split_decisions_total
            .with_label_values(&["r", "stable", "primary"])
            .inc();
        m.traffic_split_sticky_sessions
            .with_label_values(&["r"])
            .set(0);
        m.forge_endpoint_up
            .with_label_values(&["c", "127.0.0.1:80"])
            .set(1);
        m.forge_health_check_duration_seconds
            .with_label_values(&["c", "127.0.0.1:80", "http"])
            .observe(0.01);
        m.forge_health_transitions_total
            .with_label_values(&["c", "127.0.0.1:80", "healthy", "unhealthy"])
            .inc();
        m.xds_updates_total
            .with_label_values(&["cds", "cluster_added"])
            .inc();
        m.xds_current_version.with_label_values(&["cds"]).set(1);
        m.xds_nack_total
            .with_label_values(&["cds", "parse_error"])
            .inc();
        m.mesh_svid_rotations_total
            .with_label_values(&["spiffe://x"])
            .inc();
        m.mesh_svid_expires_seconds
            .with_label_values(&["spiffe://x"])
            .set(9_999_999);
        m.mesh_svid_fetch_errors_total
            .with_label_values(&["timeout"])
            .inc();

        let encoder = TextEncoder::new();
        let text = encoder
            .encode_to_string(&r.gather())
            .expect("encode ok");

        let expected = [
            "armageddon_shadow_requests_total",
            "armageddon_shadow_diverged_total",
            "armageddon_shadow_latency_diff_seconds",
            "armageddon_shadow_sample_rate",
            "armageddon_traffic_split_decisions_total",
            "armageddon_traffic_split_sticky_sessions",
            "armageddon_forge_endpoint_up",
            "armageddon_forge_health_check_duration_seconds",
            "armageddon_forge_health_transitions_total",
            "armageddon_xds_updates_total",
            "armageddon_xds_current_version",
            "armageddon_xds_nack_total",
            "armageddon_mesh_svid_rotations_total",
            "armageddon_mesh_svid_expires_seconds",
            "armageddon_mesh_svid_fetch_errors_total",
        ];

        for name in expected {
            assert!(
                text.contains(name),
                "metric '{name}' not found in Prometheus text output"
            );
        }
    }
}
