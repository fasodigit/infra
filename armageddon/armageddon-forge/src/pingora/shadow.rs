// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Shadow-mode runtime for Pingora ↔ hyper parity validation.
//!
//! ## Diff sink integration (added post-M6)
//!
//! [`ShadowSampler`] now accepts an optional [`super::shadow_sink::DiffEventSender`].
//! When a divergence is classified (bucket ≠ `Identical`), the sampler builds a
//! [`super::shadow_sink::ShadowDiffEvent`] and calls
//! [`super::shadow_sink::DiffEventSender::try_send`].  The send is fire-and-forget
//! and non-blocking: if the channel is full the event is dropped and the metric
//! `armageddon_shadow_sink_dropped_total{reason="channel_full"}` is incremented.
//!
//! **Failure modes added by this integration:**
//!
//! | Scenario | Behaviour |
//! |----------|-----------|
//! | `diff_sink = None` | Classification still happens; no event is persisted |
//! | Channel full | Event dropped; `dropped_total` incremented; no panic |
//! | Background drain task panics | Channel is closed; subsequent `try_send` silently fails |
//!
//! # Overview
//!
//! When shadow mode is enabled, a fraction (`sample_rate`) of inbound
//! requests are duplicated and sent to a secondary (shadow) backend.  The
//! primary response is **always** returned to the downstream client; the
//! shadow response is compared asynchronously and the result is logged and
//! published as Prometheus metrics.
//!
//! This is gate M5-3 of the Pingora migration.  It implements the runtime
//! described in [`SHADOW-MODE.md`].
//!
//! # Topology
//!
//! ```text
//! inbound request
//!       │
//!       ▼
//!  primary handler (hyper :8080)
//!       │
//!       ├──── 10% sample ──────► shadow handler (pingora :8081) [fire-and-forget]
//!       │                              │
//!       │                         diff-queue (async, non-blocking)
//!       │
//!       ▼
//!  response to client  (ground truth — always primary)
//! ```
//!
//! # Sampling
//!
//! Uses a deterministic blake3 hash of `request_id` so the same request ID
//! always makes the same sampling decision.  Rate is controlled by an
//! `AtomicU32` (`shadow_sample_percent`) so it can be adjusted at runtime
//! without redeploy.
//!
//! Setting `shadow_sample_percent = 0` is the atomic rollback mechanism.
//!
//! # Multi-process note
//!
//! Pingora runs its workers in a separate process via `pingora::Server::run_forever`.
//! Running both hyper and Pingora in the **same process** on different ports
//! would require `tokio::spawn` for the hyper listener alongside Pingora's
//! own scheduler — which is straightforward with the forge bridge runtime.
//!
//! **Alternative (chosen for M5)**: the shadow comparison is implemented
//! fully within the hyper path as a `ForgeFilter`-like middleware that sends
//! a mirrored request to the Pingora listener port via a lightweight HTTP
//! client.  This avoids embedding Pingora's scheduler inside hyper's runtime.
//!
//! # Failure modes
//!
//! | Scenario | Behaviour |
//! |---|---|
//! | Shadow target unreachable | `timeout_on_pingora` bucket; primary response unaffected |
//! | Shadow response timeout | Same as above |
//! | Diff queue full (> 4096) | Shadow event dropped; `shadow_diff_dropped_total` incremented |
//! | `sample_percent = 0` | No shadow requests; feature atomically disabled |

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tracing::warn;

use crate::pingora::metrics::PingoraMetrics;
use crate::pingora::shadow_sink::{DiffEventSender, HeadersDiff, ShadowDiffEvent};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Top-level shadow-mode configuration.
///
/// Set in `forge.toml` under `[shadow_mode]`.
#[derive(Debug, Clone)]
pub struct ShadowModeConfig {
    /// Whether shadow mode is active.
    pub enabled: bool,
    /// Port the primary (hyper) listener is bound to.
    pub hyper_port: u16,
    /// Port the shadow (Pingora) listener is bound to.
    pub pingora_port: u16,
    /// Fraction of requests to mirror, expressed as an integer percentage
    /// 0–100.  The live value is stored in an `AtomicU32` for runtime flip.
    pub sample_rate_percent: u32,
    /// Timeout for shadow requests.
    pub shadow_timeout_ms: u64,
}

impl Default for ShadowModeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            hyper_port: 8080,
            pingora_port: 8081,
            sample_rate_percent: 10,
            shadow_timeout_ms: 5_000,
        }
    }
}

// ---------------------------------------------------------------------------
// MirroredResponse — result of one completed request side
// ---------------------------------------------------------------------------

/// Captured metadata from one completed HTTP response (primary or shadow).
#[derive(Debug, Clone)]
pub struct MirroredResponse {
    /// HTTP status code.
    pub status: u16,
    /// Sorted, lowercased header names and values (infrastructure-stripped).
    pub headers: Vec<(String, Vec<u8>)>,
    /// blake3 hash of the response body.
    pub body_hash: [u8; 32],
    /// Body byte length.
    pub body_len: usize,
    /// When this side finished.
    pub finished_at: Instant,
}

// ---------------------------------------------------------------------------
// DiffBucket — classification of a (primary, shadow) pair
// ---------------------------------------------------------------------------

/// Classification of a shadow comparison pair.
///
/// Ordered: first matching rule wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiffBucket {
    /// Pingora slot did not arrive before the 30 s TTL.
    TimeoutOnPingora,
    /// Hyper slot did not arrive (request failed on primary path).
    TimeoutOnHyper,
    /// HTTP status codes differ.
    StatusDiffer,
    /// Response bodies differ (hash or length mismatch).
    BodyDiffer,
    /// Headers differ after normalisation.
    HeaderDiffer,
    /// Streaming response — not comparable.
    Streaming,
    /// Primary and shadow responses are bit-exact (after normalisation).
    Identical,
}

impl DiffBucket {
    /// Classify a pair of completed responses.
    ///
    /// Normalisation strips infrastructure-added headers before comparison.
    pub fn classify(primary: &MirroredResponse, shadow: &MirroredResponse) -> Self {
        if primary.status != shadow.status {
            return Self::StatusDiffer;
        }
        if primary.body_hash != shadow.body_hash || primary.body_len != shadow.body_len {
            return Self::BodyDiffer;
        }
        let p_hdrs = normalise_headers(&primary.headers);
        let s_hdrs = normalise_headers(&shadow.headers);
        if p_hdrs != s_hdrs {
            return Self::HeaderDiffer;
        }
        Self::Identical
    }

    /// String label for Prometheus metric.
    pub fn label(&self) -> &'static str {
        match self {
            Self::TimeoutOnPingora => "timeout_on_pingora",
            Self::TimeoutOnHyper => "timeout_on_hyper",
            Self::StatusDiffer => "status_differ",
            Self::BodyDiffer => "body_differ",
            Self::HeaderDiffer => "header_differ",
            Self::Streaming => "streaming",
            Self::Identical => "identical",
        }
    }
}

// ---------------------------------------------------------------------------
// Header normalisation
// ---------------------------------------------------------------------------

/// Headers that are stripped before comparison — infrastructure-added noise.
const INFRA_HEADERS: &[&str] = &[
    "date",
    "server",
    "x-forge-id",
    "x-forge-via",
    "x-request-id",
];

/// Remove infrastructure headers, sort by name, return a stable list.
fn normalise_headers(headers: &[(String, Vec<u8>)]) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = headers
        .iter()
        .filter(|(name, _)| !INFRA_HEADERS.contains(&name.as_str()))
        .cloned()
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

// ---------------------------------------------------------------------------
// Sampling decision
// ---------------------------------------------------------------------------

/// Decide whether a request with the given `request_id` should be shadowed.
///
/// Uses a deterministic blake3 hash so retries with the same request ID
/// always produce the same decision.
///
/// `sample_percent` must be in `[0, 100]`.
pub fn should_shadow(request_id: &str, sample_percent: u32) -> bool {
    if sample_percent == 0 {
        return false;
    }
    if sample_percent >= 100 {
        return true;
    }
    let digest = blake3::hash(request_id.as_bytes());
    let bucket = u32::from_le_bytes(digest.as_bytes()[0..4].try_into().unwrap()) % 100;
    bucket < sample_percent
}

// ---------------------------------------------------------------------------
// ShadowSampler — runtime-adjustable sampling controller
// ---------------------------------------------------------------------------

/// Runtime-adjustable shadow sampling controller.
///
/// The `sample_percent` field is an `AtomicU32` so it can be flipped to 0
/// for instant rollback without a service restart.
pub struct ShadowSampler {
    /// Live sample rate 0–100 (inclusive).  Write 0 to disable shadow mode
    /// atomically.
    pub sample_percent: AtomicU32,
    /// Port of the shadow (Pingora) listener.
    pub shadow_port: u16,
    /// Per-shadow-request timeout.
    pub timeout: Duration,
    /// Shared Prometheus metrics bundle.  `None` disables metric emission
    /// (e.g. in unit tests that do not wire a registry).
    metrics: Option<Arc<PingoraMetrics>>,
    /// Optional channel sender to the diff sink background task.
    ///
    /// When `Some`, every non-`Identical` comparison result is forwarded as
    /// a [`ShadowDiffEvent`] via [`DiffEventSender::try_send`] (fire-and-forget,
    /// non-blocking).
    diff_sink: Option<DiffEventSender>,
}

impl ShadowSampler {
    /// Create a new sampler from config without Prometheus metrics.
    ///
    /// Prefer [`ShadowSampler::with_metrics`] when a shared `PingoraMetrics`
    /// bundle is available.
    pub fn new(config: &ShadowModeConfig) -> Arc<Self> {
        Arc::new(Self {
            sample_percent: AtomicU32::new(config.sample_rate_percent.min(100)),
            shadow_port: config.pingora_port,
            timeout: Duration::from_millis(config.shadow_timeout_ms),
            metrics: None,
            diff_sink: None,
        })
    }

    /// Create a new sampler from config with a shared metrics bundle.
    pub fn with_metrics(config: &ShadowModeConfig, metrics: Arc<PingoraMetrics>) -> Arc<Self> {
        let sampler = Arc::new(Self {
            sample_percent: AtomicU32::new(config.sample_rate_percent.min(100)),
            shadow_port: config.pingora_port,
            timeout: Duration::from_millis(config.shadow_timeout_ms),
            metrics: Some(metrics.clone()),
            diff_sink: None,
        });
        // Publish the initial sample rate.
        if let Some(m) = &sampler.metrics {
            m.shadow_sample_rate
                .with_label_values(&["shadow"])
                .set(i64::from(config.sample_rate_percent.min(100)));
        }
        sampler
    }

    /// Attach a diff sink sender.
    ///
    /// Returns a new `Arc<ShadowSampler>` with the sender wired in.  This is a
    /// builder-style API: call it after `new` or `with_metrics`.
    ///
    /// ```rust,ignore
    /// let sampler = ShadowSampler::new(&cfg)
    ///     .with_sink(dispatcher.sender());
    /// ```
    pub fn with_sink(self: Arc<Self>, sender: DiffEventSender) -> Arc<Self> {
        // Safety: we have the only Arc at this point (caller just constructed it).
        // We need to mutate the `diff_sink` field, which requires ownership.
        // We use `Arc::try_unwrap` + rebuild.
        let inner = Arc::try_unwrap(self).unwrap_or_else(|arc| {
            // Fallback: clone the data and rebuild (metrics is Option<Arc<_>>, so clone is cheap).
            let data = &*arc;
            ShadowSampler {
                sample_percent: AtomicU32::new(
                    data.sample_percent.load(Ordering::Relaxed),
                ),
                shadow_port: data.shadow_port,
                timeout: data.timeout,
                metrics: data.metrics.clone(),
                diff_sink: data.diff_sink.clone(),
            }
        });
        Arc::new(ShadowSampler {
            diff_sink: Some(sender),
            ..inner
        })
    }

    /// Test whether this request should be shadowed.
    pub fn should_shadow(&self, request_id: &str) -> bool {
        let pct = self.sample_percent.load(Ordering::Relaxed);
        should_shadow(request_id, pct)
    }

    /// Set the sample rate atomically.  `percent` is clamped to `[0, 100]`.
    pub fn set_sample_percent(&self, percent: u32) {
        let clamped = percent.min(100);
        self.sample_percent.store(clamped, Ordering::Relaxed);
        if let Some(m) = &self.metrics {
            m.shadow_sample_rate
                .with_label_values(&["shadow"])
                .set(i64::from(clamped));
        }
    }

    /// Disable shadow mode atomically (sets `sample_percent` to 0).
    pub fn disable(&self) {
        self.set_sample_percent(0);
    }

    /// Emit a [`ShadowDiffEvent`] to the wired sink when a divergence is
    /// detected.
    ///
    /// `route` — matched route template (e.g. `/api/v1/orders`).
    /// `primary` / `shadow` — completed responses from both sides.
    /// `bucket` — pre-computed classification (must be non-`Identical` for the
    ///            event to carry meaningful diff data; `Identical` events are
    ///            silently dropped before hitting the channel).
    /// `tenant_id` — optional tenant identifier for Redpanda partitioning.
    ///
    /// This method is synchronous (non-async): it calls
    /// [`DiffEventSender::try_send`] which is non-blocking.
    pub fn record_diff(
        &self,
        request_id: &str,
        route: &str,
        method: &str,
        primary: &MirroredResponse,
        shadow_resp: &MirroredResponse,
        bucket: DiffBucket,
        tenant_id: Option<String>,
    ) {
        let Some(sender) = &self.diff_sink else {
            return;
        };

        // Do not flood the sink with identical responses.
        if bucket == DiffBucket::Identical {
            return;
        }

        let diverged_fields: Vec<String> = match bucket {
            DiffBucket::StatusDiffer => vec!["status".to_string()],
            DiffBucket::BodyDiffer => vec!["body_hash".to_string()],
            DiffBucket::HeaderDiffer => vec!["headers".to_string()],
            DiffBucket::TimeoutOnPingora | DiffBucket::TimeoutOnHyper => {
                vec!["timeout".to_string()]
            }
            DiffBucket::Streaming => vec!["streaming".to_string()],
            DiffBucket::Identical => unreachable!(),
        };

        let headers_diff = if bucket == DiffBucket::HeaderDiffer {
            let p_hdrs = normalise_headers(&primary.headers);
            let s_hdrs = normalise_headers(&shadow_resp.headers);

            let only_in_hyper: Vec<(String, String)> = p_hdrs
                .iter()
                .filter(|(k, v)| {
                    !s_hdrs.iter().any(|(sk, sv)| sk == k && sv == v)
                })
                .map(|(k, v)| (k.clone(), String::from_utf8_lossy(v).into_owned()))
                .collect();

            let only_in_pingora: Vec<(String, String)> = s_hdrs
                .iter()
                .filter(|(k, v)| {
                    !p_hdrs.iter().any(|(pk, pv)| pk == k && pv == v)
                })
                .map(|(k, v)| (k.clone(), String::from_utf8_lossy(v).into_owned()))
                .collect();

            Some(HeadersDiff {
                only_in_hyper,
                only_in_pingora,
            })
        } else {
            None
        };

        // Convert blake3 bytes → hex strings.
        let hyper_body_hash = hex::encode(primary.body_hash);
        let pingora_body_hash = hex::encode(shadow_resp.body_hash);

        // Compute latencies from Instant deltas (relative — absolute ms are
        // computed from `Instant::now()` at the call site).
        let now = Instant::now();
        let hyper_latency_ms =
            now.duration_since(primary.finished_at).as_millis().min(u32::MAX as u128) as u32;
        let pingora_latency_ms =
            now.duration_since(shadow_resp.finished_at).as_millis().min(u32::MAX as u128) as u32;

        let event = ShadowDiffEvent {
            timestamp_unix_ms: ShadowDiffEvent::now_unix_ms(),
            request_id: request_id.to_string(),
            route: route.to_string(),
            method: method.to_string(),
            hyper_status: primary.status,
            pingora_status: shadow_resp.status,
            hyper_body_hash,
            pingora_body_hash,
            hyper_latency_ms,
            pingora_latency_ms,
            diverged_fields,
            headers_diff,
            tenant_id,
        };

        sender.try_send(event);
    }
}

// ---------------------------------------------------------------------------
// ShadowEvent — the result pushed to the diff sink
// ---------------------------------------------------------------------------

/// One completed shadow comparison event.
#[derive(Debug, Clone)]
pub struct ShadowEvent {
    /// RFC 3339 timestamp string.
    pub ts: String,
    /// Unique request identifier (uuid v4).
    pub request_id: String,
    /// HTTP method.
    pub method: String,
    /// Request path.
    pub path: String,
    /// Primary (hyper) status code.
    pub hyper_status: u16,
    /// Shadow (Pingora) status code. `None` on timeout.
    pub pingora_status: Option<u16>,
    /// Bucket classification.
    pub bucket: DiffBucket,
    /// Primary latency in microseconds.
    pub latency_us_hyper: u64,
    /// Shadow latency in microseconds. `None` on timeout.
    pub latency_us_pingora: Option<u64>,
    /// Primary body length.
    pub body_len_hyper: usize,
    /// Shadow body length. `None` on timeout.
    pub body_len_pingora: Option<usize>,
}

// ---------------------------------------------------------------------------
// ShadowDiffQueue — the channel between request hooks and the diff analyser
// ---------------------------------------------------------------------------

/// Bounded channel for `ShadowEvent`s.
///
/// The sender is cloned per-request; the receiver is consumed by the diff
/// analyser task.
pub struct ShadowDiffQueue {
    tx: tokio::sync::mpsc::Sender<ShadowEvent>,
    pub rx: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<ShadowEvent>>,
    /// Shared Prometheus metrics bundle (optional).
    metrics: Option<Arc<PingoraMetrics>>,
}

impl ShadowDiffQueue {
    /// Create a bounded queue with `capacity` slots without metrics.
    pub fn new(capacity: usize) -> Arc<Self> {
        let (tx, rx) = tokio::sync::mpsc::channel(capacity);
        Arc::new(Self {
            tx,
            rx: tokio::sync::Mutex::new(rx),
            metrics: None,
        })
    }

    /// Create a bounded queue with `capacity` slots and a metrics bundle.
    pub fn with_metrics(capacity: usize, metrics: Arc<PingoraMetrics>) -> Arc<Self> {
        let (tx, rx) = tokio::sync::mpsc::channel(capacity);
        Arc::new(Self {
            tx,
            rx: tokio::sync::Mutex::new(rx),
            metrics: Some(metrics),
        })
    }

    /// Push an event.  Returns `false` (and increments the drop counter) when
    /// the queue is full.
    pub fn push(&self, event: ShadowEvent) -> bool {
        match self.tx.try_send(event) {
            Ok(()) => true,
            Err(e) => {
                let reason = match e {
                    tokio::sync::mpsc::error::TrySendError::Full(_) => "full",
                    tokio::sync::mpsc::error::TrySendError::Closed(_) => "closed",
                };
                warn!("shadow diff queue {reason} — dropping event");
                increment_dropped_counter(self.metrics.as_deref());
                false
            }
        }
    }

    /// Depth of the queue (approximate, for metrics).
    pub fn depth(&self) -> usize {
        self.tx.max_capacity().saturating_sub(self.tx.capacity())
    }
}

// ---------------------------------------------------------------------------
// Metrics helpers
// ---------------------------------------------------------------------------

/// Increment `armageddon_shadow_requests_total{status="dropped"}` via the
/// diff-queue sender side.  `metrics` is optional; if absent, falls back to a
/// tracing log line.
fn increment_dropped_counter(metrics: Option<&PingoraMetrics>) {
    tracing::debug!("shadow diff queue full — event dropped");
    if let Some(m) = metrics {
        m.shadow_requests_total
            .with_label_values(&["dropped"])
            .inc();
    }
}

/// Increment `armageddon_shadow_requests_total{status=<bucket>}` and, when the
/// response diverged, also increment `armageddon_shadow_diverged_total{field}`.
pub fn record_shadow_outcome(bucket: DiffBucket, metrics: Option<&PingoraMetrics>) {
    tracing::debug!(outcome = bucket.label(), "shadow outcome recorded");
    let Some(m) = metrics else { return };
    m.shadow_requests_total
        .with_label_values(&[bucket.label()])
        .inc();
    // Increment the diverged counter for each contributing field.
    match bucket {
        DiffBucket::StatusDiffer => {
            m.shadow_diverged_total.with_label_values(&["status"]).inc();
        }
        DiffBucket::BodyDiffer => {
            m.shadow_diverged_total
                .with_label_values(&["body_hash"])
                .inc();
        }
        DiffBucket::HeaderDiffer => {
            m.shadow_diverged_total
                .with_label_values(&["headers"])
                .inc();
        }
        _ => {}
    }
}

/// Record `armageddon_shadow_latency_diff_seconds` for one comparison.
///
/// `diff_secs` = pingora_latency_seconds − hyper_latency_seconds
/// (negative means Pingora was faster).
pub fn record_shadow_latency_diff(
    route: &str,
    diff_secs: f64,
    metrics: Option<&PingoraMetrics>,
) {
    if let Some(m) = metrics {
        m.shadow_latency_diff_seconds
            .with_label_values(&[route])
            .observe(diff_secs);
    }
}

/// Increment `armageddon_shadow_requests_total{status="sampled"}`.
pub fn record_shadow_sampled(metrics: Option<&PingoraMetrics>) {
    if let Some(m) = metrics {
        m.shadow_requests_total
            .with_label_values(&["sampled"])
            .inc();
    } else {
        tracing::debug!("shadow request sampled");
    }
}

/// Increment `armageddon_shadow_requests_total{status="skipped"}`.
pub fn record_shadow_skipped(reason: &str, metrics: Option<&PingoraMetrics>) {
    tracing::debug!(reason, "shadow request skipped");
    if let Some(m) = metrics {
        m.shadow_requests_total
            .with_label_values(&["skipped"])
            .inc();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── should_shadow ──────────────────────────────────────────────────────

    /// With 0% rate, no request is ever shadowed.
    #[test]
    fn shadow_rate_zero_never_shadows() {
        for i in 0..200u32 {
            assert!(
                !should_shadow(&format!("req-{i}"), 0),
                "0% must never shadow"
            );
        }
    }

    /// With 100% rate, every request is shadowed.
    #[test]
    fn shadow_rate_hundred_always_shadows() {
        for i in 0..200u32 {
            assert!(
                should_shadow(&format!("req-{i}"), 100),
                "100% must always shadow"
            );
        }
    }

    /// With 50% rate, approximately half of a large sample is shadowed.
    #[test]
    fn shadow_rate_fifty_is_approximately_half() {
        let total = 10_000u32;
        let shadowed = (0..total)
            .filter(|i| should_shadow(&format!("req-{i}"), 50))
            .count();
        // 50% ± 5% is acceptable for a hash-based sampler.
        let pct = shadowed as f64 / total as f64 * 100.0;
        assert!(
            (45.0..=55.0).contains(&pct),
            "50% sample rate should produce 45–55% shadows, got {pct:.1}%"
        );
    }

    /// With 10% rate, approximately 10% of a large sample is shadowed.
    #[test]
    fn shadow_rate_ten_percent_respected() {
        let total = 10_000u32;
        let shadowed = (0..total)
            .filter(|i| should_shadow(&format!("uid-{i}"), 10))
            .count();
        let pct = shadowed as f64 / total as f64 * 100.0;
        assert!(
            (7.0..=13.0).contains(&pct),
            "10% sample rate should produce 7–13% shadows, got {pct:.1}%"
        );
    }

    /// Sampling is deterministic: same request ID → same decision.
    #[test]
    fn shadow_sampling_is_deterministic() {
        for i in 0..100u32 {
            let id = format!("deterministic-{i}");
            let d1 = should_shadow(&id, 30);
            let d2 = should_shadow(&id, 30);
            assert_eq!(d1, d2, "same request_id must always produce same decision");
        }
    }

    // ── DiffBucket::classify ───────────────────────────────────────────────

    fn make_resp(status: u16, body: &[u8], headers: Vec<(&str, &[u8])>) -> MirroredResponse {
        let headers = headers
            .into_iter()
            .map(|(k, v)| (k.to_lowercase(), v.to_vec()))
            .collect();
        let body_hash = *blake3::hash(body).as_bytes();
        MirroredResponse {
            status,
            headers,
            body_hash,
            body_len: body.len(),
            finished_at: Instant::now(),
        }
    }

    #[test]
    fn classify_identical_responses() {
        let body = b"hello world";
        let a = make_resp(200, body, vec![("content-type", b"text/plain")]);
        let b = make_resp(200, body, vec![("content-type", b"text/plain")]);
        assert_eq!(DiffBucket::classify(&a, &b), DiffBucket::Identical);
    }

    #[test]
    fn classify_status_differ() {
        let body = b"error";
        let a = make_resp(200, body, vec![]);
        let b = make_resp(500, body, vec![]);
        assert_eq!(DiffBucket::classify(&a, &b), DiffBucket::StatusDiffer);
    }

    #[test]
    fn classify_body_differ() {
        let a = make_resp(200, b"primary response", vec![]);
        let b = make_resp(200, b"shadow response", vec![]);
        assert_eq!(DiffBucket::classify(&a, &b), DiffBucket::BodyDiffer);
    }

    #[test]
    fn classify_header_differ() {
        let body = b"same body";
        let a = make_resp(200, body, vec![("x-custom", b"a")]);
        let b = make_resp(200, body, vec![("x-custom", b"b")]);
        assert_eq!(DiffBucket::classify(&a, &b), DiffBucket::HeaderDiffer);
    }

    /// Infrastructure headers (date, server, x-forge-via) are stripped before
    /// comparison — they must not cause false positives.
    #[test]
    fn classify_infra_headers_stripped() {
        let body = b"same";
        let a = make_resp(
            200,
            body,
            vec![
                ("date", b"Thu, 01 Jan 2026 00:00:00 GMT"),
                ("server", b"armageddon-hyper"),
                ("content-type", b"application/json"),
            ],
        );
        let b = make_resp(
            200,
            body,
            vec![
                ("date", b"Thu, 01 Jan 2026 00:00:01 GMT"),
                ("server", b"armageddon-pingora"),
                ("content-type", b"application/json"),
            ],
        );
        assert_eq!(
            DiffBucket::classify(&a, &b),
            DiffBucket::Identical,
            "infra headers (date, server) must be stripped before comparison"
        );
    }

    // ── ShadowSampler ──────────────────────────────────────────────────────

    #[test]
    fn sampler_disable_stops_mirroring() {
        let cfg = ShadowModeConfig {
            enabled: true,
            sample_rate_percent: 100,
            ..Default::default()
        };
        let sampler = ShadowSampler::new(&cfg);
        assert!(sampler.should_shadow("any-id"), "100% rate must shadow");
        sampler.disable();
        assert!(
            !sampler.should_shadow("any-id"),
            "after disable(), no request must be shadowed"
        );
    }

    #[test]
    fn sampler_runtime_rate_change() {
        let cfg = ShadowModeConfig {
            enabled: true,
            sample_rate_percent: 0,
            ..Default::default()
        };
        let sampler = ShadowSampler::new(&cfg);
        assert!(!sampler.should_shadow("req-1"), "0% must not shadow");
        sampler.set_sample_percent(100);
        assert!(sampler.should_shadow("req-1"), "100% must shadow");
    }

    // ── DiffBucket::label ──────────────────────────────────────────────────

    #[test]
    fn diff_bucket_labels_are_stable() {
        assert_eq!(DiffBucket::Identical.label(), "identical");
        assert_eq!(DiffBucket::StatusDiffer.label(), "status_differ");
        assert_eq!(DiffBucket::BodyDiffer.label(), "body_differ");
        assert_eq!(DiffBucket::HeaderDiffer.label(), "header_differ");
        assert_eq!(DiffBucket::TimeoutOnPingora.label(), "timeout_on_pingora");
        assert_eq!(DiffBucket::TimeoutOnHyper.label(), "timeout_on_hyper");
        assert_eq!(DiffBucket::Streaming.label(), "streaming");
    }

    // ── ShadowDiffQueue ────────────────────────────────────────────────────

    #[tokio::test]
    async fn diff_queue_push_and_depth() {
        let q = ShadowDiffQueue::new(16);
        let event = ShadowEvent {
            ts: "2026-01-01T00:00:00Z".to_string(),
            request_id: "uuid-1".to_string(),
            method: "GET".to_string(),
            path: "/api/v1/test".to_string(),
            hyper_status: 200,
            pingora_status: Some(200),
            bucket: DiffBucket::Identical,
            latency_us_hyper: 1000,
            latency_us_pingora: Some(900),
            body_len_hyper: 42,
            body_len_pingora: Some(42),
        };
        assert!(q.push(event.clone()));
        assert_eq!(q.depth(), 1);
        assert!(q.push(event));
        assert_eq!(q.depth(), 2);
    }

    // ── Metrics wiring ─────────────────────────────────────────────────────

    /// ShadowSampler::with_metrics updates the sample_rate gauge on construction.
    #[test]
    fn sampler_with_metrics_publishes_sample_rate() {
        use crate::pingora::metrics::PingoraMetrics;
        use prometheus::Registry;
        use std::sync::Arc;

        let r = Registry::new();
        let m = Arc::new(PingoraMetrics::new(&r).unwrap());
        let cfg = ShadowModeConfig {
            sample_rate_percent: 25,
            ..Default::default()
        };
        let _sampler = ShadowSampler::with_metrics(&cfg, Arc::clone(&m));

        let families = r.gather();
        let rate = families
            .iter()
            .find(|f| f.get_name() == "armageddon_shadow_sample_rate")
            .expect("gauge must exist");
        let val = rate
            .get_metric()
            .first()
            .map(|m| m.get_gauge().get_value())
            .unwrap_or(0.0);
        assert_eq!(val, 25.0, "initial sample rate should be 25");
    }

    /// set_sample_percent updates both the atomic and the gauge.
    #[test]
    fn sampler_set_sample_percent_updates_gauge() {
        use crate::pingora::metrics::PingoraMetrics;
        use prometheus::Registry;
        use std::sync::Arc;

        let r = Registry::new();
        let m = Arc::new(PingoraMetrics::new(&r).unwrap());
        let cfg = ShadowModeConfig {
            sample_rate_percent: 10,
            ..Default::default()
        };
        let sampler = ShadowSampler::with_metrics(&cfg, Arc::clone(&m));
        sampler.set_sample_percent(50);

        let families = r.gather();
        let rate = families
            .iter()
            .find(|f| f.get_name() == "armageddon_shadow_sample_rate")
            .expect("gauge must exist");
        let val = rate
            .get_metric()
            .first()
            .map(|m| m.get_gauge().get_value())
            .unwrap_or(0.0);
        assert_eq!(val, 50.0, "sample rate should be updated to 50");
    }

    /// record_shadow_outcome increments the correct counter and diverged counter.
    #[test]
    fn record_shadow_outcome_increments_counters() {
        use crate::pingora::metrics::PingoraMetrics;
        use prometheus::Registry;
        use std::sync::Arc;

        let r = Registry::new();
        let m = Arc::new(PingoraMetrics::new(&r).unwrap());

        record_shadow_outcome(DiffBucket::StatusDiffer, Some(&m));
        record_shadow_outcome(DiffBucket::BodyDiffer, Some(&m));
        record_shadow_outcome(DiffBucket::Identical, Some(&m));

        let families = r.gather();

        let requests = families
            .iter()
            .find(|f| f.get_name() == "armageddon_shadow_requests_total")
            .expect("requests counter must exist");
        // status_differ + body_differ + identical = 3 label-value combinations.
        assert!(requests.get_metric().len() >= 2);

        let diverged = families
            .iter()
            .find(|f| f.get_name() == "armageddon_shadow_diverged_total")
            .expect("diverged counter must exist");
        // status and body_hash fields should each have 1 count.
        assert!(diverged.get_metric().len() >= 2);
    }
}
